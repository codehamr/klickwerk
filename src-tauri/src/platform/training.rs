//! Explicit, read-only demonstration capture. This module never dispatches input.
use crate::training::{self, Input, Options, Report, Screenshot, Window};
use base64::Engine;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

struct Raw {
    at: u64,
    foreground: usize,
    focused: usize,
    input: RawInput,
}
// Keep the keyboard snapshot inline: the bounded queue costs about 70 KiB and
// avoids a separate heap allocation on every low-level keyboard callback.
#[allow(clippy::large_enum_variant)]
enum RawInput {
    Pointer(u32, [i32; 2], u32),
    Key(u32, u32, [u8; 256], bool),
}
struct HookState {
    sender: SyncSender<Raw>,
    keys: [u8; 256],
    paused: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    overflow: Arc<AtomicUsize>,
}
thread_local! { static HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) }; }

fn own_window(foreground: usize) -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(foreground as HWND, &mut pid);
    }
    pid == 0 || pid == std::process::id()
}

// Only native, visible EDIT/RichEdit controls are eligible for literal text.
// Unknown/custom/IME controls produce an omission marker, never raw key codes.
fn permits_text(focused: usize) -> bool {
    if focused == 0 {
        return false;
    }
    unsafe {
        let mut name = [0u16; 64];
        let count = GetClassNameW(focused as HWND, name.as_mut_ptr(), 64).max(0) as usize;
        let name = String::from_utf16_lossy(&name[..count]).to_ascii_lowercase();
        (name == "edit" || name.starts_with("richedit"))
            && GetWindowLongPtrW(focused as HWND, GWL_STYLE) & ES_PASSWORD as isize == 0
            && IsWindowVisible(focused as HWND) != 0
    }
}

fn send(state: &HookState, input: RawInput) {
    if state.paused.load(Ordering::SeqCst) || state.cancel.load(Ordering::SeqCst) {
        return;
    }
    let foreground = match &input {
        RawInput::Pointer(_, [x, y], _) => unsafe {
            GetAncestor(WindowFromPoint(POINT { x: *x, y: *y }), GA_ROOT) as usize
        },
        _ => (unsafe { GetForegroundWindow() }) as usize,
    };
    if own_window(foreground) {
        return;
    }
    let focused = if unsafe { GetForegroundWindow() } as usize == foreground {
        super::capture::focused_control()
    } else {
        0
    };
    let raw = Raw {
        at: super::now(),
        foreground,
        focused,
        input,
    };
    if state.sender.try_send(raw).is_err() {
        state.overflow.fetch_add(1, Ordering::SeqCst);
        state.cancel.store(true, Ordering::SeqCst);
    }
}

unsafe extern "system" fn mouse(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 && message as u32 != WM_MOUSEMOVE {
            let event = &*(data as *const MSLLHOOKSTRUCT);
            if event.flags & LLMHF_INJECTED == 0 {
                HOOK.with(|slot| {
                    if let Some(state) = slot.borrow().as_ref() {
                        send(
                            state,
                            RawInput::Pointer(
                                message as u32,
                                [event.pt.x, event.pt.y],
                                event.mouseData,
                            ),
                        );
                    }
                });
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, message, data)
    }
}
unsafe extern "system" fn keyboard(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    unsafe {
        if code == HC_ACTION as i32 {
            let event = &*(data as *const KBDLLHOOKSTRUCT);
            if event.flags & LLKHF_INJECTED == 0 && event.vkCode < 256 {
                HOOK.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    let Some(state) = slot.as_mut() else {
                        return;
                    };
                    let down = matches!(message as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
                    let key = event.vkCode as usize;
                    let was_down = state.keys[key] & 128 != 0;
                    if down && !was_down && key == VK_CAPITAL as usize {
                        state.keys[key] ^= 1;
                    }
                    state.keys[key] = (state.keys[key] & 1) | if down { 128 } else { 0 };
                    for (general, left, right) in [
                        (VK_SHIFT, VK_LSHIFT, VK_RSHIFT),
                        (VK_CONTROL, VK_LCONTROL, VK_RCONTROL),
                        (VK_MENU, VK_LMENU, VK_RMENU),
                    ] {
                        state.keys[general as usize] =
                            state.keys[left as usize] | state.keys[right as usize];
                    }
                    if !down {
                        return;
                    }
                    let ctrl = state.keys[VK_CONTROL as usize] & 128 != 0;
                    let shift = state.keys[VK_SHIFT as usize] & 128 != 0;
                    if ctrl && shift && matches!(event.vkCode as u16, VK_F8 | VK_F9) {
                        return;
                    }
                    if matches!(
                        event.vkCode as u16,
                        VK_SHIFT
                            | VK_CONTROL
                            | VK_MENU
                            | VK_LSHIFT
                            | VK_RSHIFT
                            | VK_LCONTROL
                            | VK_RCONTROL
                            | VK_LMENU
                            | VK_RMENU
                            | VK_LWIN
                            | VK_RWIN
                            | VK_CAPITAL
                    ) {
                        return;
                    }
                    let focused = super::capture::focused_control();
                    send(
                        state,
                        RawInput::Key(
                            event.vkCode,
                            event.scanCode,
                            state.keys,
                            permits_text(focused),
                        ),
                    );
                });
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, message, data)
    }
}

