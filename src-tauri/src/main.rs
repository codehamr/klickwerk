#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        // Capture workers and the input process must use the same physical pixel space.
        unsafe {
            windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
                windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            );
        }
        if klickwerk_core::platform::broker::run_if_requested() {
            return;
        }
        klickwerk_core::desktop::run();
    }
    #[cfg(not(windows))]
    eprintln!("Use npm run dev for the browser preview. Desktop control requires Windows 11.");
}
