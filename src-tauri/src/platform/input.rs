use super::{Handle, default_desktop, wide};
use crate::{
    action::{Action, Button, Modifier, key_code},
    guard::Frame,
};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Security::*,
    System::{Memory::*, Threading::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

pub const INPUT_TAG: usize = 0x4b57494e;
#[repr(C)]
struct Held {
    keys: [AtomicU32; 8],
    mouse: AtomicU32,
    unicode: [AtomicU32; 4],
}
pub struct HeldInput {
    mapping: Handle,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}
// The shared allocation contains only aligned atomic integers; the mapping outlives every access.
unsafe impl Send for HeldInput {}
unsafe impl Sync for HeldInput {}

impl HeldInput {
    pub fn open(name: &str, create: bool) -> Result<Self, String> {
        unsafe {
            let name = wide(name);
            let handle = if create {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    std::ptr::null(),
                    PAGE_READWRITE,
                    0,
                    size_of::<Held>() as u32,
                    name.as_ptr(),
                )
            } else {
                OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, name.as_ptr())
            };
            if handle.is_null() {
                return Err("The emergency input cleanup channel could not be created.".into());
            }
            let mapping = Handle(handle);
            let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size_of::<Held>());
            if view.Value.is_null() {
                return Err("The emergency input cleanup channel could not be mapped.".into());
            }
            if create {
                std::ptr::write_bytes(view.Value, 0, size_of::<Held>());
            }
            Ok(Self { mapping, view })
        }
    }
    fn held(&self) -> &Held {
        unsafe { &*(self.view.Value as *const Held) }
    }
    pub fn send(&self, events: &[INPUT]) -> bool {
        // Write-ahead tracking lets the surviving process release partial input after a crash.
        for event in events {
            self.track(event, true);
        }
        let count = unsafe {
            SendInput(
                events.len() as u32,
                events.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
        if count != events.len() as u32 {
            self.release();
            return false;
        }
        for event in events {
            self.track(event, false);
        }
        true
    }
    fn track(&self, event: &INPUT, before: bool) {
        unsafe {
            let held = self.held();
            if event.r#type == INPUT_KEYBOARD {
                let key = event.Anonymous.ki;
                let up = key.dwFlags & KEYEVENTF_KEYUP != 0;
                if key.dwFlags & KEYEVENTF_UNICODE != 0 {
                    if before && !up {
                        for slot in &held.unicode {
                            if slot
                                .compare_exchange(
                                    0,
                                    key.wScan as u32,
                                    Ordering::SeqCst,
                                    Ordering::SeqCst,
                                )
                                .is_ok()
                            {
                                break;
                            }
                        }
                    }
                    if !before && up {
                        for slot in &held.unicode {
                            if slot.load(Ordering::SeqCst) == key.wScan as u32 {
                                slot.store(0, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                } else {
                    let code = key.wVk as usize;
                    if code < 256 {
                        if before && !up {
                            held.keys[code / 32].fetch_or(1 << (code % 32), Ordering::SeqCst);
                        }
                        if !before && up {
                            held.keys[code / 32].fetch_and(!(1 << (code % 32)), Ordering::SeqCst);
                        }
                    }
                }
            } else if event.r#type == INPUT_MOUSE {
                let flags = event.Anonymous.mi.dwFlags;
                for (down, up, bit) in [
                    (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 1),
                    (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 2),
                ] {
                    if before && flags & down != 0 {
                        held.mouse.fetch_or(bit, Ordering::SeqCst);
                    }
                    if !before && flags & up != 0 {
                        held.mouse.fetch_and(!bit, Ordering::SeqCst);
                    }
                }
            }
        }
    }
    pub fn release(&self) {
        let held = self.held();
        let mut events = Vec::new();
        for (index, word) in held.keys.iter().enumerate() {
            let bits = word.load(Ordering::SeqCst);
            for bit in 0..32 {
                if bits & (1 << bit) != 0 {
                    events.push(key_event((index * 32 + bit) as u16, true));
                }
            }
        }
        let mouse = held.mouse.load(Ordering::SeqCst);
        if mouse & 1 != 0 {
            events.push(mouse_event(0, 0, 0, MOUSEEVENTF_LEFTUP));
        }
        if mouse & 2 != 0 {
            events.push(mouse_event(0, 0, 0, MOUSEEVENTF_RIGHTUP));
        }
        for slot in &held.unicode {
            let scan = slot.load(Ordering::SeqCst);
            if scan != 0 {
                events.push(unicode_event(scan as u16, true));
            }
        }
        if !events.is_empty() {
            let sent = unsafe {
                SendInput(
                    events.len() as u32,
                    events.as_ptr(),
                    size_of::<INPUT>() as i32,
                )
            };
            if sent == events.len() as u32 {
                for word in &held.keys {
                    word.store(0, Ordering::SeqCst);
                }
                held.mouse.store(0, Ordering::SeqCst);
                for slot in &held.unicode {
                    slot.store(0, Ordering::SeqCst);
                }
            }
        }
    }
}
impl Drop for HeldInput {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view);
        }
        let _ = &self.mapping;
    }
}