unsafe extern "system" fn panel_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_COMMAND || message == WM_CLOSE {
            HOOK.with(|slot| {
                if let Some(state) = slot.borrow().as_ref() {
                    if message == WM_CLOSE || w & 0xffff == 82 {
                        state.cancel.store(true, Ordering::SeqCst);
                    }
                    if message == WM_COMMAND && w & 0xffff == 81 {
                        state.paused.fetch_xor(true, Ordering::SeqCst);
                    }
                }
            });
            return 0;
        }
        if message == WM_MOUSEACTIVATE {
            return MA_NOACTIVATE as isize;
        }
        DefWindowProcW(hwnd, message, w, l)
    }
}
unsafe fn create_panel(german: bool) -> HWND {
    unsafe {
        let instance =
            windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
        let class = super::wide("KlickwerkTrainingPanel");
        let definition = WNDCLASSW {
            lpfnWndProc: Some(panel_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: (windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW + 1) as _,
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        RegisterClassW(&definition);
        let mut work: RECT = std::mem::zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut _ as _, 0);
        let panel = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            super::wide("klickwerk").as_ptr(),
            WS_POPUP | WS_BORDER,
            (work.right - 380).max(work.left),
            (work.bottom - 125).max(work.top),
            360,
            105,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if panel.is_null() {
            return panel;
        }
        for (id, class, title, x, y, w, h) in [
            (
                80,
                "STATIC",
                if german {
                    "Aufnahme läuft"
                } else {
                    "Recording your example"
                },
                14,
                10,
                330,
                22,
            ),
            (
                83,
                "STATIC",
                if german {
                    "Tastenkürzel: Strg + Umschalt + F8 / F9"
                } else {
                    "Shortcuts: Ctrl + Shift + F8 / F9"
                },
                14,
                34,
                330,
                20,
            ),
            (81, "BUTTON", "Pause · F8", 14, 60, 158, 30),
            (
                82,
                "BUTTON",
                if german {
                    "Fertig · F9"
                } else {
                    "Finish · F9"
                },
                184,
                60,
                158,
                30,
            ),
        ] {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                super::wide(class).as_ptr(),
                super::wide(title).as_ptr(),
                WS_CHILD | WS_VISIBLE,
                x,
                y,
                w,
                h,
                panel,
                id as _,
                instance,
                std::ptr::null(),
            );
        }
        ShowWindow(panel, SW_SHOWNOACTIVATE);
        panel
    }
}

