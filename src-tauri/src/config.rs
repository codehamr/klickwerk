use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub screenshot_max_edge: u32,
    pub request_timeout_seconds: u64,
    pub max_steps: u32,
    pub theme: String,
    pub reduce_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            provider: "local".into(),
            base_url: "http://localhost:11434/v1".into(),
            model: String::new(),
            api_key: String::new(),
            screenshot_max_edge: 1280,
            request_timeout_seconds: 120,
            max_steps: 50,
            theme: "system".into(),
            reduce_motion: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("This config.cfg version is not supported.".into());
        }
        api_base(&self.base_url)?;
        if !["local", "custom"].contains(&self.provider.as_str()) {
            return Err("Choose Local or Custom connection.".into());
        }
        if self.model.len() > 512
            || self.api_key.len() > 8192
            || self.api_key.contains(['\r', '\n'])
        {
            return Err("The model or API key is invalid.".into());
        }
        if ![960, 1280, 1920].contains(&self.screenshot_max_edge)
            || !(10..=300).contains(&self.request_timeout_seconds)
            || !(1..=100).contains(&self.max_steps)
            || !["system", "light", "dark"].contains(&self.theme.as_str())
        {
            return Err("One of the preferences is outside its supported range.".into());
        }
        Ok(())
    }

    pub fn ready(&self) -> Result<(), String> {
        self.validate()?;
        if self.model.trim().is_empty() {
            return Err("Choose a vision model in Settings first.".into());
        }
        Ok(())
    }

    pub fn public(&self) -> Self {
        let mut value = self.clone();
        value.api_key.clear();
        value
    }
}

pub fn api_base(value: &str) -> Result<Url, String> {
    let mut url =
        Url::parse(value.trim()).map_err(|_| "Enter a complete http:// or https:// server URL.")?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Use an HTTP(S) URL without credentials, query parameters, or fragments.".into(),
        );
    }
    let path = url
        .path()
        .trim_end_matches('/')
        .trim_end_matches("/chat/completions");
    let path = if path.is_empty() { "/v1" } else { path };
    url.set_path(&format!("{path}/"));
    Ok(url)
}

pub fn same_origin(a: &str, b: &str) -> bool {
    matches!((api_base(a), api_base(b)), (Ok(a), Ok(b)) if a.origin() == b.origin())
}

pub struct ConfigStore {
    pub path: PathBuf,
    pub settings: Settings,
    pub error: Option<String>,
}

impl ConfigStore {
    pub fn beside_executable() -> Self {
        match std::env::current_exe() {
            Ok(exe) => Self::load(exe.with_file_name("config.cfg")),
            Err(_) => Self {
                path: PathBuf::new(),
                settings: Settings::default(),
                error: Some("The executable folder could not be located.".into()),
            },
        }
    }

    pub fn load(path: PathBuf) -> Self {
        let result = (|| {
            if !path.exists() {
                let settings = Settings::default();
                save_file(&path, &settings)?;
                return Ok(settings);
            }
            if fs::metadata(&path)
                .map_err(|_| "config.cfg could not be read.")?
                .len()
                > 65536
            {
                return Err("config.cfg exceeds 64 KiB.".into());
            }
            let contents =
                fs::read_to_string(&path).map_err(|_| "config.cfg could not be read as UTF-8.")?;
            let mut settings: Settings = toml::from_str(contents.trim_start_matches('\u{feff}'))
                .map_err(|_| {
                    "config.cfg is not valid TOML. The existing file has been preserved."
                        .to_string()
                })?;
            settings.api_key = unprotect(&settings.api_key)?;
            settings.validate()?;
            Ok::<_, String>(settings)
        })();
        match result {
            Ok(settings) => Self {
                path,
                settings,
                error: None,
            },
            Err(error) => Self {
                path,
                settings: Settings::default(),
                error: Some(error),
            },
        }
    }

    pub fn save(&mut self, mut settings: Settings, key: Option<String>) -> Result<(), String> {
        if self.path.as_os_str().is_empty() {
            return Err("The executable folder is unavailable.".into());
        }
        settings.api_key = key.unwrap_or_else(|| {
            if same_origin(&settings.base_url, &self.settings.base_url) {
                self.settings.api_key.clone()
            } else {
                String::new()
            }
        });
        settings.model = settings.model.trim().to_owned();
        settings.base_url = settings.base_url.trim().trim_end_matches('/').to_owned();
        settings.validate()?;
        // Preserve a damaged file before an explicit replacement from Settings.
        if self.error.is_some() && self.path.exists() {
            let backup = self
                .path
                .with_file_name(format!("config.invalid-{}.cfg", std::process::id()));
            fs::copy(&self.path, backup)
                .map_err(|_| "The existing config.cfg could not be backed up.")?;
        }
        save_file(&self.path, &settings)?;
        self.settings = settings;
        self.error = None;
        Ok(())
    }
}

