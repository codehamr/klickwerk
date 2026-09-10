#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        if klickwerk_core::platform::broker::run_if_requested() {
            return;
        }
        klickwerk_core::desktop::run();
    }
    #[cfg(not(windows))]
    eprintln!("Use npm run dev for the browser preview. Desktop control requires Windows 11.");
}