fn hook_thread(
    sender: SyncSender<Raw>,
    paused: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    overflow: Arc<AtomicUsize>,
    ready: SyncSender<Result<(), String>>,
    german: bool,
) {
    unsafe {
        let mut keys = [0u8; 256];
        for (key, value) in keys.iter_mut().enumerate() {
            *value = if GetAsyncKeyState(key as i32) < 0 {
                128
            } else {
                0
            };
        }
        keys[VK_CAPITAL as usize] |= (GetKeyState(VK_CAPITAL as i32) & 1) as u8;
        HOOK.with(|s| {
            *s.borrow_mut() = Some(HookState {
                sender,
                keys,
                paused: paused.clone(),
                cancel: cancel.clone(),
                overflow,
            })
        });
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), std::ptr::null_mut(), 0);
        let key_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), std::ptr::null_mut(), 0);
        let hotkey_pause = RegisterHotKey(
            std::ptr::null_mut(),
            81,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            VK_F8 as u32,
        ) != 0;
        let hotkey_stop = RegisterHotKey(
            std::ptr::null_mut(),
            82,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            VK_F9 as u32,
        ) != 0;
        let panel = create_panel(german);
        if mouse_hook.is_null()
            || key_hook.is_null()
            || !hotkey_pause
            || !hotkey_stop
            || panel.is_null()
        {
            let _ = ready.send(Err("The demonstration recorder could not start.".into()));
        } else {
            let _ = ready.send(Ok(()));
            let mut last_paused = false;
            while !cancel.load(Ordering::SeqCst) {
                if paused.load(Ordering::SeqCst) != last_paused {
                    last_paused = paused.load(Ordering::SeqCst);
                    SetWindowTextW(
                        GetDlgItem(panel, 80),
                        super::wide(if last_paused {
                            if german {
                                "Aufnahme pausiert"
                            } else {
                                "Recording paused"
                            }
                        } else if german {
                            "Aufnahme läuft"
                        } else {
                            "Recording your example"
                        })
                        .as_ptr(),
                    );
                    SetWindowTextW(
                        GetDlgItem(panel, 81),
                        super::wide(if last_paused {
                            if german {
                                "Weiter · F8"
                            } else {
                                "Resume · F8"
                            }
                        } else {
                            "Pause · F8"
                        })
                        .as_ptr(),
                    );
                }
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if message.message == WM_HOTKEY {
                        if message.wParam == 82 {
                            cancel.store(true, Ordering::SeqCst);
                        }
                        if message.wParam == 81 {
                            paused.fetch_xor(true, Ordering::SeqCst);
                        }
                    } else {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if hotkey_pause {
            UnregisterHotKey(std::ptr::null_mut(), 81);
        }
        if hotkey_stop {
            UnregisterHotKey(std::ptr::null_mut(), 82);
        }
        if !panel.is_null() {
            DestroyWindow(panel);
        }
        if !mouse_hook.is_null() {
            UnhookWindowsHookEx(mouse_hook);
        }
        if !key_hook.is_null() {
            UnhookWindowsHookEx(key_hook);
        }
        HOOK.with(|s| *s.borrow_mut() = None);
    }
}

fn window(foreground: usize, focused: usize) -> Window {
    let info = super::window::inspect(foreground, std::process::id()).window;
    Window {
        handle: foreground,
        title: info.title,
        application: info.executable,
        class_name: info.class_name,
        bounds: info.bounds,
        focused_control: focused,
    }
}

fn translate(raw: &Raw, options: &Options) -> Option<Input> {
    match &raw.input {
        RawInput::Pointer(message, position, data) => {
            let (phase, button) = match *message {
                WM_LBUTTONDOWN => ("down", "left"),
                WM_LBUTTONUP => ("up", "left"),
                WM_RBUTTONDOWN => ("down", "right"),
                WM_RBUTTONUP => ("up", "right"),
                WM_MBUTTONDOWN => ("down", "middle"),
                WM_MBUTTONUP => ("up", "middle"),
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                    return Some(Input::Scroll {
                        axis: if *message == WM_MOUSEWHEEL {
                            "vertical"
                        } else {
                            "horizontal"
                        }
                        .into(),
                        delta: (*data >> 16) as i16 as i32,
                        position: *position,
                    });
                }
                _ => return None,
            };
            Some(Input::Pointer {
                phase: phase.into(),
                button: button.into(),
                position: *position,
            })
        }
        RawInput::Key(key, scan, keys, allowed_at_event) => {
            let mut modifiers = vec![];
            for (code, name) in [(VK_CONTROL, "ctrl"), (VK_MENU, "alt"), (VK_SHIFT, "shift")] {
                if keys[code as usize] & 128 != 0 {
                    modifiers.push(name.into());
                }
            }
            if keys[VK_LWIN as usize] & 128 != 0 || keys[VK_RWIN as usize] & 128 != 0 {
                modifiers.push("win".into());
            }
            let named = match *key as u16 {
                VK_RETURN => Some("Enter"),
                VK_TAB => Some("Tab"),
                VK_ESCAPE => Some("Escape"),
                VK_BACK => Some("Backspace"),
                VK_DELETE => Some("Delete"),
                VK_LEFT => Some("Left"),
                VK_RIGHT => Some("Right"),
                VK_UP => Some("Up"),
                VK_DOWN => Some("Down"),
                VK_HOME => Some("Home"),
                VK_END => Some("End"),
                VK_PRIOR => Some("PageUp"),
                VK_NEXT => Some("PageDown"),
                _ => None,
            };
            if (VK_F1 as u32..=VK_F24 as u32).contains(key) {
                return Some(Input::Key {
                    key: format!("F{}", *key - VK_F1 as u32 + 1),
                    modifiers,
                });
            }
            if let Some(key) = named {
                return Some(Input::Key {
                    key: key.into(),
                    modifiers,
                });
            }
            let altgr = keys[VK_RMENU as usize] & 128 != 0 && keys[VK_CONTROL as usize] & 128 != 0;
            if !altgr && modifiers.iter().any(|m| m != "shift") {
                let key = if (0x30..=0x5a).contains(key) {
                    char::from_u32(*key).unwrap_or('?').to_string()
                } else if (VK_F1 as u32..=VK_F24 as u32).contains(key) {
                    format!("F{}", *key - VK_F1 as u32 + 1)
                } else {
                    "Other shortcut".into()
                };
                return Some(Input::Key { key, modifiers });
            }
            if !options.text
                || !allowed_at_event
                || !permits_text(raw.focused)
                || unsafe { GetForegroundWindow() } as usize != raw.foreground
                || super::capture::focused_control() != raw.focused
            {
                return Some(Input::OmittedText);
            }
            unsafe {
                let thread = GetWindowThreadProcessId(raw.foreground as HWND, std::ptr::null_mut());
                let mut text = [0u16; 8];
                // Flag 4 prevents changing the user's dead-key composition state.
                let count = ToUnicodeEx(
                    *key,
                    *scan,
                    keys.as_ptr(),
                    text.as_mut_ptr(),
                    8,
                    4,
                    GetKeyboardLayout(thread),
                );
                if count > 0 && count <= 8 {
                    let text = String::from_utf16_lossy(&text[..count as usize]);
                    if !text.chars().any(char::is_control) {
                        return Some(Input::Text { text });
                    }
                }
            }
            Some(Input::OmittedText)
        }
    }
}

