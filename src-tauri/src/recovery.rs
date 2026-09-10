use crate::{
    diagnostics::InputFailure,
    session::{Evidence, RunView},
    workflow::Step,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const RESTART_READY: &str = "klickwerk restarted as administrator. Click Continue to inspect the current desktop and resume your task.";
pub const RESTART_FAILED: &str = "The administrator restart failed. Your session is still open. You can retry or export its history.";
pub const RESTART_CANCELLED: &str =
    "The administrator restart was cancelled. Your session is still open.";
pub const WAITING_FOR_PERMISSION: &str = "Windows permission is needed to control this app. Approve the Windows prompt to continue the task.";
pub const RESUMING: &str =
    "Administrator access was approved. Resuming your task from the current desktop…";
pub const RESUME_CANCELLED: &str = "Automatic continuation was cancelled. Your task is paused.";
pub const AUTO_SOURCE: &str = "controller_privilege_recovery";
const MAX_TRANSFER: usize = 32 * 1024 * 1024;
const TRANSFER_VERSION: u32 = 2;

// This is a one-time, memory-only transfer, not a saved workflow or a model action.
#[derive(Serialize, Deserialize)]
pub struct RestartSession {
    version: u32,
    pub run: RunView,
    evidence: Evidence,
    pub memory: String,
    pub refinement: String,
    #[serde(default)]
    pub resume_after_approval: bool,
}
impl RestartSession {
    pub fn new(run: RunView, memory: String, refinement: String) -> Result<Self, String> {
        if !can_restart(&run) || refinement.len() > 32768 {
            return Err("This session is not available for an administrator restart.".into());
        }
        let evidence = (*run.evidence).clone();
        Ok(Self {
            version: TRANSFER_VERSION,
            run,
            evidence,
            memory,
            refinement,
            resume_after_approval: false,
        })
    }

    pub fn restore(mut self, sender: u32) -> Result<Self, String> {
        if self.version != TRANSFER_VERSION
            || !is_recoverable_block(&self.run)
            || !(0x3000..0x4000).contains(&sender)
        {
            return Err(RESTART_FAILED.into());
        }
        self.run.evidence = Arc::new(std::mem::take(&mut self.evidence));
        self.run.phase = if self.resume_after_approval {
            "recovering"
        } else {
            "stopped"
        }
        .into();
        self.run.question.clear();
        self.run.message = if self.resume_after_approval {
            RESUMING
        } else {
            RESTART_READY
        }
        .into();
        self.run.recovery = None;
        self.run.recovery_event(
            "elevation_restored",
            if self.resume_after_approval {
                AUTO_SOURCE
            } else {
                "user_restart"
            },
            Some(sender),
            None,
        );
        self.run.steps.push(Step::note(self.run.steps.len() as u32 + 1, "system",
            if self.resume_after_approval {
                "The user approved Windows administrator access to continue the original task. Resume from a NEW screenshot and current native permissions. Do not repeat completed actions or consider old permission refusals current. Complete every remaining requirement of the original task and verify the result before finishing."
            } else { "The user restarted klickwerk with administrator privileges. The session is paused until Continue. On resumption, inspect the NEW screenshot and native input permissions; earlier privilege blocks are historical. Continue the original task from its current state without repeating completed input blindly." }, self.run.elapsed_ms));
        Ok(self)
    }
}

// A task can request the ordinary Windows consent dialog once. Cancellation,
// takeover, unknown permissions and stronger-than-administrator targets stay paused.
pub fn automatic_restart_skip_reason(run: &RunView, cancelled: bool) -> Option<&'static str> {
    if cancelled || run.interrupted {
        return Some("user_cancelled");
    }
    if !can_restart(run) {
        return Some("not_recoverable");
    }
    if run
        .recovery_events
        .iter()
        .any(|event| event.kind == "elevation_requested")
    {
        return Some("already_requested");
    }
    None
}

pub fn automatic_session(
    run: &mut RunView,
    memory: String,
    cancelled: bool,
) -> Result<RestartSession, String> {
    if let Some(reason) = automatic_restart_skip_reason(run, cancelled) {
        return Err(reason.into());
    }
    let mut session = RestartSession::new(run.clone(), memory, String::new())?;
    let sender = run
        .recovery
        .as_ref()
        .and_then(|failure| failure.target.as_ref())
        .and_then(|target| target.sender_integrity_level);
    run.recovery_event("elevation_requested", AUTO_SOURCE, sender, None);
    session.run = run.clone();
    session.resume_after_approval = true;
    run.phase = "recovering".into();
    run.message = WAITING_FOR_PERMISSION.into();
    Ok(session)
}

pub fn can_restart(run: &RunView) -> bool {
    is_recoverable_block(run) && run.recovery_events.len() < 1000
}

