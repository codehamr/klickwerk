use super::{
    Handle, default_desktop,
    input::{HeldInput, INPUT_TAG, Plan},
    now, wide,
};
use crate::guard::{Command, Gate, Reply};
use std::{
    io::{BufRead, Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::*, Threading::*},
    UI::{Controls::DRAWITEMSTRUCT, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

static STOP: AtomicBool = AtomicBool::new(false);
static ARMED: AtomicBool = AtomicBool::new(false);
static REASON: AtomicU32 = AtomicU32::new(0);

fn latch(reason: u32) {
    REASON
        .compare_exchange(0, reason, Ordering::SeqCst, Ordering::SeqCst)
        .ok();
    STOP.store(true, Ordering::SeqCst);
}
fn reason() -> &'static str {
    match REASON.load(Ordering::SeqCst) {
        1 => "Stopped with Ctrl + Alt + F8.",
        2 => "Stopped from the emergency stop bar.",
        3 => "Stopped because you used the mouse or keyboard.",
        4 => "Stopped because the main app closed or lost its connection.",
        5 => "Stopped because the desktop, display, or session changed.",
        6 => "Stopped because the controller heartbeat expired.",
        7 => "Stopped because an input permission expired or an action was refused.",
        _ => "Stopped. Your mouse and keyboard are yours again.",
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 {
            let event = &*(lparam as *const KBDLLHOOKSTRUCT);
            if event.dwExtraInfo != INPUT_TAG
                && event.flags & LLKHF_INJECTED == 0
                && ARMED.load(Ordering::SeqCst)
            {
                latch(3);
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }
}
unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code >= 0 {
            let event = &*(lparam as *const MSLLHOOKSTRUCT);
            if event.dwExtraInfo != INPUT_TAG
                && event.flags & LLMHF_INJECTED == 0
                && ARMED.load(Ordering::SeqCst)
            {
                latch(3);
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_HOTKEY => {
                latch(1);
                0
            }
            WM_COMMAND if wparam & 0xffff == 1 => {
                latch(2);
                0
            }
            WM_CLOSE | WM_LBUTTONDOWN => {
                latch(2);
                0
            }
            WM_DISPLAYCHANGE | WM_WTSSESSION_CHANGE => {
                latch(5);
                0
            }
            WM_POWERBROADCAST if wparam == PBT_APMSUSPEND as usize => {
                latch(5);
                1
            }
            WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
            WM_CTLCOLORSTATIC => {
                SetTextColor(wparam as HDC, 0x00ffffff);
                SetBkMode(wparam as HDC, TRANSPARENT as i32);
                GetStockObject(BLACK_BRUSH) as isize
            }
            WM_DRAWITEM => {
                let item = &*(lparam as *const DRAWITEMSTRUCT);
                let brush = CreateSolidBrush(0x004635c7);
                FillRect(item.hDC, &item.rcItem, brush);
                DeleteObject(brush);
                SetBkMode(item.hDC, TRANSPARENT as i32);
                SetTextColor(item.hDC, 0x00ffffff);
                let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0);
                let old = SelectObject(item.hDC, font as _);
                let mut rect = item.rcItem;
                DrawTextW(
                    item.hDC,
                    wide("STOP").as_ptr(),
                    -1,
                    &mut rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                SelectObject(item.hDC, old);
                1
            }
            WM_DESTROY => {
                latch(2);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn reply(value: &Reply) -> bool {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value).is_ok()
        && output.write_all(b"\n").and_then(|_| output.flush()).is_ok()
}

pub fn run_if_requested() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--stop-broker") {
        return false;
    }
    let result = (|| {
        if args.len() != 5 {
            return Err("The stop broker arguments are invalid.".into());
        }
        let parent = args[2]
            .parse::<u32>()
            .map_err(|_| "The controller process is invalid.")?;
        if !args[3].starts_with("Local\\klickwerk-") {
            return Err("The cleanup channel is invalid.".into());
        }
        run(parent, &args[3], args[4] == "simulate")
    })();
    if let Err(message) = result {
        reply(&Reply::Error { message });
    }
    true
}

fn run(parent_id: u32, mapping: &str, simulate: bool) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if !default_desktop() {
            return Err("The default Windows desktop is unavailable.".into());
        }
        let parent = Handle(OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_id));
        if parent.0.is_null() {
            return Err("The controller process could not be verified.".into());
        }
        let held = HeldInput::open(mapping, false)?;
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("KlickwerkEmergencyStopV2");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: GetStockObject(BLACK_BRUSH) as _,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("The emergency stop window could not be registered.".into());
        }
        let mut cursor: POINT = std::mem::zeroed();
        GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err("The stop bar monitor could not be located.".into());
        }
        let scale = GetDpiForSystem() as f64 / 96.0;
        let dip = |n: i32| (n as f64 * scale).round() as i32;
        let width = dip(490);
        let height = dip(88);
        let left = info.rcWork.right - width - dip(18);
        let top = info.rcWork.top + dip(18);
        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("klickwerk emergency stop — Ctrl + Alt + F8").as_ptr(),
            WS_POPUP,
            left,
            top,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            return Err("The emergency stop bar could not be created.".into());
        }
        let scale = GetDpiForWindow(window).max(96) as f64 / 96.0;
        let dip = |n: i32| (n as f64 * scale).round() as i32;
        let width = dip(490);
        let height = dip(88);
        let left = info.rcWork.right - width - dip(18);
        let top = info.rcWork.top + dip(18);
        let corner = 2u32;
        DwmSetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner as *const _ as _,
            4,
        );
        SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE);
        let font = CreateFontW(
            -dip(20),
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let small = CreateFontW(
            -dip(12),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let label = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("Ctrl + Alt + F8").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            dip(22),
            dip(18),
            dip(290),
            dip(28),
            window,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        let status = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("Starting in 3 seconds · let go of your controls").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            dip(22),
            dip(50),
            dip(330),
            dip(21),
            window,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        let button = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("STOP").as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_OWNERDRAW as u32,
            dip(375),
            dip(18),
            dip(95),
            dip(52),
            window,
            1usize as _,
            instance,
            std::ptr::null(),
        );
        SendMessageW(label, WM_SETFONT, font as usize, 1);
        SendMessageW(status, WM_SETFONT, small as usize, 1);
        SendMessageW(button, WM_SETFONT, font as usize, 1);
        if label.is_null()
            || status.is_null()
            || button.is_null()
            || font.is_null()
            || small.is_null()
        {
            DestroyWindow(window);
            return Err("The stop controls could not be created.".into());
        }
        if RegisterHotKey(
            window,
            1,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_F8 as u32,
        ) == 0
        {
            DestroyWindow(window);
            DeleteObject(font);
            DeleteObject(small);
            return Err("Ctrl + Alt + F8 is already used by another app. No desktop control was started. Free this shortcut and try again.".into());
        }
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), instance, 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0);
        if keyboard.is_null() || mouse.is_null() {
            if !keyboard.is_null() {
                UnhookWindowsHookEx(keyboard);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
            UnregisterHotKey(window, 1);
            DestroyWindow(window);
            DeleteObject(font);
            DeleteObject(small);
            return Err("The physical takeover hooks could not be installed. No desktop control was started.".into());
        }
        if WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) == 0 {
            latch(5);
        }
        SetWindowPos(
            window,
            HWND_TOPMOST,
            left,
            top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        UpdateWindow(window);
        let (sender, receiver) = mpsc::sync_channel::<Command>(16);
        std::thread::spawn(move || {
            let input = std::io::stdin();
            let mut reader = input.lock();
            loop {
                let mut line = Vec::new();
                let result = std::io::Read::by_ref(&mut reader)
                    .take(65537)
                    .read_until(b'\n', &mut line);
                if !matches!(result,Ok(n) if n>0 && n<=65536 && line.last()==Some(&b'\n')) {
                    latch(4);
                    break;
                }
                match serde_json::from_slice::<Command>(&line) {
                    Ok(Command::Stop) => {
                        latch(0);
                        break;
                    }
                    Ok(command) => {
                        if sender.try_send(command).is_err() {
                            latch(4);
                            break;
                        }
                    }
                    Err(_) => {
                        latch(4);
                        break;
                    }
                }
            }
        });
        let started = now();
        let mut gate = Gate::new(started);
        let mut plan: Option<(u64, Plan, u64)> = None;
        let mut last_topmost = 0;
        let mut last_label = 0;
        if !reply(&Reply::Ready {
            bounds: [left, top, left + width, top + height],
        }) {
            latch(4);
        }
        while !STOP.load(Ordering::SeqCst) {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    latch(2);
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let clock = now();
            if STOP.load(Ordering::SeqCst) {
                break;
            }
            if WaitForSingleObject(parent.0, 0) != WAIT_TIMEOUT {
                latch(4);
                break;
            }
            if !default_desktop() || IsWindowVisible(window) == 0 || IsIconic(window) != 0 {
                latch(5);
                break;
            }
            if GetAsyncKeyState(VK_CONTROL as i32) < 0
                && GetAsyncKeyState(VK_MENU as i32) < 0
                && GetAsyncKeyState(VK_F8 as i32) < 0
            {
                latch(1);
                break;
            }
            while let Ok(command) = receiver.try_recv() {
                match command {
                    Command::Heartbeat {
                        sent_ms,
                        lease_sequence,
                    } => {
                        if !gate.heartbeat(now(), sent_ms, lease_sequence) {
                            latch(6);
                        }
                    }
                    Command::Execute {
                        sequence,
                        sent_ms,
                        frame,
                        action,
                    } => {
                        if !gate.begin(now(), sent_ms, sequence)
                            || clock < frame.captured_ms
                            || clock - frame.captured_ms > 5000
                        {
                            latch(7);
                            break;
                        }
                        match Plan::new(frame, action, parent_id) {
                            Ok(value) => {
                                plan = Some((sequence, value, clock));
                            }
                            Err(message) => {
                                reply(&Reply::Error { message });
                                latch(7);
                                break;
                            }
                        }
                    }
                    Command::Stop => latch(0),
                }
            }
            let clock = now();
            if !gate.healthy(clock) {
                latch(6);
                break;
            }
            if clock - last_topmost >= 250 {
                if SetWindowPos(
                    window,
                    HWND_TOPMOST,
                    left,
                    top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                ) == 0
                {
                    latch(5);
                    break;
                }
                last_topmost = clock;
            }
            if !gate.armed {
                let all_released = (1..256).all(|key| GetAsyncKeyState(key) >= 0);
                if clock - started >= 3000 && all_released {
                    gate.armed = true;
                    ARMED.store(true, Ordering::SeqCst);
                    SetWindowTextW(
                        status,
                        wide(if simulate {
                            "Stop test · no desktop input is sent"
                        } else {
                            "Agent running · stop from any app"
                        })
                        .as_ptr(),
                    );
                    if !reply(&Reply::Armed) {
                        latch(4);
                    }
                } else if clock - last_label >= 250 {
                    let remaining = 3000u64.saturating_sub(clock - started).div_ceil(1000);
                    let text = if remaining == 0 {
                        "Let go of your mouse and keyboard to begin".into()
                    } else {
                        format!("Starting in {remaining} seconds · let go of your controls")
                    };
                    SetWindowTextW(status, wide(&text).as_ptr());
                    last_label = clock;
                    if clock - started > 15000 {
                        latch(3);
                    }
                }
            }
            if let Some((sequence, ref mut current, ref mut due)) = plan {
                if !gate.can_input(clock) {
                    latch(7);
                    break;
                }
                // At most eight balanced batches per turn; messages are pumped between turns.
                for _ in 0..8 {
                    if STOP.load(Ordering::SeqCst) || now() < *due {
                        break;
                    }
                    let Some(batch) = current.batches.pop_front() else {
                        break;
                    };
                    if !gate.can_input(now()) || !current.target_valid(batch.point) {
                        latch(7);
                        break;
                    }
                    if !simulate && !held.send(&batch.events) {
                        latch(7);
                        break;
                    }
                    *due = now() + current.batches.front().map_or(0, |b| b.delay_ms);
                }
                if current.batches.is_empty() && !STOP.load(Ordering::SeqCst) {
                    gate.complete();
                    plan = None;
                    if !reply(&Reply::Completed { sequence }) {
                        latch(4);
                    }
                }
            }
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 10, QS_ALLINPUT);
        }
        gate.stop();
        ARMED.store(false, Ordering::SeqCst);
        held.release();
        reply(&Reply::Stopped {
            reason: reason().into(),
        });
        WTSUnRegisterSessionNotification(window);
        UnhookWindowsHookEx(keyboard);
        UnhookWindowsHookEx(mouse);
        UnregisterHotKey(window, 1);
        DestroyWindow(window);
        DeleteObject(font);
        DeleteObject(small);
        // The stdin reader is intentionally not joined: it may be waiting for the parent.
        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }
}
