use super::{Handle, wide};
use crate::{
    diagnostics::InputFailure,
    recovery::{self, RestartSession},
};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    os::windows::ffi::OsStrExt,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::Cryptography::*,
    System::{Com::*, Threading::*},
    UI::{Shell::*, WindowsAndMessaging::*},
};

fn failure(code: &str, message: &str, win32_error: Option<u32>) -> InputFailure {
    let mut failure = InputFailure::new(code, message);
    failure.win32_error = win32_error;
    failure
}
fn transfer_failure() -> InputFailure {
    failure("restart_transfer_failed", recovery::RESTART_FAILED, None)
}
fn configure(stream: &TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(10))))
        .map_err(|_| recovery::RESTART_FAILED.into())
}

// Only the native recovery command calls this, after the input broker has exited.
// ShellExecute receives our executable path separately: no shell, task text or credentials.
pub fn launch(owner: usize, session: RestartSession) -> Result<(), InputFailure> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|_| transfer_failure())?;
    listener
        .set_nonblocking(true)
        .map_err(|_| transfer_failure())?;
    let mut random = [0u8; 32];
    if unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            random.as_mut_ptr(),
            random.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    } != 0
    {
        return Err(transfer_failure());
    }
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let port = listener
        .local_addr()
        .map_err(|_| transfer_failure())?
        .port();
    let exe = std::env::current_exe().map_err(|_| transfer_failure())?;
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let directory: Vec<u16> = exe
        .parent()
        .ok_or_else(transfer_failure)?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let parameters = wide(&format!(
        "--elevated-restart {} {port} {token}",
        std::process::id()
    ));
    let verb = wide("runas");
    let process = unsafe {
        let initialized = CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        ) >= 0;
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.hwnd = owner as HWND;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory = directory.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        let ok = ShellExecuteExW(&mut info);
        let error = GetLastError();
        if initialized {
            CoUninitialize();
        }
        if ok == 0 {
            return Err(if error == ERROR_CANCELLED {
                failure(
                    "elevation_cancelled",
                    recovery::RESTART_CANCELLED,
                    Some(error),
                )
            } else {
                failure(
                    "elevation_launch_failed",
                    recovery::RESTART_FAILED,
                    Some(error),
                )
            });
        }
        Handle(info.hProcess)
    };
    if process.0.is_null() {
        return Err(transfer_failure());
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() >= deadline
            || unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(transfer_failure());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                configure(&stream).map_err(|_| transfer_failure())?;
                let mut received = [0u8; 64];
                if stream.read_exact(&mut received).is_err() || received != token.as_bytes() {
                    return Err(transfer_failure());
                }
                recovery::write_transfer(&mut stream, &session).map_err(|_| transfer_failure())?;
                let mut ack = [0u8; 1];
                stream
                    .read_exact(&mut ack)
                    .map_err(|_| transfer_failure())?;
                return if ack == [1] {
                    Ok(())
                } else {
                    Err(transfer_failure())
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(_) => return Err(transfer_failure()),
        }
    }
}

pub fn receive_if_requested() -> Result<Option<RestartSession>, String> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_none_or(|arg| arg != "--elevated-restart") {
        return Ok(None);
    }
    if args.len() != 5 || args[4].len() != 64 || !args[4].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(recovery::RESTART_FAILED.into());
    }
    let parent: u32 = args[2].parse().map_err(|_| recovery::RESTART_FAILED)?;
    let port: u16 = args[3].parse().map_err(|_| recovery::RESTART_FAILED)?;
    let integrity = super::window::privileges(std::process::id())
        .map_err(|_| recovery::RESTART_FAILED)?
        .0;
    if !(0x3000..0x4000).contains(&integrity) {
        return Err(recovery::RESTART_FAILED.into());
    }
    let parent = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, parent) });
    if parent.0.is_null() {
        return Err(recovery::RESTART_FAILED.into());
    }
    let mut stream =
        TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(5))
            .map_err(|_| recovery::RESTART_FAILED)?;
    configure(&stream)?;
    stream
        .write_all(args[4].as_bytes())
        .map_err(|_| recovery::RESTART_FAILED)?;
    let session = recovery::read_transfer(&mut stream)?.restore(integrity)?;
    stream
        .write_all(&[1])
        .map_err(|_| recovery::RESTART_FAILED)?;
    // The old instance exits only after our acknowledgement. Wait before acquiring
    // its single-instance mutex; never run two controllers or race config writes.
    if unsafe { WaitForSingleObject(parent.0, 10000) } != WAIT_OBJECT_0 {
        return Err(recovery::RESTART_FAILED.into());
    }
    Ok(Some(session))
}