fn is_recoverable_block(run: &RunView) -> bool {
    ["waiting", "stopped", "error"].contains(&run.phase.as_str())
        && run
            .recovery
            .as_ref()
            .is_some_and(InputFailure::can_restart_elevated)
        && run.steps.len() < crate::workflow::MAX_STEPS - 3
}

pub fn write_transfer(writer: &mut impl Write, session: &RestartSession) -> Result<(), String> {
    let bytes = serde_json::to_vec(session).map_err(|_| RESTART_FAILED)?;
    if bytes.len() > MAX_TRANSFER {
        return Err(RESTART_FAILED.into());
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .map_err(|_| RESTART_FAILED.into())
}

pub fn read_transfer(reader: &mut impl Read) -> Result<RestartSession, String> {
    let mut size = [0u8; 4];
    reader.read_exact(&mut size).map_err(|_| RESTART_FAILED)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > MAX_TRANSFER {
        return Err(RESTART_FAILED.into());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(|_| RESTART_FAILED)?;
    serde_json::from_slice(&bytes).map_err(|_| RESTART_FAILED.into())
}

// Receipt alone never authorizes continuation. Stop can cancel even while the
// new process is decoding the session; it must also receive the final commit.
pub fn prepare_transfer(
    stream: &mut (impl Read + Write),
    session: &RestartSession,
) -> Result<(), &'static str> {
    write_transfer(stream, session).map_err(|_| "restart_transfer_write_failed")?;
    let mut ack = [0u8; 1];
    stream
        .read_exact(&mut ack)
        .map_err(|_| "restart_ack_failed")?;
    if ack != [1] {
        return Err("restart_ack_failed");
    }
    Ok(())
}

pub fn commit_transfer(stream: &mut impl Write, cancel: &AtomicBool) -> Result<(), &'static str> {
    let proceed = !cancel.load(Ordering::SeqCst);
    stream
        .write_all(&[u8::from(proceed)])
        .map_err(|_| "restart_commit_failed")?;
    if proceed {
        Ok(())
    } else {
        Err("restart_cancelled")
    }
}

