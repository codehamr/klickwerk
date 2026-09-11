use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const RELEASE_TAG: &str = env!("KLICKWERK_RELEASE_TAG");
pub const MANIFEST_URL: &str =
    "https://github.com/codehamr/klickwerk/releases/latest/download/klickwerk-update.json";
pub const MAX_SIZE: u64 = 64 * 1024 * 1024;
pub const CANCELLED: &str = "Update skipped.";
const MANIFEST_LIMIT: usize = 16 * 1024;
const STAGING_PREFIX: &str = ".klickwerk-update-";
const BACKUP_NAME: &str = "previous.exe";
const INVALID: &str = "The update could not be verified. Try again on the next start.";
pub const CHECK_FAILED: &str =
    "Updates could not be checked. You can keep working and try again later.";
pub const INSTALL_FAILED: &str = "The update could not be installed. Close other copies of klickwerk and use a writable folder, then try again on the next start.";

#[derive(Clone, Serialize)]
pub struct Status {
    pub phase: &'static str,
    pub current: String,
    pub latest: Option<String>,
    pub downloaded: u64,
    pub total: u64,
    pub startup: bool,
    pub message: Option<String>,
}
impl Status {
    pub fn new(startup: bool) -> Self {
        Self {
            phase: "checking",
            current: if RELEASE_TAG.is_empty() {
                format!("{} · local", env!("CARGO_PKG_VERSION"))
            } else {
                RELEASE_TAG.into()
            },
            latest: None,
            downloaded: 0,
            total: 0,
            startup,
            message: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub tag: String,
    pub asset: String,
    pub sha256: String,
    pub size: u64,
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MANIFEST_LIMIT {
            return Err(INVALID.into());
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        if value.version != 1
            || !valid_tag(&value.tag)
            || value.asset != "klickwerk.exe"
            || value.sha256.len() != 64
            || !value
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !(256..=MAX_SIZE).contains(&value.size)
        {
            return Err(INVALID.into());
        }
        Ok(value)
    }
    pub fn download_url(&self) -> String {
        format!(
            "https://github.com/codehamr/klickwerk/releases/download/{}/klickwerk.exe",
            self.tag
        )
    }
    pub fn newer_than(&self, current_tag: &str, current_hash: &str) -> bool {
        // Date and time are fixed-width. The run suffix only ensures tag uniqueness.
        self.sha256 != current_hash
            && (!valid_tag(current_tag) || self.tag[..17] > current_tag[..17])
    }
}

fn valid_tag(tag: &str) -> bool {
    let parts: Vec<_> = tag.split('-').collect();
    tag.len() <= 64
        && parts.len() == 6
        && parts[..4].iter().map(|p| p.len()).eq([4, 2, 2, 6])
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && ("01"..="12").contains(&parts[1])
        && ("01"..="31").contains(&parts[2])
        && &parts[3][..2] <= "23"
        && &parts[3][2..4] <= "59"
        && &parts[3][4..] <= "59"
}

pub fn allowed_url(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|port| port == 443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(90))
        .user_agent(concat!("klickwerk/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed_url(attempt.url()) {
                attempt.error("Untrusted update redirect.")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| CHECK_FAILED.into())
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

pub async fn wait_cancel(cancel: &AtomicBool) {
    while !cancel.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub async fn check(client: &reqwest::Client, cancel: &AtomicBool) -> Result<Manifest, String> {
    fetch_manifest(client, MANIFEST_URL, cancel).await
}

async fn fetch_manifest(
    client: &reqwest::Client,
    endpoint: &str,
    cancel: &AtomicBool,
) -> Result<Manifest, String> {
    cancelled(cancel)?;
    let mut response = client
        .get(endpoint)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| CHECK_FAILED)?;
    if response
        .content_length()
        .is_some_and(|size| size > MANIFEST_LIMIT as u64)
    {
        return Err(INVALID.into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| CHECK_FAILED)? {
        cancelled(cancel)?;
        if bytes.len() + chunk.len() > MANIFEST_LIMIT {
            return Err(INVALID.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    cancelled(cancel)?;
    Manifest::parse(&bytes)
}

pub fn file_hash(path: &Path, cancel: &AtomicBool) -> Result<String, String> {
    let mut file = File::open(path).map_err(|_| CHECK_FAILED)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        cancelled(cancel)?;
        let size = file.read(&mut buffer).map_err(|_| CHECK_FAILED)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub struct Staged {
    directory: tempfile::TempDir,
    executable: PathBuf,
}
impl Staged {
    pub fn token(&self) -> &str {
        self.directory.path().file_name().unwrap().to_str().unwrap()
    }
}

pub async fn download(
    client: &reqwest::Client,
    manifest: &Manifest,
    current: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<Staged, String> {
    download_from(
        client,
        &manifest.download_url(),
        manifest,
        current,
        cancel,
        progress,
    )
    .await
}

async fn download_from(
    client: &reqwest::Client,
    endpoint: &str,
    manifest: &Manifest,
    current: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<Staged, String> {
    cancelled(cancel)?;
    let directory = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(current.parent().ok_or(INSTALL_FAILED)?)
        .map_err(|_| INSTALL_FAILED)?;
    let executable = directory.path().join("next.exe");
    let mut file = File::options()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&executable)
        .map_err(|_| INSTALL_FAILED)?;
    let mut response = client
        .get(endpoint)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| CHECK_FAILED)?;
    if response
        .content_length()
        .is_some_and(|size| size != manifest.size)
    {
        return Err(INVALID.into());
    }
    let mut hash = Sha256::new();
    let mut received = 0;
    while let Some(chunk) = response.chunk().await.map_err(|_| CHECK_FAILED)? {
        cancelled(cancel)?;
        received += chunk.len() as u64;
        if received > manifest.size {
            return Err(INVALID.into());
        }
        file.write_all(&chunk).map_err(|_| INSTALL_FAILED)?;
        hash.update(&chunk);
        progress(received);
    }
    cancelled(cancel)?;
    if received != manifest.size || format!("{:x}", hash.finalize()) != manifest.sha256 {
        return Err(INVALID.into());
    }
    verify_executable(&mut file, received)?;
    file.sync_all().map_err(|_| INSTALL_FAILED)?;
    drop(file);
    Ok(Staged {
        directory,
        executable,
    })
}

fn verify_executable(file: &mut File, size: u64) -> Result<(), String> {
    let mut dos = [0u8; 64];
    file.rewind()
        .and_then(|_| file.read_exact(&mut dos))
        .map_err(|_| INVALID)?;
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if &dos[..2] != b"MZ" || offset < 64 || offset + 26 > size {
        return Err(INVALID.into());
    }
    let mut pe = [0u8; 26];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut pe))
        .map_err(|_| INVALID)?;
    let flags = u16::from_le_bytes([pe[22], pe[23]]);
    if &pe[..6] != b"PE\0\0\x64\x86" || pe[24..26] != [0x0b, 0x02] || flags & 0x2002 != 0x0002 {
        return Err(INVALID.into());
    }
    Ok(())
}

/// Keep the original recoverable until the verified replacement has launched.
/// The caller serializes cancellation with this short commit operation.
pub fn install(
    current: &Path,
    staged: Staged,
    launch: impl FnOnce(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    let backup = staged.directory.path().join(BACKUP_NAME);
    fs::rename(current, &backup).map_err(|_| INSTALL_FAILED)?;
    let result = fs::rename(&staged.executable, current)
        .map_err(|_| INSTALL_FAILED.to_string())
        .and_then(|_| launch(current, staged.token()));
    if let Err(error) = result {
        // Moving aside instead of deleting keeps both versions available if rollback fails.
        if current.exists() && fs::rename(current, &staged.executable).is_err() {
            let _ = staged.directory.keep();
            return Err(INSTALL_FAILED.into());
        }
        if fs::rename(&backup, current).is_err() {
            let _ = staged.directory.keep();
            return Err(INSTALL_FAILED.into());
        }
        return Err(error);
    }
    // Windows still maps the original image. The new process cleans up after we exit.
    let _ = staged.directory.keep();
    Ok(())
}

pub fn cleanup(current: &Path, token: &str) {
    if !token.starts_with(STAGING_PREFIX)
        || token.len() > 80
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return;
    }
    let Some(parent) = current.parent() else {
        return;
    };
    let directory = parent.join(token);
    if fs::symlink_metadata(&directory)
        .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
    {
        let _ = fs::remove_file(directory.join(BACKUP_NAME));
        let _ = fs::remove_dir(directory);
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