fn screenshot(event: usize, elapsed_ms: u64, target: &Window) -> Result<Screenshot, String> {
    if unsafe { GetForegroundWindow() } as usize != target.handle {
        return Err("Focus changed".into());
    }
    // Crop to the demonstrated foreground window, never the entire desktop.
    let (x, y, width, height) = super::capture::layout();
    let [left, top, right, bottom] = target.bounds;
    let bounds = [
        left.max(x),
        top.max(y),
        right.min(x + width as i32),
        bottom.min(y + height as i32),
    ];
    let w = bounds[2] - bounds[0];
    let h = bounds[3] - bounds[1];
    if w <= 0 || h <= 0 || w as u64 * h as u64 > 32_000_000 || !super::default_desktop() {
        return Err("Capture unavailable".into());
    }
    let mut pixels = super::capture::read_pixels(bounds[0], bounds[1], w as u32, h as u32)?;
    unsafe {
        let panel = FindWindowW(
            super::wide("KlickwerkTrainingPanel").as_ptr(),
            std::ptr::null(),
        );
        let mut rect: RECT = std::mem::zeroed();
        if !panel.is_null() && GetWindowRect(panel, &mut rect) != 0 {
            for y in rect.top.max(bounds[1])..rect.bottom.min(bounds[3]) {
                for x in rect.left.max(bounds[0])..rect.right.min(bounds[2]) {
                    pixels.put_pixel(
                        (x - bounds[0]) as u32,
                        (y - bounds[1]) as u32,
                        image::Rgb([240, 240, 240]),
                    );
                }
            }
        }
    }
    if unsafe { GetForegroundWindow() } as usize != target.handle {
        return Err("Focus changed".into());
    }
    let scale = (960.0 / w.max(h) as f64).min(1.0);
    let size = [
        (w as f64 * scale).max(1.0) as u32,
        (h as f64 * scale).max(1.0) as u32,
    ];
    let pixels = image::imageops::resize(
        &pixels,
        size[0],
        size[1],
        image::imageops::FilterType::Triangle,
    );
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 65)
        .encode_image(&pixels)
        .map_err(|_| "Capture unavailable")?;
    Ok(Screenshot {
        after_event_id: event,
        elapsed_ms,
        bounds,
        image_size: size,
        jpeg_base64: base64::engine::general_purpose::STANDARD.encode(jpeg),
    })
}

