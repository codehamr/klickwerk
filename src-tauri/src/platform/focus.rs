use crate::diagnostics::FocusedElement;
use windows::{
    Win32::{
        System::{Com::*, Ole::*},
        UI::Accessibility::*,
    },
    core::Result,
};
use windows_sys::Win32::{
    Foundation::*,
    UI::{Input::KeyboardAndMouse::IsWindowEnabled, WindowsAndMessaging::*},
};

// Read-only field identification. No UIA invocation, text values, or password data.
// Run on the capture worker, never the UI thread; unknown fields retain the
// conservative full-screen validation. Each provider call has a bounded timeout.
pub fn inspect(foreground: usize, focused: usize) -> (Option<FocusedElement>, Option<String>) {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(foreground as HWND, &mut pid);
        if pid == 0 || pid == std::process::id() {
            return (None, None);
        }
        if focused != 0 && GetAncestor(focused as HWND, GA_ROOT) as usize == foreground {
            let mut class = [0u16; 128];
            let count = GetClassNameW(focused as HWND, class.as_mut_ptr(), class.len() as i32);
            let class =
                String::from_utf16_lossy(&class[..count.max(0) as usize]).to_ascii_lowercase();
            if (class == "edit" || class.starts_with("richedit"))
                && GetWindowLongPtrW(focused as HWND, GWL_STYLE) & ES_PASSWORD as isize == 0
                && IsWindowVisible(focused as HWND) != 0
                && IsWindowEnabled(focused as HWND) != 0
            {
                let mut rect: RECT = std::mem::zeroed();
                if GetWindowRect(focused as HWND, &mut rect) != 0 {
                    return (
                        Some(FocusedElement {
                            source: "win32_edit".into(),
                            identity: vec![focused as i32, ((focused as u64) >> 32) as i32],
                            bounds: [rect.left, rect.top, rect.right, rect.bottom],
                            process_id: pid,
                        }),
                        None,
                    );
                }
            }
        }
        let initialized = CoInitializeEx(None, COINIT_MULTITHREADED);
        if initialized.is_err() {
            return (
                None,
                Some(format!(
                    "uia_com_initialization_failed: {:#x}",
                    initialized.0
                )),
            );
        }
        let result = editable_element(pid);
        CoUninitialize();
        match result {
            Ok(element) => (element, None),
            Err(error) => (
                None,
                Some(format!(
                    "uia_focus_inspection_failed: {:#x}",
                    error.code().0
                )),
            ),
        }
    }
}

unsafe fn editable_element(pid: u32) -> Result<Option<FocusedElement>> {
    unsafe {
        let automation: IUIAutomation2 =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
        automation.SetConnectionTimeout(200)?;
        automation.SetTransactionTimeout(200)?;
        let element = automation.GetFocusedElement()?;
        if element.CurrentControlType()? != UIA_EditControlTypeId
            || element.CurrentProcessId()? as u32 != pid
            || !element.CurrentHasKeyboardFocus()?.as_bool()
            || !element.CurrentIsEnabled()?.as_bool()
            || element.CurrentIsPassword()?.as_bool()
            || element.CurrentIsOffscreen()?.as_bool()
        {
            return Ok(None);
        }
        let rect = element.CurrentBoundingRectangle()?;
        let array = element.GetRuntimeId()?;
        if array.is_null() {
            return Ok(None);
        }
        let identity = (|| -> Result<Vec<i32>> {
            if SafeArrayGetDim(array) != 1 || SafeArrayGetElemsize(array) != 4 {
                return Ok(vec![]);
            }
            let low = SafeArrayGetLBound(array, 1)?;
            let high = SafeArrayGetUBound(array, 1)?;
            let count = high as i64 - low as i64 + 1;
            if !(1..=64).contains(&count) {
                return Ok(vec![]);
            }
            let mut data = std::ptr::null_mut();
            SafeArrayAccessData(array, &mut data)?;
            let values = if data.is_null() {
                vec![]
            } else {
                std::slice::from_raw_parts(data as *const i32, count as usize).to_vec()
            };
            SafeArrayUnaccessData(array)?;
            Ok(values)
        })();
        let _ = SafeArrayDestroy(array);
        let identity = identity?;
        if identity.is_empty() {
            return Ok(None);
        }
        Ok(Some(FocusedElement {
            source: "uia_edit".into(),
            identity,
            bounds: [rect.left, rect.top, rect.right, rect.bottom],
            process_id: pid,
        }))
    }
}
