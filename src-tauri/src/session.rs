use crate::{
    config::Settings,
    workflow::{Step, Workflow},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExecutionSettings {
    base_url: String,
    model: String,
    screenshot_max_edge: u32,
    request_timeout_seconds: u64,
    max_steps: u32,
    language: String,
}
impl From<&Settings> for ExecutionSettings {
    fn from(settings: &Settings) -> Self {
        Self {
            base_url: settings.base_url.clone(),
            model: settings.model.clone(),
            screenshot_max_edge: settings.screenshot_max_edge,
            request_timeout_seconds: settings.request_timeout_seconds,
            max_steps: settings.max_steps,
            language: settings.ui_language().into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Attempt {
    number: usize,
    started_at: u64,
    finished_at: Option<u64>,
    first_step_id: u32,
    last_step_id: Option<u32>,
    user_reply: Option<String>,
    warm_start_prompt: String,
    phase: String,
    message: String,
    result: String,
    question: String,
    settings: ExecutionSettings,
    handoff: Option<crate::diagnostics::Handoff>,
    terminal_desktop: Option<crate::diagnostics::TerminalDesktop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    interruption: Option<crate::diagnostics::InputInterruption>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RunView {
    pub id: u64,
    pub phase: String,
    pub task: String,
    pub message: String,
    pub steps: Vec<Step>,
    pub result: String,
    pub question: String,
    pub elapsed_ms: u64,
    pub interrupted: bool,
    pub workflow_id: Option<String>,
    pub started_at: u64,
    pub attempts: Vec<Attempt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training: Option<crate::training::Summary>,
    #[serde(default)]
    pub recovery: Option<crate::diagnostics::InputFailure>,
    #[serde(default)]
    pub recovery_events: Vec<crate::diagnostics::RecoveryEvent>,
    #[serde(skip)]
    pub evidence: Arc<Evidence>,
}
impl Default for RunView {
    fn default() -> Self {
        Self {
            id: 0,
            phase: "idle".into(),
            task: String::new(),
            message: String::new(),
            steps: vec![],
            result: String::new(),
            question: String::new(),
            elapsed_ms: 0,
            interrupted: false,
            workflow_id: None,
            started_at: 0,
            attempts: vec![],
            training: None,
            recovery: None,
            recovery_events: vec![],
            evidence: Arc::default(),
        }
    }
}
impl RunView {
    pub fn block_for_privileges(
        &mut self,
        failure: crate::diagnostics::InputFailure,
        source: &str,
    ) {
        self.phase = "stopped".into();
        self.question.clear();
        self.message = failure.message.clone();
        let sender = failure
            .target
            .as_ref()
            .and_then(|t| t.sender_integrity_level);
        self.recovery_event("privilege_blocked", source, sender, Some(failure.clone()));
        self.recovery = Some(failure);
    }

    pub fn recovery_event(
        &mut self,
        kind: &str,
        source: &str,
        sender: Option<u32>,
        failure: Option<crate::diagnostics::InputFailure>,
    ) {
        self.recovery_events
            .push(crate::diagnostics::RecoveryEvent {
                recorded_at: timestamp(),
                kind: kind.into(),
                source: source.into(),
                sender_integrity_level: sender,
                failure,
            });
    }

    pub fn begin_attempt(
        &mut self,
        settings: &Settings,
        first_step_id: u32,
        reply: Option<String>,
        memory: &str,
    ) {
        let now = timestamp();
        if self.started_at == 0 {
            self.started_at = now;
        }
        self.attempts.push(Attempt {
            number: self.attempts.len() + 1,
            started_at: now,
            finished_at: None,
            first_step_id,
            last_step_id: None,
            user_reply: reply,
            warm_start_prompt: memory.into(),
            phase: "running".into(),
            message: String::new(),
            result: String::new(),
            question: String::new(),
            settings: settings.into(),
            handoff: None,
            terminal_desktop: None,
            interruption: None,
        });
    }

    pub fn record_handoff(&mut self, handoff: crate::diagnostics::Handoff) {
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.handoff = Some(handoff);
        }
    }

    pub fn record_terminal_desktop(&mut self, desktop: crate::diagnostics::TerminalDesktop) {
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.terminal_desktop = Some(desktop);
        }
    }

    pub fn record_interruption(&mut self, interruption: crate::diagnostics::InputInterruption) {
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.interruption = Some(interruption);
        }
    }

    pub fn finish_attempt(&mut self) {
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.finished_at = Some(timestamp());
            attempt.last_step_id = self
                .steps
                .last()
                .filter(|s| s.id >= attempt.first_step_id)
                .map(|s| s.id);
            attempt.phase = self.phase.clone();
            attempt.message = self.message.clone();
            attempt.result = self.result.clone();
            attempt.question = self.question.clone();
        }
    }
}

// Keep image evidence out of UI snapshots and workflow/model history. The shared
// allocation also avoids copying several MiB on every progress event.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training: Option<crate::training::Report>,
    pub frames_observed: usize,
    pub frames_omitted: usize,
    pub frames: Vec<ScreenEvidence>,
    pub model_responses_observed: usize,
    pub model_responses_omitted: usize,
    pub model_responses: Vec<crate::diagnostics::ModelResponse>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ScreenEvidence {
    pub frame: crate::guard::Frame,
    pub observed_at: u64,
    pub purpose: String,
    pub mime: String,
    pub jpeg_base64: String,
}
impl Evidence {
    pub const MAX_FRAMES: usize = 12;
    pub const MAX_BYTES: usize = 8 * 1024 * 1024;

    pub fn retain_model(&mut self, response: crate::diagnostics::ModelResponse) {
        self.model_responses_observed += 1;
        if self.model_responses.len() >= Self::MAX_FRAMES {
            self.model_responses.remove(0);
            self.model_responses_omitted += 1;
        }
        self.model_responses.push(response);
    }

    pub fn retain(&mut self, frame: &crate::guard::Frame, jpeg: &[u8]) {
        self.retain_for(frame, jpeg, "model_observation");
    }

    pub fn retain_for(&mut self, frame: &crate::guard::Frame, jpeg: &[u8], purpose: &str) {
        use base64::Engine;
        self.frames_observed += 1;
        if jpeg.len().div_ceil(3) * 4 > Self::MAX_BYTES {
            self.frames_omitted += 1;
            return;
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
        while self.frames.len() >= Self::MAX_FRAMES
            || self
                .frames
                .iter()
                .map(|f| f.jpeg_base64.len())
                .sum::<usize>()
                + encoded.len()
                > Self::MAX_BYTES
        {
            self.frames.remove(0);
            self.frames_omitted += 1;
        }
        self.frames.push(ScreenEvidence {
            frame: frame.clone(),
            observed_at: timestamp(),
            purpose: purpose.into(),
            mime: "image/jpeg".into(),
            jpeg_base64: encoded,
        });
    }
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    platform: &'static str,
}
#[derive(Serialize)]
struct Coverage {
    history: &'static str,
    screenshots: &'static str,
    raw_model_responses: &'static str,
    controller_revision: &'static str,
    capture_backend: &'static str,
    clock: &'static str,
}
#[derive(Serialize)]
pub struct SessionExport {
    schema_version: u32,
    exported_at: u64,
    app: AppInfo,
    run: RunView,
    workflow: Option<Workflow>,
    warm_start_prompt: String,
    unsent_refinement: Option<String>,
    coverage: Coverage,
    evidence: Evidence,
}
impl SessionExport {
    pub fn new(
        run: RunView,
        run_id: u64,
        workflow: Option<Workflow>,
        memory: String,
        refinement: Option<String>,
    ) -> Result<Self, String> {
        if run.id != run_id
            || !["done", "error", "stopped", "waiting"].contains(&run.phase.as_str())
        {
            return Err("This session is no longer available to export.".into());
        }
        if refinement.as_ref().is_some_and(|text| text.len() > 16384) {
            return Err("The refinement is too long to export.".into());
        }
        Ok(Self {
            schema_version: 3,
            evidence: (*run.evidence).clone(),
            exported_at: timestamp(),
            app: AppInfo {
                name: "klickwerk",
                version: env!("CARGO_PKG_VERSION"),
                platform: "windows",
            },
            run,
            workflow,
            warm_start_prompt: memory,
            unsent_refinement: refinement.filter(|text| !text.trim().is_empty()),
            coverage: Coverage {
                history: "all_recorded_steps_and_attempts",
                screenshots: "recent_frames_bounded_12_and_8_mib_base64",
                raw_model_responses: "recent_assistant_text_bounded_12_and_32_kib_each",
                controller_revision: crate::provider::CONTROLLER_REVISION,
                capture_backend: "gdi_bitblt_physical_virtual_desktop",
                clock: "captured_ms_and_input_completed_ms_are_monotonic_other_timestamps_are_unix_ms",
            },
        })
    }

    pub fn filename(&self) -> String {
        format!(
            "klickwerk-session-{}-{}.json",
            self.run.id, self.exported_at
        )
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|_| "The session could not be encoded as JSON.")?;
        bytes.push(b'\n');
        write_report(path, &bytes)
    }
}

// Replace only after the complete report reaches disk; keep an existing export on failure.
pub fn write_report(path: &Path, bytes: &[u8]) -> Result<(), String> {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("Choose a folder for the export.")?;
    let temporary = parent.join(format!(
        ".klickwerk-export-{}-{}.tmp",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::SeqCst)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(
            |_| "The history could not be saved. Check the folder permissions and available space.",
        )?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.map_err(|_| "The history could not be saved. Check the folder permissions and available space.".to_owned())
        .and_then(|()| crate::config::replace(&temporary, path).map_err(|_| "The history file could not be replaced. Choose another file or check its permissions.".to_owned()));
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::add_correction;

    #[test]
    fn demonstration_text_stays_out_of_ui_snapshots_but_is_explicitly_exportable() {
        let mut run = RunView {
            id: 9,
            phase: "stopped".into(),
            ..RunView::default()
        };
        let mut training = crate::training::Report::default();
        training.push(
            10,
            None,
            crate::training::Input::Text {
                text: "sample-private-demonstration".into(),
            },
        );
        run.training = Some(training.summary());
        Arc::make_mut(&mut run.evidence).training = Some(training);
        let snapshot = serde_json::to_string(&run).unwrap();
        assert!(!snapshot.contains("sample-private-demonstration"));
        assert!(snapshot.contains("\"events\":1"));
        let export = SessionExport::new(run, 9, None, String::new(), None).unwrap();
        let report = serde_json::to_value(export).unwrap();
        assert_eq!(
            report["evidence"]["training"]["events"][0]["text"],
            "sample-private-demonstration"
        );
    }

    #[test]
    fn diagnostic_frames_are_bounded_exported_and_excluded_from_ui_snapshots() {
        let frame = crate::guard::Frame {
            id: 2,
            captured_ms: 4000,
            left: 0,
            top: 0,
            width: 2304,
            height: 1536,
            image_width: 960,
            image_height: 640,
            foreground: 123,
        };
        // Reproduce the coordinates in the reported Task Manager failure.
        assert_eq!(frame.map(716, 13), Some((1719, 32)));
        let mut run = RunView {
            id: 1,
            phase: "stopped".into(),
            ..RunView::default()
        };
        for id in 1..=15 {
            let mut next = frame.clone();
            next.id = id;
            Arc::make_mut(&mut run.evidence).retain(&next, b"fixture-jpeg");
        }
        assert_eq!(run.evidence.frames.len(), 12);
        assert_eq!(run.evidence.frames_omitted, 3);
        assert_eq!(run.evidence.frames[0].frame.id, 4);
        let snapshot = serde_json::to_value(&run).unwrap();
        assert!(snapshot.get("evidence").is_none());
        let export = SessionExport::new(run.clone(), 1, None, String::new(), None).unwrap();
        let json = serde_json::to_value(export).unwrap();
        assert_eq!(json["schema_version"], 3);
        assert_eq!(json["evidence"]["frames_observed"], 15);
        assert_eq!(json["evidence"]["frames"][11]["frame"]["id"], 15);
        assert!(
            json["evidence"]["frames"][0]["jpeg_base64"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        );
        Arc::make_mut(&mut run.evidence).retain(&frame, &vec![0; Evidence::MAX_BYTES]);
        assert_eq!(run.evidence.frames_omitted, 4);
        Arc::make_mut(&mut run.evidence).retain(&frame, &vec![0; 5 * 1024 * 1024]);
        Arc::make_mut(&mut run.evidence).retain(&frame, &vec![0; 5 * 1024 * 1024]);
        assert_eq!(run.evidence.frames.len(), 1);
        assert!(run.evidence.frames[0].jpeg_base64.len() <= Evidence::MAX_BYTES);
    }

    #[test]
    fn model_diagnostics_and_terminal_evidence_survive_export_with_explicit_coverage() {
        let mut run = RunView {
            id: 1,
            phase: "waiting".into(),
            ..RunView::default()
        };
        run.begin_attempt(&Settings::default(), 1, None, "");
        for frame_id in 1..=15 {
            Arc::make_mut(&mut run.evidence).retain_model(crate::diagnostics::ModelResponse {
                frame_id,
                received_at: 1000,
                response_model: Some("fixture".into()),
                finish_reason: Some("stop".into()),
                assistant_content: Some("fixture-response".into()),
                content_truncated: false,
                parse_error: None,
            });
        }
        run.record_interruption(crate::diagnostics::InputInterruption {
            event: 512,
            flags: 1,
            detected_ms: 5000,
            since_agent_input_ms: Some(31),
            position: Some([2343, 196]),
            anchor: Some([2242, 196]),
        });
        run.record_terminal_desktop(crate::diagnostics::TerminalDesktop {
            frame_id: None,
            capture_error: Some("Fixture capture unavailable.".into()),
            captured_ms: 5000,
            foreground: crate::diagnostics::TargetInfo {
                window: crate::diagnostics::WindowInfo {
                    handle: 456,
                    executable: "Taskmgr.exe".into(),
                    ..Default::default()
                },
                sender_integrity_level: Some(8192),
                input_block: None,
            },
            focused_control: 789,
        });
        assert!(
            serde_json::to_value(&run)
                .unwrap()
                .get("evidence")
                .is_none()
        );
        let json =
            serde_json::to_value(SessionExport::new(run, 1, None, String::new(), None).unwrap())
                .unwrap();
        assert_eq!(json["evidence"]["model_responses_observed"], 15);
        let interruption = &json["run"]["attempts"][0]["interruption"];
        assert_eq!(interruption["event"], 512);
        assert_eq!(interruption["since_agent_input_ms"], 31);
        assert_eq!(interruption["anchor"], serde_json::json!([2242, 196]));
        let restored: RunView = serde_json::from_value(json["run"].clone()).unwrap();
        assert!(restored.attempts[0].interruption.is_some());
        let mut legacy = json["run"].clone();
        legacy["attempts"][0]
            .as_object_mut()
            .unwrap()
            .remove("interruption");
        assert!(
            serde_json::from_value::<RunView>(legacy).unwrap().attempts[0]
                .interruption
                .is_none()
        );
        assert_eq!(json["evidence"]["model_responses_omitted"], 3);
        assert_eq!(json["evidence"]["model_responses"][0]["frame_id"], 4);
        assert_eq!(
            json["run"]["attempts"][0]["terminal_desktop"]["foreground"]["window"]["executable"],
            "Taskmgr.exe"
        );
        assert_eq!(
            json["coverage"]["controller_revision"],
            crate::provider::CONTROLLER_REVISION
        );
    }

    #[test]
    fn export_keeps_the_whole_history_attempts_and_draft_without_credentials() {
        let mut settings = Settings {
            model: "first-model".into(),
            api_key: "fixture-secret-never-export".into(),
            ..Settings::default()
        };
        let mut run = RunView {
            id: 7,
            task: "Write Grüße 世界".into(),
            ..RunView::default()
        };
        run.begin_attempt(&settings, 1, None, "Original warm start");
        for id in 1..=200 {
            run.steps
                .push(Step::note(id, "agent", format!("Action {id}"), id as u64));
        }
        run.phase = "error".into();
        run.message = "The server timed out.".into();
        run.finish_attempt();
        add_correction(&mut run.steps, "Use the other field instead.", 201).unwrap();
        settings.model = "second-model".into();
        run.begin_attempt(
            &settings,
            201,
            Some("Use the other field instead.".into()),
            "Improved warm start",
        );
        run.phase = "done".into();
        run.result = "The note is ready.".into();
        run.finish_attempt();
        let report = SessionExport::new(
            run.clone(),
            7,
            None,
            "Verify the result".into(),
            Some("Add a closing sentence.".into()),
        )
        .unwrap();
        let text = serde_json::to_string(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["run"]["steps"].as_array().unwrap().len(), 201);
        assert_eq!(value["run"]["steps"][0]["description"], "Action 1");
        assert_eq!(value["run"]["steps"][200]["actor"], "user");
        assert_eq!(
            value["run"]["attempts"][0]["message"],
            "The server timed out."
        );
        assert_eq!(
            value["run"]["attempts"][1]["settings"]["model"],
            "second-model"
        );
        assert_eq!(
            value["run"]["attempts"][0]["warm_start_prompt"],
            "Original warm start"
        );
        assert_eq!(
            value["run"]["attempts"][1]["warm_start_prompt"],
            "Improved warm start"
        );
        assert_eq!(value["unsent_refinement"], "Add a closing sentence.");
        assert_eq!(run.steps.len(), 201);
        assert!(!text.contains("fixture-secret") && !text.contains("api_key"));
        assert!(text.contains("Grüße 世界"));
    }

    #[test]
    fn export_rejects_running_reset_and_stale_sessions() {
        for phase in ["idle", "checking", "countdown", "running"] {
            let run = RunView {
                id: 7,
                phase: phase.into(),
                ..RunView::default()
            };
            assert!(SessionExport::new(run, 7, None, String::new(), None).is_err());
        }
        let run = RunView {
            id: 8,
            phase: "done".into(),
            ..RunView::default()
        };
        assert!(SessionExport::new(run, 7, None, String::new(), None).is_err());
    }

    #[test]
    fn export_writes_unicode_json_and_preserves_existing_data_on_replace_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Verlauf 世界.json");
        write_report(&path, b"{\"version\":1}\n").unwrap();
        write_report(&path, b"{\"version\":2}\n").unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "{\"version\":2}\n");
        let blocked = directory.path().join("existing");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("keep.txt"), "keep").unwrap();
        assert!(write_report(&blocked, b"{}").is_err());
        assert_eq!(
            fs::read_to_string(blocked.join("keep.txt")).unwrap(),
            "keep"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