fn save_file(path: &Path, settings: &Settings) -> Result<(), String> {
    let mut stored = settings.clone();
    stored.api_key = protect(&settings.api_key)?;
    let text = format!(
        "# klickwerk settings. Saved beside the application.\n# API keys are protected with Windows DPAPI for the current Windows user.\n{}",
        toml::to_string_pretty(&stored).map_err(|_| "Settings could not be encoded.")?
    );
    let temp = path.with_file_name(format!(
        ".config-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(
                |_| "Cannot write config.cfg beside the EXE. Move klickwerk to a writable folder.",
            )?;
        file.write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|_| "config.cfg could not be saved.")?;
        drop(file);
        replace(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|_| "config.cfg could not be replaced.".into())
}
#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err("config.cfg could not be replaced. Check folder permissions.".into())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn crypt(value: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    let input = CRYPT_INTEGER_BLOB {
        cbData: value.len() as u32,
        pbData: value.as_ptr() as *mut _,
    };
    let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    let success = unsafe {
        if decrypt {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(
            "The API key cannot be unlocked on this Windows account. Re-enter it in Settings."
                .into(),
        );
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData as _);
    }
    Ok(bytes)
}

fn protect(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    #[cfg(windows)]
    {
        use base64::Engine;
        Ok(format!(
            "dpapi:{}",
            base64::engine::general_purpose::STANDARD.encode(crypt(value.as_bytes(), false)?)
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(value.to_owned())
    }
}

fn unprotect(value: &str) -> Result<String, String> {
    if let Some(encoded) = value.strip_prefix("dpapi:") {
        #[cfg(windows)]
        {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|_| "The saved API key is damaged.")?;
            String::from_utf8(crypt(&bytes, true)?)
                .map_err(|_| "The saved API key is damaged.".into())
        }
        #[cfg(not(windows))]
        {
            let _ = encoded;
            Err("This API key requires the Windows account that saved it.".into())
        }
    } else {
        Ok(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creates_and_reloads_beside_selected_executable() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.cfg");
        let mut store = ConfigStore::load(path.clone());
        assert!(path.exists());
        assert!(store.error.is_none());
        let mut settings = store.settings.clone();
        settings.model = "Mixed/Model:Q4".into();
        store.save(settings, Some("secret".into())).unwrap();
        let loaded = ConfigStore::load(path);
        assert_eq!(loaded.settings.model, "Mixed/Model:Q4");
        assert_eq!(loaded.settings.api_key, "secret");
        assert!(loaded.settings.public().api_key.is_empty());
    }
    #[test]
    fn malformed_file_survives_startup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.cfg");
        fs::write(&path, "not = [valid").unwrap();
        assert!(ConfigStore::load(path.clone()).error.is_some());
        assert_eq!(fs::read_to_string(path).unwrap(), "not = [valid");
    }
    #[test]
    fn rejects_invalid_preferences_without_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.cfg");
        let mut store = ConfigStore::load(path.clone());
        let before = fs::read(&path).unwrap();
        let mut settings = store.settings.clone();
        settings.max_steps = 0;
        assert!(store.save(settings, None).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }
    #[test]
    fn origin_change_clears_old_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = ConfigStore::load(temp.path().join("config.cfg"));
        store
            .save(store.settings.clone(), Some("old-key".into()))
            .unwrap();
        let mut next = store.settings.clone();
        next.base_url = "https://example.org/proxy/v1".into();
        store.save(next, None).unwrap();
        assert!(store.settings.api_key.is_empty());
    }
    #[test]
    fn url_rules_keep_proxy_paths() {
        assert_eq!(
            api_base("https://example.org/proxy/v1/chat/completions")
                .unwrap()
                .as_str(),
            "https://example.org/proxy/v1/"
        );
        assert_eq!(
            api_base("http://[::1]:11434").unwrap().as_str(),
            "http://[::1]:11434/v1/"
        );
        for bad in [
            "file:///etc/passwd",
            "https://a:b@example.org",
            "https://example.org/?key=secret",
            "localhost:11434",
        ] {
            assert!(api_base(bad).is_err());
        }
    }
}