pub fn record(
    options: Options,
    german: bool,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    mut heartbeat: impl FnMut() -> bool,
    mut update: impl FnMut(training::Summary, bool),
) -> Report {
    let mut report = Report::new(options);
    let (sender, receiver) = mpsc::sync_channel(256);
    let (ready, started) = mpsc::sync_channel(1);
    let overflow = Arc::new(AtomicUsize::new(0));
    let (c, p, o) = (cancel.clone(), paused.clone(), overflow.clone());
    let hook = std::thread::spawn(move || hook_thread(sender, p, c, o, ready, german));
    let result = started.recv_timeout(Duration::from_secs(2));
    if !matches!(result, Ok(Ok(()))) {
        report.stop_reason = "The demonstration recorder could not start.".into();
        cancel.store(true, Ordering::SeqCst);
        let _ = hook.join();
        return report;
    }
    let start = super::now();
    let mut was_paused = false;
    let mut last_window = None;
    let mut last_update = Instant::now();
    let mut pending_image: Option<(u64, usize, Window)> = None;
    let mut last_image = 0;
    while !cancel.load(Ordering::SeqCst) {
        let elapsed = super::now().saturating_sub(start);
        if elapsed >= training::MAX_DURATION_MS {
            report.stop_reason = "The 15-minute recording limit was reached.".into();
            break;
        }
        if !heartbeat() {
            report.stop_reason = "Recording stopped because the interface disconnected.".into();
            break;
        }
        if !super::default_desktop() {
            report.stop_reason = "Recording stopped because the active desktop changed.".into();
            break;
        }
        let is_paused = paused.load(Ordering::SeqCst);
        if is_paused != was_paused {
            if !report.push(
                elapsed,
                None,
                if is_paused {
                    Input::Pause
                } else {
                    Input::Resume
                },
            ) {
                break;
            }
            was_paused = is_paused;
            last_window = None;
            pending_image = None;
            while receiver.try_recv().is_ok() {}
        }
        let foreground = unsafe { GetForegroundWindow() } as usize;
        if !is_paused && !own_window(foreground) {
            let focused = super::capture::focused_control();
            if last_window
                .as_ref()
                .is_none_or(|w: &Window| w.handle != foreground || w.focused_control != focused)
            {
                let target = window(foreground, focused);
                if !report.push(elapsed, Some(target.clone()), Input::Focus) {
                    break;
                }
                last_window = Some(target);
            }
        }
        if let Ok(raw) = receiver.recv_timeout(Duration::from_millis(30)) {
            // Discard queued data after pause, focus changes, or cancellation.
            if is_paused || paused.load(Ordering::SeqCst) || cancel.load(Ordering::SeqCst) {
                continue;
            }
            if let Some(input) = translate(&raw, &report.options) {
                let target = if last_window
                    .as_ref()
                    .is_some_and(|w| w.handle == raw.foreground && w.focused_control == raw.focused)
                {
                    last_window.clone().unwrap()
                } else {
                    window(raw.foreground, raw.focused)
                };
                if !report.push(raw.at.saturating_sub(start), Some(target.clone()), input) {
                    break;
                }
                pending_image = Some((elapsed + 500, report.events.len(), target));
            }
        }
        if report.options.screenshots
            && !is_paused
            && elapsed >= last_image + 1500
            && pending_image
                .as_ref()
                .is_some_and(|(due, _, _)| elapsed >= *due)
        {
            let (_, event, target) = pending_image.take().unwrap();
            // Known password fields are excluded even when screenshots are enabled.
            let focused = super::capture::focused_control();
            let password = focused != 0
                && unsafe { GetWindowLongPtrW(focused as HWND, GWL_STYLE) } & ES_PASSWORD as isize
                    != 0;
            if !password && !cancel.load(Ordering::SeqCst) && !paused.load(Ordering::SeqCst) {
                match screenshot(event, elapsed, &target) {
                    Ok(image)
                        if !cancel.load(Ordering::SeqCst) && !paused.load(Ordering::SeqCst) =>
                    {
                        report.retain_image(image)
                    }
                    _ => report.omitted_screenshots += 1,
                }
            }
            last_image = elapsed;
        }
        report.elapsed_ms = elapsed;
        if last_update.elapsed() >= Duration::from_millis(300) {
            update(report.summary(), is_paused);
            last_update = Instant::now();
        }
    }
    cancel.store(true, Ordering::SeqCst);
    let _ = hook.join();
    report.omitted_events += overflow.load(Ordering::SeqCst);
    if report.stop_reason.is_empty() {
        report.stop_reason = if report.omitted_events > 0 {
            "The recording limit was reached. Review the partial demonstration."
        } else {
            "Demonstration recorded."
        }
        .into();
    }
    report
}

