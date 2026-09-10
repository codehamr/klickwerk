fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        assert!(
            std::env::var("PROFILE").as_deref() != Ok("release") || !tauri_build::is_dev(),
            "Windows release builds must embed the frontend. Use npm run desktop:build or add --features tauri/custom-protocol."
        );
        tauri_build::build();
    }
}
