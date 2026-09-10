fn main() {
    #[cfg(windows)]
    {
        if klickwerk_core::platform::broker::run_if_requested() {
            return;
        }
        if klickwerk_core::desktop::safety_tests::editor_if_requested() {
            return;
        }
        std::process::exit(klickwerk_core::desktop::safety_tests::run());
    }
    #[cfg(not(windows))]
    eprintln!("Run this fixture on an isolated Windows or Wine desktop.");
}