fn key_event(vk: u16, up: bool) -> INPUT {
    let extended = matches!(vk, 0x21..=0x28 | 0x2d | 0x2e);
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: (if up { KEYEVENTF_KEYUP } else { 0 })
                    | (if extended { KEYEVENTF_EXTENDEDKEY } else { 0 }),
                time: 0,
                dwExtraInfo: INPUT_TAG,
            },
        },
    }
}
fn unicode_event(scan: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: scan,
                dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: INPUT_TAG,
            },
        },
    }
}
fn mouse_event(x: i32, y: i32, data: u32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INPUT_TAG,
            },
        },
    }
}
fn movement(frame: &Frame, x: i32, y: i32) -> Result<INPUT, String> {
    let (px, py) = frame.map(x, y).ok_or("The pointer target is invalid.")?;
    Ok(mouse_event(
        ((px - frame.left) as i64 * 65535 / (frame.width.saturating_sub(1).max(1)) as i64) as i32,
        ((py - frame.top) as i64 * 65535 / (frame.height.saturating_sub(1).max(1)) as i64) as i32,
        0,
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
    ))
}

fn allowed_window(window: HWND, parent: u32) -> bool {
    unsafe {
        if window.is_null() {
            return false;
        }
        let root = GetAncestor(window, GA_ROOT);
        let mut pid = 0;
        GetWindowThreadProcessId(root, &mut pid);
        if pid == 0 || pid == parent || pid == GetCurrentProcessId() {
            return false;
        }
        let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            return false;
        }
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let token = Handle(token);
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut length = 0;
        GetTokenInformation(
            token.0,
            TokenElevation,
            &mut elevation as *mut _ as _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        ) != 0
            && elevation.TokenIsElevated == 0
    }
}

fn point_allowed(frame: &Frame, x: i32, y: i32, parent: u32) -> bool {
    let Some((x, y)) = frame.map(x, y) else {
        return false;
    };
    unsafe {
        !MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONULL).is_null()
            && allowed_window(WindowFromPoint(POINT { x, y }), parent)
    }
}

fn path_point_allowed(frame: &Frame, x: i32, y: i32, parent: u32) -> bool {
    let Some((x, y)) = frame.map(x, y) else {
        return false;
    };
    unsafe {
        if MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONULL).is_null() {
            return false;
        }
        let window = WindowFromPoint(POINT { x, y });
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        pid != 0 && pid != parent && pid != GetCurrentProcessId()
    }
}

