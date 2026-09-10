use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{Media::Speech::*, System::Com::*},
    core::{Interface, PCWSTR, PWSTR},
};

#[derive(Clone, Serialize)]
pub struct SpeechUpdate {
    pub text: String,
    pub finished: bool,
    pub error: Option<String>,
}

pub fn dictate(cancel: Arc<AtomicBool>, emit: impl Fn(SpeechUpdate)) {
    let result = unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|_| "The Windows speech service could not initialize.".to_owned())
            .and_then(|_| {
                let result = session(cancel, &emit);
                CoUninitialize();
                result
            })
    };
    emit(SpeechUpdate {
        text: String::new(),
        finished: true,
        error: result.err(),
    });
}

unsafe fn default_token(category: PCWSTR) -> windows::core::Result<ISpObjectToken> {
    unsafe {
        let catalog: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_INPROC_SERVER)?;
        catalog.SetId(category, false)?;
        let id = catalog.GetDefaultTokenId()?;
        let result = (|| {
            let token: ISpObjectToken =
                CoCreateInstance(&SpObjectToken, None, CLSCTX_INPROC_SERVER)?;
            token.SetId(category, PCWSTR(id.0), false)?;
            Ok(token)
        })();
        CoTaskMemFree(Some(id.0 as _));
        result
    }
}

fn session(cancel: Arc<AtomicBool>, emit: &impl Fn(SpeechUpdate)) -> Result<(), String> {
    unsafe {
        let recognizer:ISpRecognizer=CoCreateInstance(&SpInprocRecognizer,None,CLSCTX_INPROC_SERVER)
            .map_err(|_|"Windows speech recognition is unavailable. Install a Windows speech language or type your task.")?;
        let token=default_token(SPCAT_RECOGNIZERS).map_err(|_|"No Windows speech language is configured. Set up speech recognition in Windows Settings, or type your task.")?;
        recognizer
            .SetRecognizer(&token)
            .map_err(|_| "The default Windows speech recognizer could not start.")?;
        let audio = default_token(SPCAT_AUDIOIN)
            .map_err(|_| "No default microphone is available. Check Windows sound settings.")?;
        recognizer.SetInput(&audio, true).map_err(
            |_| "The microphone could not be opened. Check Windows microphone permissions.",
        )?;
        let context = recognizer
            .CreateRecoContext()
            .map_err(|_| "The dictation context could not start.")?;
        context
            .SetNotifyWin32Event()
            .map_err(|_| "Speech notifications could not start.")?;
        let interest = (1u64 << SPEI_RECOGNITION.0)
            | (1u64 << SPEI_HYPOTHESIS.0)
            | (1u64 << SPEI_END_SR_STREAM.0)
            | (1u64 << 30)
            | (1u64 << 33);
        context
            .SetInterest(interest, interest)
            .map_err(|_| "Speech events could not start.")?;
        let grammar = context
            .CreateGrammar(1)
            .map_err(|_| "Dictation grammar is unavailable.")?;
        grammar
            .LoadDictation(PCWSTR::null(), SPLO_STATIC)
            .map_err(|_| "This Windows speech language does not support dictation.")?;
        grammar
            .SetDictationState(SPRS_ACTIVE)
            .map_err(|_| "Dictation could not be activated.")?;
        recognizer
            .SetRecoState(SPRST_ACTIVE_ALWAYS)
            .map_err(|_| "The microphone could not begin listening.")?;
        let start = Instant::now();
        let mut committed = String::new();
        let mut stopping = None;
        loop {
            // Pump the STA so speech COM calls cannot wait behind an unserviced message queue.
            let mut message: windows_sys::Win32::UI::WindowsAndMessaging::MSG = std::mem::zeroed();
            while windows_sys::Win32::UI::WindowsAndMessaging::PeekMessageW(
                &mut message,
                std::ptr::null_mut(),
                0,
                0,
                windows_sys::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            ) != 0
            {
                windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&message);
                windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&message);
            }
            if (cancel.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(120))
                && stopping.is_none()
            {
                let _ = recognizer.SetRecoState(SPRST_INACTIVE);
                stopping = Some(Instant::now());
            }
            let mut event = SPEVENT::default();
            let mut fetched = 0;
            if context.GetEvents(1, &mut event, &mut fetched).is_err() {
                break;
            }
            if fetched > 0 {
                let kind = event._bitfield & 0xffff;
                let parameter_type = (event._bitfield >> 16) & 0xffff;
                if parameter_type == SPET_LPARAM_IS_OBJECT.0 && event.lParam.0 != 0 {
                    let object = windows::core::IUnknown::from_raw(event.lParam.0 as _);
                    if (kind == SPEI_RECOGNITION.0 || kind == SPEI_HYPOTHESIS.0)
                        && let Ok(result) = object.cast::<ISpRecoResult>()
                    {
                        let mut text = PWSTR::null();
                        if result.GetText(0, u32::MAX, true, &mut text, None).is_ok()
                            && !text.is_null()
                        {
                            let value = text.to_string().unwrap_or_default();
                            CoTaskMemFree(Some(text.0 as _));
                            if kind == SPEI_RECOGNITION.0 {
                                if !committed.is_empty() {
                                    committed.push(' ');
                                }
                                committed.push_str(&value);
                            }
                            let display = if kind == SPEI_HYPOTHESIS.0 {
                                format!(
                                    "{}{}{}",
                                    committed,
                                    if committed.is_empty() { "" } else { " " },
                                    value
                                )
                            } else {
                                committed.clone()
                            };
                            if display.len() > 16384 {
                                break;
                            }
                            emit(SpeechUpdate {
                                text: display,
                                finished: false,
                                error: None,
                            });
                        }
                    }
                    // Taking ownership of the event's IUnknown releases the SAPI event reference.
                    drop(object);
                } else if parameter_type == SPET_LPARAM_IS_POINTER.0
                    || parameter_type == SPET_LPARAM_IS_STRING.0
                {
                    CoTaskMemFree(Some(event.lParam.0 as _));
                } else if parameter_type == SPET_LPARAM_IS_TOKEN.0 && event.lParam.0 != 0 {
                    drop(windows::core::IUnknown::from_raw(event.lParam.0 as _));
                }
                if kind == SPEI_END_SR_STREAM.0 {
                    break;
                }
            } else {
                std::thread::sleep(Duration::from_millis(30));
            }
            if stopping.is_some_and(|at| at.elapsed() > Duration::from_millis(400)) {
                break;
            }
        }
        let _ = grammar.SetDictationState(SPRS_INACTIVE);
        let _ = recognizer.SetRecoState(SPRST_INACTIVE_WITH_PURGE);
        emit(SpeechUpdate {
            text: committed,
            finished: true,
            error: None,
        });
        Ok(())
    }
}