pub fn receive_committed_transfer(
    stream: &mut (impl Read + Write),
    integrity: u32,
) -> Result<RestartSession, String> {
    let session = read_transfer(stream)?.restore(integrity)?;
    stream.write_all(&[1]).map_err(|_| RESTART_FAILED)?;
    let mut commit = [0u8; 1];
    stream.read_exact(&mut commit).map_err(|_| RESTART_FAILED)?;
    if commit != [1] {
        return Err(RESUME_CANCELLED.into());
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Settings,
        diagnostics::{TargetInfo, WindowInfo},
    };

    fn blocked() -> RunView {
        RunView {
            id: 7,
            task: "Filter chr; read largest RAM usage".into(),
            phase: "waiting".into(),
            recovery: Some(InputFailure::target(TargetInfo {
                window: WindowInfo {
                    executable: "Taskmgr.exe".into(),
                    integrity_level: Some(0x3000),
                    elevated: Some(true),
                    ..WindowInfo::default()
                },
                sender_integrity_level: Some(0x2000),
                input_block: Some("higher_integrity".into()),
            })),
            ..RunView::default()
        }
    }

    #[test]
    fn confirmed_task_manager_block_requests_consent_once_and_keeps_original_task() {
        let mut run = blocked();
        run.begin_attempt(&Settings::default(), 1, None, "Use the visible filter");
        run.steps
            .push(Step::note(1, "agent", "Opened Task Manager", 100));
        run.block_for_privileges(run.recovery.clone().unwrap(), "model_handoff");
        run.finish_attempt();
        Arc::make_mut(&mut run.evidence).model_responses_observed = 2;
        let attempts = serde_json::to_value(&run.attempts).unwrap();
        let session = automatic_session(&mut run, "Use the visible filter".into(), false).unwrap();
        assert_eq!(run.phase, "recovering");
        assert_eq!(run.recovery_events[1].kind, "elevation_requested");
        assert_eq!(run.recovery_events[1].source, AUTO_SOURCE);
        let mut wire = Vec::new();
        write_transfer(&mut wire, &session).unwrap();
        let restored = read_transfer(&mut wire.as_slice())
            .unwrap()
            .restore(12288)
            .unwrap();
        assert!(restored.resume_after_approval);
        assert_eq!(restored.run.id, 7);
        assert_eq!(restored.run.task, run.task);
        assert_eq!(restored.run.phase, "recovering");
        assert_eq!(restored.run.message, RESUMING);
        assert_eq!(restored.run.steps[0].description, "Opened Task Manager");
        assert_eq!(
            serde_json::to_value(&restored.run.attempts).unwrap(),
            attempts
        );
        assert_eq!(restored.run.evidence.model_responses_observed, 2);
        assert_eq!(restored.memory, "Use the visible filter");
        assert!(restored.run.recovery.is_none());
        let mut resumed = restored.run;
        resumed.phase = "stopped".into();
        let export = serde_json::to_value(
            crate::session::SessionExport::new(resumed, 7, None, String::new(), None).unwrap(),
        )
        .unwrap();
        assert_eq!(
            export["run"]["recovery_events"][2]["kind"],
            "elevation_restored"
        );
        assert_eq!(
            export["run"]["recovery_events"][2]["sender_integrity_level"],
            12288
        );
    }

    #[test]
    fn automatic_restart_never_retries_cancelled_requests_or_overrides_takeover() {
        let mut run = blocked();
        assert!(automatic_session(&mut run, String::new(), true).is_err());
        assert_eq!(run.phase, "waiting");
        assert!(run.recovery_events.is_empty());
        run.interrupted = true;
        assert_eq!(
            automatic_restart_skip_reason(&run, false),
            Some("user_cancelled")
        );
        run.interrupted = false;
        automatic_session(&mut run, String::new(), false).unwrap();
        // A declined UAC prompt leaves the blocked session available for manual retry.
        run.phase = "stopped".into();
        assert!(can_restart(&run));
        assert_eq!(
            automatic_restart_skip_reason(&run, false),
            Some("already_requested")
        );
        assert!(automatic_session(&mut run, String::new(), false).is_err());
        let mut unknown = blocked();
        unknown
            .recovery
            .as_mut()
            .unwrap()
            .target
            .as_mut()
            .unwrap()
            .sender_integrity_level = None;
        assert_eq!(
            automatic_restart_skip_reason(&unknown, false),
            Some("not_recoverable")
        );
        assert_eq!(
            automatic_restart_skip_reason(&RunView::default(), false),
            Some("not_recoverable")
        );
    }

    #[test]
    fn committed_transfer_requires_receipt_and_a_final_uncancelled_commit() {
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;
        for cancellation_stage in ["none", "receipt", "prepared"] {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let mut writer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (mut reader, _) = listener.accept().unwrap();
            for stream in [&reader, &writer] {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let child_cancel = cancel.clone();
            let child = std::thread::spawn(move || {
                if cancellation_stage != "none" {
                    let _session = read_transfer(&mut reader).unwrap().restore(12288).unwrap();
                    if cancellation_stage == "receipt" {
                        child_cancel.store(true, Ordering::SeqCst);
                    }
                    reader.write_all(&[1]).unwrap();
                    let mut commit = [9];
                    reader.read_exact(&mut commit).unwrap();
                    assert_eq!(commit, [0]);
                    None
                } else {
                    Some(receive_committed_transfer(&mut reader, 12288).unwrap())
                }
            });
            let session = automatic_session(&mut blocked(), String::new(), false).unwrap();
            let result = prepare_transfer(&mut writer, &session).and_then(|()| {
                if cancellation_stage == "prepared" {
                    cancel.store(true, Ordering::SeqCst);
                }
                commit_transfer(&mut writer, &cancel)
            });
            let restored = child.join().unwrap();
            if cancellation_stage != "none" {
                assert_eq!(result, Err("restart_cancelled"));
                assert!(restored.is_none());
            } else {
                assert!(result.is_ok());
                let restored = restored.unwrap();
                assert!(restored.resume_after_approval);
                assert_eq!(restored.run.id, 7);
            }
        }
    }

    #[test]
    fn child_cannot_continue_from_an_uncommitted_or_cancelled_transfer() {
        struct Duplex {
            input: std::io::Cursor<Vec<u8>>,
            output: Vec<u8>,
        }
        impl Read for Duplex {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                self.input.read(bytes)
            }
        }
        impl Write for Duplex {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.output.write(bytes)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let session = automatic_session(&mut blocked(), String::new(), false).unwrap();
        for commit in [None, Some(0), Some(2)] {
            let mut wire = Vec::new();
            write_transfer(&mut wire, &session).unwrap();
            wire.extend(commit);
            let mut stream = Duplex {
                input: std::io::Cursor::new(wire),
                output: vec![],
            };
            assert!(receive_committed_transfer(&mut stream, 12288).is_err());
            assert_eq!(stream.output, [1]);
        }
    }

    #[test]
    fn restart_preserves_history_evidence_and_refinement_without_starting_control() {
        let mut run = blocked();
        let settings = Settings {
            api_key: "private-api-key".into(),
            ..Settings::default()
        };
        run.begin_attempt(&settings, 1, None, "Use the filter first");
        run.steps
            .push(Step::note(1, "agent", "Opened Task Manager", 100));
        Arc::make_mut(&mut run.evidence).model_responses_observed = 2;
        run.finish_attempt();
        let mut bytes = Vec::new();
        write_transfer(
            &mut bytes,
            &RestartSession::new(run, "Use the filter first".into(), "Grüße 世界".into()).unwrap(),
        )
        .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private-api-key"));
        let restored = read_transfer(&mut bytes.as_slice())
            .unwrap()
            .restore(0x3000)
            .unwrap();
        assert_eq!(restored.run.id, 7);
        assert_eq!(restored.run.phase, "stopped");
        assert!(restored.run.recovery.is_none());
        assert_eq!(restored.run.steps[0].description, "Opened Task Manager");
        assert_eq!(restored.run.attempts.len(), 1);
        assert_eq!(restored.run.evidence.model_responses_observed, 2);
        assert_eq!(restored.refinement, "Grüße 世界");
        assert_eq!(restored.memory, "Use the filter first");
        assert_eq!(restored.run.recovery_events[0].kind, "elevation_restored");
    }

    #[test]
    fn task_manager_privilege_handoff_replaces_manual_filter_request_and_exports_evidence() {
        let mut run = blocked();
        let failure = run.recovery.clone().unwrap();
        run.question = "Please enter only chr manually; I will sort afterward.".into();
        run.begin_attempt(&Settings::default(), 1, None, "");
        let frame = crate::guard::Frame {
            id: 15,
            captured_ms: 137814562,
            left: 0,
            top: 0,
            width: 2304,
            height: 1536,
            image_width: 960,
            image_height: 640,
            foreground: 1312720,
        };
        Arc::make_mut(&mut run.evidence).retain(&frame, &[1, 2, 3]);
        run.block_for_privileges(failure, "model_handoff");
        run.finish_attempt();
        assert!(can_restart(&run));
        assert!(run.question.is_empty());
        assert_eq!(run.phase, "stopped");
        let before = serde_json::to_value(
            crate::session::SessionExport::new(run.clone(), 7, None, String::new(), None).unwrap(),
        )
        .unwrap();
        assert_eq!(
            before["run"]["recovery"]["target"]["sender_integrity_level"],
            8192
        );
        assert_eq!(
            before["run"]["recovery"]["target"]["window"]["integrity_level"],
            12288
        );
        assert_eq!(
            before["run"]["recovery_events"][0]["source"],
            "model_handoff"
        );
        let mut wire = Vec::new();
        write_transfer(
            &mut wire,
            &RestartSession::new(run, String::new(), String::new()).unwrap(),
        )
        .unwrap();
        let restored = read_transfer(&mut wire.as_slice())
            .unwrap()
            .restore(12288)
            .unwrap();
        let after = serde_json::to_value(
            crate::session::SessionExport::new(restored.run, 7, None, String::new(), None).unwrap(),
        )
        .unwrap();
        assert_eq!(before["evidence"], after["evidence"]);
        assert_eq!(before["run"]["attempts"], after["run"]["attempts"]);
        assert_eq!(
            after["run"]["recovery_events"][1]["sender_integrity_level"],
            12288
        );
        assert!(after["run"]["recovery"].is_null());
    }

    #[test]
    fn restart_requires_a_confirmed_recoverable_privilege_block_and_elevated_child() {
        let mut run = blocked();
        assert!(can_restart(&run));
        run.phase = "running".into();
        assert!(!can_restart(&run));
        run.phase = "waiting".into();
        run.recovery
            .as_mut()
            .unwrap()
            .target
            .as_mut()
            .unwrap()
            .window
            .integrity_level = Some(0x4000);
        assert!(!can_restart(&run));
        assert!(
            RestartSession::new(blocked(), String::new(), String::new())
                .unwrap()
                .restore(0x2000)
                .is_err()
        );
        assert!(RestartSession::new(RunView::default(), String::new(), String::new()).is_err());
        // An older peer exits after receipt alone and cannot implement cancellation
        // before commit. Reject it before sending any acknowledgement.
        let mut old = RestartSession::new(blocked(), String::new(), String::new()).unwrap();
        old.version = 1;
        assert!(old.restore(0x3000).is_err());
    }

    #[test]
    fn transfer_rejects_oversized_truncated_or_invalid_payloads() {
        assert!(read_transfer(&mut ((MAX_TRANSFER + 1) as u32).to_le_bytes().as_slice()).is_err());
        assert!(read_transfer(&mut [10, 0, 0, 0, 123].as_slice()).is_err());
        assert!(read_transfer(&mut [2, 0, 0, 0, 123, 125].as_slice()).is_err());
    }
}
