use super::Handle;
use std::{
    path::Path,
    process::{Command, Stdio},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

pub fn launch(path: &Path, token: &str) -> Result<(), String> {
    Command::new(path)
        .arg("--klickwerk-updated")
        .arg(std::process::id().to_string())
        .arg(token)
        .current_dir(path.parent().ok_or(crate::update::INSTALL_FAILED)?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| crate::update::INSTALL_FAILED.into())
}

pub fn receive_if_requested() -> Result<Option<String>, String> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--klickwerk-updated") {
        return Ok(None);
    }
    if args.len() != 4 {
        return Err(crate::update::INSTALL_FAILED.into());
    }
    let pid = args[2]
        .parse::<u32>()
        .ok()
        .filter(|&id| id != 0 && id != std::process::id())
        .ok_or(crate::update::INSTALL_FAILED)?;
    let parent = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) });
    if parent.0.is_null() {
        // A quick parent exit can precede OpenProcess. Other failures must not skip the wait.
        if unsafe { GetLastError() } != ERROR_INVALID_PARAMETER {
            return Err(crate::update::INSTALL_FAILED.into());
        }
    } else if unsafe { WaitForSingleObject(parent.0, 15000) } != WAIT_OBJECT_0 {
        return Err(crate::update::INSTALL_FAILED.into());
    }
    Ok(Some(args[3].clone()))
}
