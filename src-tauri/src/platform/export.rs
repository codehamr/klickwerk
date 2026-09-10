use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::*,
        UI::Shell::{Common::COMDLG_FILTERSPEC, *},
    },
    core::{HRESULT, HSTRING, w},
};

pub fn choose_path(owner: usize, filename: &str, german: bool) -> Result<Option<PathBuf>, String> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|_| "The export dialog could not be opened.")?;
        let result = choose(owner, filename, german);
        CoUninitialize();
        result.map_err(|_| "The export dialog could not be opened. Try again.".into())
    }
}

unsafe fn choose(
    owner: usize,
    filename: &str,
    german: bool,
) -> windows::core::Result<Option<PathBuf>> {
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            dialog.GetOptions()?
                | FOS_FORCEFILESYSTEM
                | FOS_PATHMUSTEXIST
                | FOS_OVERWRITEPROMPT
                | FOS_NOCHANGEDIR,
        )?;
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: w!("JSON (*.json)"),
            pszSpec: w!("*.json"),
        }])?;
        dialog.SetDefaultExtension(w!("json"))?;
        dialog.SetFileName(&HSTRING::from(filename))?;
        dialog.SetTitle(&HSTRING::from(if german {
            "Verlauf als JSON exportieren"
        } else {
            "Export history as JSON"
        }))?;
        if let Err(error) = dialog.Show(Some(HWND(owner as *mut _))) {
            if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error);
        }
        let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let result = PathBuf::from(OsString::from_wide(path.as_wide()));
        CoTaskMemFree(Some(path.0 as _));
        Ok(Some(result))
    }
}
