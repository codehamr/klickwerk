pub mod broker;
pub mod capture;
pub mod export;
pub mod input;
pub mod speech;
pub mod window;

use windows_sys::Win32::{
    Foundation::*,
    System::{StationsAndDesktops::*, SystemInformation::GetTickCount64},
};
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub fn now() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn default_desktop() -> bool {
    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return false;
        }
        let mut name = [0u16; 128];
        let mut needed = 0;
        let ok = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            name.as_mut_ptr() as _,
            size_of_val(&name) as u32,
            &mut needed,
        );
        CloseDesktop(desktop);
        ok != 0
            && String::from_utf16_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(128)])
                .eq_ignore_ascii_case("default")
    }
}

pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