pub struct Batch {
    pub events: Vec<INPUT>,
    pub delay_ms: u64,
    pub point: Option<(i32, i32)>,
}
pub struct Plan {
    pub batches: VecDeque<Batch>,
    pub frame: Frame,
    pub focus: Option<usize>,
    pub parent: u32,
}
impl Plan {
    pub fn new(frame: Frame, action: Action, parent: u32) -> Result<Self, String> {
        action.validate(frame.image_width, frame.image_height)?;
        if frame.width == 0 || frame.height == 0 || frame.width > 32768 || frame.height > 32768 {
            return Err("Invalid desktop dimensions.".into());
        }
        for (x, y) in action.points() {
            if !point_allowed(&frame, x, y, parent) {
                return Err("The target is protected, elevated, or belongs to klickwerk.".into());
            }
        }
        let focus = if matches!(action, Action::Key { .. } | Action::Text { .. }) {
            Some(frame.foreground)
        } else {
            None
        };
        let mut batches = VecDeque::new();
        match action {
            Action::Click { x, y, button } => {
                let (down, up) = match button {
                    Button::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
                    Button::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                };
                batches.push_back(Batch {
                    events: vec![
                        movement(&frame, x, y)?,
                        mouse_event(0, 0, 0, down),
                        mouse_event(0, 0, 0, up),
                    ],
                    delay_ms: 0,
                    point: Some((x, y)),
                });
            }
            Action::DoubleClick { x, y } => {
                for delay in [0, 60] {
                    batches.push_back(Batch {
                        events: vec![
                            movement(&frame, x, y)?,
                            mouse_event(0, 0, 0, MOUSEEVENTF_LEFTDOWN),
                            mouse_event(0, 0, 0, MOUSEEVENTF_LEFTUP),
                        ],
                        delay_ms: delay,
                        point: Some((x, y)),
                    });
                }
            }
            Action::Move { x, y } => batches.push_back(Batch {
                events: vec![movement(&frame, x, y)?],
                delay_ms: 0,
                point: Some((x, y)),
            }),
            Action::Scroll { x, y, amount } => batches.push_back(Batch {
                events: vec![
                    movement(&frame, x, y)?,
                    mouse_event(0, 0, (amount * 120) as u32, MOUSEEVENTF_WHEEL),
                ],
                delay_ms: 0,
                point: Some((x, y)),
            }),
            Action::Drag {
                x,
                y,
                x2,
                y2,
                duration_ms,
            } => {
                let distance = (x2 - x).abs().max((y2 - y).abs()).max(1);
                for i in 0..=distance {
                    if !path_point_allowed(
                        &frame,
                        x + (x2 - x) * i / distance,
                        y + (y2 - y) * i / distance,
                        parent,
                    ) {
                        return Err("The drag path crosses a protected area.".into());
                    }
                }
                batches.push_back(Batch {
                    events: vec![
                        movement(&frame, x, y)?,
                        mouse_event(0, 0, 0, MOUSEEVENTF_LEFTDOWN),
                    ],
                    delay_ms: 0,
                    point: Some((x, y)),
                });
                let steps = (duration_ms / 15).max(2);
                for i in 1..=steps {
                    let px = x + (x2 - x) * i as i32 / steps as i32;
                    let py = y + (y2 - y) * i as i32 / steps as i32;
                    batches.push_back(Batch {
                        events: vec![movement(&frame, px, py)?],
                        delay_ms: (duration_ms / steps) as u64,
                        point: Some((px, py)),
                    });
                }
                batches.push_back(Batch {
                    events: vec![mouse_event(0, 0, 0, MOUSEEVENTF_LEFTUP)],
                    delay_ms: 0,
                    point: Some((x2, y2)),
                });
            }
            Action::Text { text } => {
                // Each batch is a complete Unicode scalar with balanced down/up events.
                for character in text.chars() {
                    let mut units = [0; 2];
                    let units = character.encode_utf16(&mut units);
                    let mut events = Vec::new();
                    for &unit in units.iter() {
                        events.push(unicode_event(unit, false));
                        events.push(unicode_event(unit, true));
                    }
                    batches.push_back(Batch {
                        events,
                        delay_ms: 0,
                        point: None,
                    });
                }
            }
            Action::Key { key, modifiers } => {
                let keys: Vec<u16> = modifiers
                    .iter()
                    .map(|m| match m {
                        Modifier::Ctrl => VK_CONTROL,
                        Modifier::Alt => VK_MENU,
                        Modifier::Shift => VK_SHIFT,
                    })
                    .collect();
                let vk = key_code(&key).ok_or("Unsupported key.")?;
                let mut events = Vec::new();
                for &m in &keys {
                    events.push(key_event(m, false));
                }
                events.push(key_event(vk, false));
                events.push(key_event(vk, true));
                for &m in keys.iter().rev() {
                    events.push(key_event(m, true));
                }
                batches.push_back(Batch {
                    events,
                    delay_ms: 0,
                    point: None,
                });
            }
            _ => return Err("Only input actions can enter the input broker.".into()),
        }
        Ok(Self {
            batches,
            frame,
            focus,
            parent,
        })
    }
    pub fn target_valid(&self, point: Option<(i32, i32)>) -> bool {
        if !default_desktop()
            || super::capture::layout()
                != (
                    self.frame.left,
                    self.frame.top,
                    self.frame.width,
                    self.frame.height,
                )
        {
            return false;
        }
        if let Some(focus) = self.focus {
            let current = unsafe { GetForegroundWindow() };
            if current as usize != focus || !allowed_window(current, self.parent) {
                return false;
            }
        }
        point.is_none_or(|(x, y)| point_allowed(&self.frame, x, y, self.parent))
    }
}
