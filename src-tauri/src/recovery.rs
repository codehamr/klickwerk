use crate::{
    diagnostics::InputFailure,
    session::{Evidence, RunView},
    workflow::Step,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    sync::Arc,
};

pub const RESTART_READY: &str = "klickwerk restarted as administrator. Click Continue to inspect the current desktop and resume your task.";
pub const RESTART_FAILED: &str = "The administrator restart failed. Your session is still open. You can retry or export its history.";
pub const RESTART_CANCELLED: &str =
    "The administrator restart was cancelled. Your session is still open.";
const MAX_TRANSFER: usize = 32 * 1024 * 1024;

// This is a one-time, memory-only transfer, not a saved workflow or a model action.
#[derive(Serialize, Deserialize)]
pub struct RestartSession {
    version: u32,
    pub run: RunView,
    evidence: Evidence,
    pub memory: String,
    pub refinement: String,
}
impl RestartSession {
    pub fn new(run: RunView, memory: String, refinement: String) -> Result<Self, String> {
        if !can_restart(&run) || refinement.len() > 32768 {
            return Err("This session is not available for an administrator restart.".into());
        }
        let evidence = (*run.evidence).clone();
        Ok(Self {
            version: 1,
            run,
            evidence,
            memory,
            refinement,
        })
    }

    pub fn restore(mut self, sender: u32) -> Result<Self, String> {
        if self.version != 1
            || !is_recoverable_block(&self.run)
            || !(0x3000..0x4000).contains(&sender)
        {
            return Err(RESTART_FAILED.into());
        }
        self.run.evidence = Arc::new(std::mem::take(&mut self.evidence));
        self.run.phase = "stopped".into();
        self.run.question.clear();
        self.run.message = RESTART_READY.into();
        self.run.recovery = None;
        self.run
            .recovery_event("elevation_restored", "user_restart", Some(sender), None);
        self.run.steps.push(Step::note(self.run.steps.len() as u32 + 1, "system",
            "The user restarted klickwerk with administrator privileges. The session is paused until Continue. On resumption, inspect the NEW screenshot and native input permissions; earlier privilege blocks are historical. Continue the original task from its current state without repeating completed input blindly.", self.run.elapsed_ms));
        Ok(self)
    }
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
    }

    #[test]
    fn transfer_rejects_oversized_truncated_or_invalid_payloads() {
        assert!(read_transfer(&mut ((MAX_TRANSFER + 1) as u32).to_le_bytes().as_slice()).is_err());
        assert!(read_transfer(&mut [10, 0, 0, 0, 123].as_slice()).is_err());
        assert!(read_transfer(&mut [2, 0, 0, 0, 123, 125].as_slice()).is_err());
    }
}
