use super::Handle;
use crate::diagnostics::{Handoff, TargetInfo, WindowInfo, input_block};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    System::Threading::*,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub fn privileges(pid: u32) -> Result<(u32, bool), u32> {
    unsafe {
        let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            return Err(GetLastError());
        }
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return Err(GetLastError());
        }
        let token = Handle(token);
        let mut needed = 0;
        GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
        if needed == 0 || needed > 65536 {
            return Err(GetLastError());
        }
        // Token information contains pointers and must be pointer-aligned.
        let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            buffer.as_mut_ptr() as _,
            needed,
            &mut needed,
        ) == 0
        {
            return Err(GetLastError());
        }
        let label = &*(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        if IsValidSid(label.Label.Sid) == 0 {
            return Err(ERROR_INVALID_SID);
        }
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        if count == 0 {
            return Err(ERROR_INVALID_SID);
        }
        let integrity = *GetSidSubAuthority(label.Label.Sid, count as u32 - 1);
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        if GetTokenInformation(
            token.0,
            TokenElevation,
            &mut elevation as *mut _ as _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut needed,
        ) == 0
        {
            return Err(GetLastError());
        }
        Ok((integrity, elevation.TokenIsElevated != 0))
    }
}

pub fn inspect(handle: usize, parent: u32) -> TargetInfo {
    unsafe {
        let root = GetAncestor(handle as HWND, GA_ROOT);
        let mut info = WindowInfo {
            handle: root as usize,
            ..WindowInfo::default()
        };
        GetWindowThreadProcessId(root, &mut info.process_id);
        let mut buffer = [0u16; 512];
        let count = GetWindowTextW(root, buffer.as_mut_ptr(), buffer.len() as i32);
        info.title = String::from_utf16_lossy(&buffer[..count.max(0) as usize]);
        let count = GetClassNameW(root, buffer.as_mut_ptr(), buffer.len() as i32);
        info.class_name = String::from_utf16_lossy(&buffer[..count.max(0) as usize]);
        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(root, &mut rect);
        info.bounds = [rect.left, rect.top, rect.right, rect.bottom];
        let process = Handle(OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            info.process_id,
        ));
        let mut length = buffer.len() as u32;
        if !process.0.is_null()
            && QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length) != 0
        {
            let path = String::from_utf16_lossy(&buffer[..length as usize]);
            info.executable = path.rsplit(['\\', '/']).next().unwrap_or_default().into();
        }
        match privileges(info.process_id) {
            Ok((level, elevated)) => {
                info.integrity_level = Some(level);
                info.elevated = Some(elevated);
            }
            Err(error) => info.inspection_error = Some(error),
        }
        let sender = privileges(GetCurrentProcessId()).ok().map(|v| v.0);
        let block = input_block(
            info.process_id == parent || info.process_id == GetCurrentProcessId(),
            sender,
            info.integrity_level,
        )
        .map(str::to_owned);
        TargetInfo {
            window: info,
            sender_integrity_level: sender,
            input_block: block,
        }
    }
}

// Called on the window's UI thread, after the broker has released all input.
// No synthetic keys or permanent topmost state; every input-queue attachment is detached.
pub fn handoff(handle: usize) -> Handoff {
    unsafe {
        let window = handle as HWND;
        let before = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(before, std::ptr::null_mut());
        let mut report = Handoff {
            method: "restore_raise_activate".into(),
            foreground_before: before as usize,
            foreground_after: 0,
            visible: false,
            minimized: false,
            focused: false,
            attachment_error: None,
        };
        if super::default_desktop() {
            if IsIconic(window) != 0 {
                ShowWindow(window, SW_RESTORE);
            } else {
                ShowWindow(window, SW_SHOW);
            }
            SetForegroundWindow(window);
            let attach = GetForegroundWindow() != window
                && foreground_thread != 0
                && foreground_thread != current_thread;
            let responsive = !attach
                || SendMessageTimeoutW(
                    before,
                    WM_NULL,
                    0,
                    0,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    100,
                    std::ptr::null_mut(),
                ) != 0;
            let attached = attach
                && responsive
                && AttachThreadInput(current_thread, foreground_thread, 1) != 0;
            if attach && !attached {
                report.attachment_error = Some(GetLastError());
            }
            if attached {
                report.method = "restore_raise_attach_activate".into();
            }
            let was_topmost = GetWindowLongPtrW(window, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0;
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE;
            SetWindowPos(window, HWND_TOPMOST, 0, 0, 0, 0, flags);
            BringWindowToTop(window);
            SetForegroundWindow(window);
            SetActiveWindow(window);
            if GetAncestor(GetFocus(), GA_ROOT) != window {
                SetFocus(window);
            }
            if !was_topmost {
                SetWindowPos(window, HWND_NOTOPMOST, 0, 0, 0, 0, flags);
            }
            if attached {
                AttachThreadInput(current_thread, foreground_thread, 0);
            }
        } else {
            report.method = "unavailable_input_desktop".into();
        }
        report.foreground_after = GetForegroundWindow() as usize;
        report.visible = IsWindowVisible(window) != 0;
        report.minimized = IsIconic(window) != 0;
        report.focused =
            report.foreground_after == handle && GetAncestor(GetFocus(), GA_ROOT) == window;
        report
    }
}