#[cfg(feature = "safety-tests")]
pub fn fixture_checks() -> Result<(), String> {
    // Inspect only disposable controls. No SendInput, clipboard, model requests,
    // or user-file writes are involved in these recording checks.
    unsafe {
        let parent = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            super::wide("STATIC").as_ptr(),
            super::wide("Disposable training fixture").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            -10000,
            -10000,
            100,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if parent.is_null() {
            return Err("Could not create the training fixture.".into());
        }
        let edit = CreateWindowExW(
            0,
            super::wide("EDIT").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            50,
            20,
            parent,
            1 as _,
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        let password = CreateWindowExW(
            0,
            super::wide("EDIT").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | ES_PASSWORD as u32,
            0,
            25,
            50,
            20,
            parent,
            2 as _,
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        let valid = !edit.is_null()
            && !password.is_null()
            && permits_text(edit as usize)
            && !permits_text(password as usize)
            && !permits_text(parent as usize)
            && !permits_text(0);
        DestroyWindow(parent);
        if !valid {
            return Err("Training field privacy checks failed.".into());
        }
        let raw = Raw {
            at: 0,
            foreground: 0,
            focused: 0,
            input: RawInput::Key(0x41, 0, [0; 256], false),
        };
        if !matches!(
            translate(
                &raw,
                &Options {
                    text: true,
                    screenshots: false
                }
            ),
            Some(Input::OmittedText)
        ) {
            return Err("Unknown training fields exposed literal keys.".into());
        }
        let raw = Raw {
            at: 0,
            foreground: 0,
            focused: 0,
            input: RawInput::Pointer(WM_LBUTTONUP, [-240, 500], 0),
        };
        if !matches!(
            translate(&raw, &Options::default()),
            Some(Input::Pointer {
                position: [-240, 500],
                ..
            })
        ) {
            return Err("Training lost physical pointer coordinates.".into());
        }
        // A missing heartbeat ends recording and removes hooks, panel and hotkeys.
        let report = record(
            Options::default(),
            false,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            || false,
            |_, _| {},
        );
        if report.stop_reason != "Recording stopped because the interface disconnected."
            || !report.events.is_empty()
        {
            return Err(format!(
                "Training heartbeat fixture failed: {}",
                report.stop_reason
            ));
        }
        if !FindWindowW(
            super::wide("KlickwerkTrainingPanel").as_ptr(),
            std::ptr::null(),
        )
        .is_null()
        {
            return Err("The training panel survived teardown.".into());
        }
        for (id, key) in [(81, VK_F8), (82, VK_F9)] {
            if RegisterHotKey(
                std::ptr::null_mut(),
                id,
                MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                key as u32,
            ) == 0
            {
                return Err("A training hotkey survived teardown.".into());
            }
            UnregisterHotKey(std::ptr::null_mut(), id);
        }
    }
    let directory = std::env::temp_dir().join(format!(
        "klickwerk-workflow-fixture-{}-{}",
        std::process::id(),
        super::now()
    ));
    std::fs::create_dir(&directory)
        .map_err(|_| "Could not create the temporary workflow fixture.")?;
    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _temporary = Temporary(directory.clone());
    let mut store = crate::workflow::WorkflowStore::load(directory.join("workflows.json"));
    let item = crate::workflow::Workflow {
        id: String::new(),
        learning: crate::workflow::Learning {
            name: "Native note".into(),
            prompt: "Only sample text".into(),
        },
        updated_at: 0,
    };
    let first = store.save(item.clone())?;
    let second = store.save(item)?;
    if first.id != "native-note" || second.id != "native-note-2" {
        return Err("Native workflow publication replaced a filename collision.".into());
    }
    let mut updated = first.clone();
    updated.learning.prompt = "Updated sample text".into();
    store.save(updated)?;
    let loaded = crate::workflow::WorkflowStore::load(directory.join("workflows.json"));
    if loaded.get(&first.id)?.learning.prompt != "Updated sample text"
        || loaded.get(&second.id)?.learning.prompt != "Only sample text"
    {
        return Err("Native workflow persistence lost sample data.".into());
    }
    println!(
        "PASS training privacy, physical coordinates, heartbeat stop, recorder teardown and portable files without desktop input"
    );
    Ok(())
}
