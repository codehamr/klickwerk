use super::{
    Handle, default_desktop,
    input::{HeldInput, INPUT_TAG, Plan},
    now, wide,
};
use crate::guard::{Command, Gate, Reply};
use std::{
    io::{BufRead, Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::*, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

static STOP: AtomicBool = AtomicBool::new(false);
static ARMED: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
static POINTER: AtomicU64 = AtomicU64::new(0);
static REASON: AtomicU32 = AtomicU32::new(0);

fn latch(reason: u32) {
    REASON
        .compare_exchange(0, reason, Ordering::SeqCst, Ordering::SeqCst)
        .ok();
    STOP.store(true, Ordering::SeqCst);
}
fn reason() -> &'static str {
    match REASON.load(Ordering::SeqCst) {
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
            if STOP.load(Ordering::SeqCst)
                && event.dwExtraInfo == INPUT_TAG
                && event.flags & LLKHF_UP == 0
            {
                return 1;
            }
            if event.dwExtraInfo != INPUT_TAG
                && (RUNNING.load(Ordering::SeqCst) || event.flags & LLKHF_UP == 0)
                && ARMED.load(Ordering::SeqCst)
            {
                #[cfg(feature = "safety-tests")]
                eprintln!(
                    "Takeover event: kind={wparam} flags={} tag={}",
                    event.flags, event.dwExtraInfo
                );
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
            let position = ((event.pt.x as u32 as u64) << 32) | event.pt.y as u32 as u64;
            let previous = POINTER.swap(position, Ordering::SeqCst);
            // Windows may echo a cursor-position update without its injection tag.
            // A duplicate position is not movement; buttons and wheels always count.
            let moved = wparam as u32 != WM_MOUSEMOVE || previous != position;
            if STOP.load(Ordering::SeqCst)
                && event.dwExtraInfo == INPUT_TAG
                && !matches!(wparam as u32, WM_LBUTTONUP | WM_RBUTTONUP)
            {
                return 1;
            }
            if event.dwExtraInfo != INPUT_TAG && moved && ARMED.load(Ordering::SeqCst) {
                #[cfg(feature = "safety-tests")]
                eprintln!(
                    "Mouse takeover: kind={wparam} flags={} tag={}",
                    event.flags, event.dwExtraInfo
                );
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
            WM_CLOSE => {
                latch(4);
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
            WM_DESTROY => {
                latch(4);
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
        let class = wide("KlickwerkInputMonitor");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("The input monitor could not be registered.".into());
        }
        // A hidden top-level window receives display, power, and session notifications.
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("klickwerk input monitor").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            return Err("The input monitor could not be created.".into());
        }
        let mut pointer: POINT = std::mem::zeroed();
        GetCursorPos(&mut pointer);
        POINTER.store(
            ((pointer.x as u32 as u64) << 32) | pointer.y as u32 as u64,
            Ordering::SeqCst,
        );
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), instance, 0);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0);
        if keyboard.is_null() || mouse.is_null() {
            if !keyboard.is_null() {
                UnhookWindowsHookEx(keyboard);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
            DestroyWindow(window);
            return Err("The physical takeover hooks could not be installed. No desktop control was started.".into());
        }
        if WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) == 0 {
            latch(5);
        }
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
        ARMED.store(true, Ordering::SeqCst);
        if !reply(&Reply::Ready { bounds: [0; 4] }) {
            latch(4);
        }
        while !STOP.load(Ordering::SeqCst) {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    latch(4);
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
            if !default_desktop() {
                latch(5);
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
            if !gate.armed {
                let all_released = (1..256).all(|key| GetAsyncKeyState(key) >= 0);
                if clock - started >= 2000 && all_released {
                    gate.armed = true;
                    RUNNING.store(true, Ordering::SeqCst);
                    if !reply(&Reply::Armed) {
                        latch(4);
                    }
                } else if clock - started > 15000 {
                    latch(3);
                }
            }
            if let Some((sequence, ref mut current, ref mut due)) = plan {
                if !gate.can_input(clock) {
                    latch(7);
                    break;
                }
                // Pump hooks before every balanced batch so takeover also interrupts long text.
                for _ in 0..32 {
                    while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
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
            let wait = if plan.as_ref().is_some_and(|(_, _, due)| *due <= now()) {
                0
            } else {
                5
            };
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, wait, QS_ALLINPUT);
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
        DestroyWindow(window);
        // The stdin reader is intentionally not joined: it may be waiting for the parent.
        std::thread::sleep(Duration::from_millis(10));
        Ok(())
    }
}
