fn main() {
    println!("cargo:rerun-if-env-changed=KLICKWERK_RELEASE_TAG");
    let tag = std::env::var("KLICKWERK_RELEASE_TAG").unwrap_or_default();
    assert!(
        tag.len() <= 64 && tag.bytes().all(|b| b.is_ascii_digit() || b == b'-'),
        "Invalid release tag."
    );
    println!("cargo:rustc-env=KLICKWERK_RELEASE_TAG={tag}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        assert!(
            std::env::var("PROFILE").as_deref() != Ok("release") || !tauri_build::is_dev(),
            "Windows release builds must embed the frontend. Use npm run desktop:build or add --features tauri/custom-protocol."
        );
        tauri_build::build();
    }
}
