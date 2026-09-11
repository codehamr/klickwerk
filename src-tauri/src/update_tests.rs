use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn executable() -> Vec<u8> {
    let mut bytes = vec![0; 512];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
    bytes[86] = 2;
    bytes[88..90].copy_from_slice(&[0x0b, 0x02]);
    bytes
}
fn manifest(bytes: &[u8]) -> Manifest {
    Manifest {
        version: 1,
        tag: "2026-09-11-123456-12-1".into(),
        asset: "klickwerk.exe".into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}
async fn serve(bytes: Vec<u8>, chunked: bool) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/fixture", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let _ = stream.read(&mut request).await;
        let response = if chunked {
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
                bytes.len()
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
        };
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.write_all(&bytes).await;
        if chunked {
            let _ = stream.write_all(b"\r\n0\r\n\r\n").await;
        }
    });
    endpoint
}
fn staged(parent: &Path, bytes: &[u8]) -> Staged {
    let directory = tempfile::Builder::new()
        .prefix(".klickwerk-update-")
        .tempdir_in(parent)
        .unwrap();
    let executable = directory.path().join("next.exe");
    fs::write(&executable, bytes).unwrap();
    Staged {
        directory,
        executable,
    }
}

#[test]
fn only_versioned_pinned_bounded_manifests_are_accepted() {
    let valid = serde_json::to_value(manifest(&executable())).unwrap();
    assert!(Manifest::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for (field, value) in [
        ("version", serde_json::json!(2)),
        ("asset", serde_json::json!("../other.exe")),
        ("tag", serde_json::json!("../../main")),
        ("tag", serde_json::json!("2026-13-11-123456-1-1")),
        ("tag", serde_json::json!("2026-09-11-256060-1-1")),
        ("sha256", serde_json::json!("a".repeat(63))),
        ("sha256", serde_json::json!("A".repeat(64))),
        ("size", serde_json::json!(MAX_SIZE + 1)),
        ("size", serde_json::json!(0)),
        ("url", serde_json::json!("https://example.com/payload")),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        assert!(
            Manifest::parse(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{field}"
        );
    }
    assert!(Manifest::parse(&vec![b' '; MANIFEST_LIMIT + 1]).is_err());
}

#[test]
fn exact_hashes_and_release_order_prevent_repeated_updates_and_downgrades() {
    let m = manifest(&executable());
    assert!(!m.newer_than("", &m.sha256));
    assert!(!m.newer_than(&m.tag, "different"));
    assert!(!m.newer_than("2026-09-12-000000-1-1", "different"));
    assert!(m.newer_than("2026-09-11-123455-999-1", "different"));
    assert!(m.newer_than("", "local build"));
    assert_eq!(
        m.download_url(),
        "https://github.com/codehamr/klickwerk/releases/download/2026-09-11-123456-12-1/klickwerk.exe"
    );
}

#[test]
fn redirects_require_github_https_without_credentials() {
    for url in [
        "https://github.com/codehamr/klickwerk",
        "https://release-assets.githubusercontent.com/github-production-release-asset/1?signature=fixture",
        "https://objects.githubusercontent.com/asset",
    ] {
        assert!(allowed_url(&url.parse().unwrap()));
    }
    for url in [
        "http://github.com/file",
        "https://github.com.evil.test/file",
        "https://evil.test/github.com",
        "https://user:pass@github.com/file",
        "https://github.com:444/file",
    ] {
        assert!(!allowed_url(&url.parse().unwrap()));
    }
}

#[tokio::test]
async fn manifest_reads_are_bounded_even_without_content_length() {
    let cancel = AtomicBool::new(false);
    let expected = manifest(&executable());
    let url = serve(serde_json::to_vec(&expected).unwrap(), true).await;
    assert_eq!(
        fetch_manifest(&http_client(), &url, &cancel)
            .await
            .unwrap()
            .sha256,
        expected.sha256
    );
    for chunked in [true, false] {
        let url = serve(vec![b' '; MANIFEST_LIMIT + 1], chunked).await;
        assert!(fetch_manifest(&http_client(), &url, &cancel).await.is_err());
    }
}

#[tokio::test]
async fn download_verifies_size_hash_and_windows_architecture_before_installing() {
    let folder = tempfile::tempdir().unwrap();
    let current = folder.path().join("Renamed assistant.exe");
    fs::write(&current, b"original").unwrap();
    let cancel = AtomicBool::new(false);
    let bytes = executable();
    let m = manifest(&bytes);
    let url = serve(bytes.clone(), true).await;
    let staged = download_from(&http_client(), &url, &m, &current, &cancel, |_| {})
        .await
        .unwrap();
    assert_eq!(fs::read(&staged.executable).unwrap(), bytes);
    assert_eq!(fs::read(&current).unwrap(), b"original");
    drop(staged);
    let mut wrong_arch = bytes.clone();
    wrong_arch[68] = 0x4c;
    for (payload, expected) in [
        (b"truncated".to_vec(), m.clone()),
        (vec![0; bytes.len()], m.clone()),
        (vec![0; bytes.len()], manifest(&vec![0; bytes.len()])),
        (wrong_arch.clone(), manifest(&wrong_arch)),
        (vec![0; bytes.len() + 1], m.clone()),
    ] {
        let url = serve(payload, true).await;
        assert!(
            download_from(&http_client(), &url, &expected, &current, &cancel, |_| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(&current).unwrap(), b"original");
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }
}

#[tokio::test]
async fn cancellation_removes_the_download_and_preserves_the_original() {
    let folder = tempfile::tempdir().unwrap();
    let current = folder.path().join("portable.exe");
    fs::write(&current, b"original").unwrap();
    let bytes = executable();
    let url = serve(bytes.clone(), false).await;
    let cancel = AtomicBool::new(false);
    let result = download_from(
        &http_client(),
        &url,
        &manifest(&bytes),
        &current,
        &cancel,
        |_| cancel.store(true, Ordering::SeqCst),
    )
    .await;
    assert!(matches!(result, Err(error) if error == CANCELLED));
    assert_eq!(fs::read(&current).unwrap(), b"original");
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    assert!(matches!(check(&http_client(), &cancel).await, Err(error) if error == CANCELLED));
}

#[test]
fn failed_promote_or_launch_restores_the_previous_executable() {
    for missing_download in [false, true] {
        let folder = tempfile::tempdir().unwrap();
        let current = folder.path().join("My portable app.exe");
        fs::write(&current, b"original").unwrap();
        let staged = staged(folder.path(), b"verified new image");
        if missing_download {
            fs::remove_file(&staged.executable).unwrap();
        }
        assert!(
            install(&current, staged, |_, _| Err(
                "Fixture launch failure.".into()
            ))
            .is_err()
        );
        assert_eq!(fs::read(&current).unwrap(), b"original");
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }
}

#[test]
fn successful_install_preserves_portable_data_and_cleans_only_its_own_backup() {
    let folder = tempfile::tempdir().unwrap();
    let current = folder.path().join("My portable app.exe");
    fs::write(&current, b"original").unwrap();
    fs::write(folder.path().join("config.cfg"), b"private fixture").unwrap();
    fs::create_dir(folder.path().join("workflows")).unwrap();
    fs::write(
        folder.path().join("workflows/daily-task.json"),
        b"saved fixture",
    )
    .unwrap();
    let staged = staged(folder.path(), b"verified new image");
    let token = staged.token().to_string();
    install(&current, staged, |path, received| {
        assert_eq!(path, current);
        assert_eq!(received, token);
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(&current).unwrap(), b"verified new image");
    assert_eq!(
        fs::read(folder.path().join(&token).join("previous.exe")).unwrap(),
        b"original"
    );
    cleanup(&current, "../workflows");
    cleanup(&current, &token);
    assert!(!folder.path().join(token).exists());
    assert_eq!(
        fs::read(folder.path().join("config.cfg")).unwrap(),
        b"private fixture"
    );
    assert_eq!(
        fs::read(folder.path().join("workflows/daily-task.json")).unwrap(),
        b"saved fixture"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "Child process fixture; invoked only by the running-image test."]
fn running_image_fixture() {
    let path = std::env::var_os("KLICKWERK_UPDATE_FIXTURE").expect("Fixture folder is required.");
    let path = PathBuf::from(path);
    fs::write(path.join("ready"), b"ready").unwrap();
    let start = std::time::Instant::now();
    while !path.join("finish").exists() && start.elapsed() < Duration::from_secs(15) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(windows)]
#[test]
fn windows_can_replace_a_running_renamed_executable_without_desktop_input() {
    use std::process::{Command, Stdio};
    let folder = tempfile::tempdir().unwrap();
    let current = folder.path().join("Portable name with spaces.exe");
    let image = fs::read(std::env::current_exe().unwrap()).unwrap();
    fs::write(&current, &image).unwrap();
    let mut child = Command::new(&current)
        .args([
            "--ignored",
            "--exact",
            "update::tests::running_image_fixture",
        ])
        .env("KLICKWERK_UPDATE_FIXTURE", folder.path())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !folder.path().join("ready").exists() && start.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(folder.path().join("ready").exists());
    let staged = staged(folder.path(), &image);
    let token = staged.token().to_string();
    let result = install(&current, staged, |path, _| {
        Command::new(path)
            .arg("--list")
            .stdout(Stdio::null())
            .status()
            .map_err(|error| error.to_string())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err("Fixture child failed.".into())
                }
            })
    });
    fs::write(folder.path().join("finish"), b"finish").unwrap();
    assert!(child.wait().unwrap().success());
    result.unwrap();
    cleanup(&current, &token);
    assert_eq!(fs::read(&current).unwrap(), image);
}
