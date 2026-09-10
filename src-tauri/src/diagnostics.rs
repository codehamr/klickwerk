use crate::guard::Frame;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WindowInfo {
    pub handle: usize,
    pub process_id: u32,
    pub title: String,
    pub class_name: String,
    pub executable: String,
    pub bounds: [i32; 4],
    pub integrity_level: Option<u32>,
    pub elevated: Option<bool>,
    pub inspection_error: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetInfo {
    pub window: WindowInfo,
    pub sender_integrity_level: Option<u32>,
    pub input_block: Option<String>,
}

// Elevation alone does not determine UIPI: an elevated sender may operate an
// elevated target at the same integrity level. Missing evidence fails closed.
pub fn input_block(
    own_window: bool,
    sender: Option<u32>,
    target: Option<u32>,
) -> Option<&'static str> {
    if own_window {
        Some("own_window")
    } else {
        match (sender, target) {
            (Some(sender), Some(target)) if target <= sender => None,
            (Some(_), Some(_)) => Some("higher_integrity"),
            _ => Some("window_inspection_failed"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonFailure {
    pub category: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputFailure {
    pub code: String,
    pub message: String,
    pub target: Option<Box<TargetInfo>>,
    pub win32_error: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub io_error_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_error: Option<Box<JsonFailure>>,
}
impl InputFailure {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            target: None,
            win32_error: None,
            io_error_kind: None,
            json_error: None,
        }
    }

    pub fn io(code: &str, message: &str, error: std::io::Error) -> Self {
        let mut failure = Self::new(code, message);
        failure.io_error_kind = Some(format!("{:?}", error.kind()));
        #[cfg(windows)]
        {
            failure.win32_error = error
                .raw_os_error()
                .and_then(|code| u32::try_from(code).ok());
        }
        failure
    }

    pub fn json(code: &str, message: &str, error: serde_json::Error) -> Self {
        let mut failure = Self::new(code, message);
        // Keep location/category, not messages that can echo private payload values.
        failure.json_error = Some(Box::new(JsonFailure {
            category: format!("{:?}", error.classify()),
            line: error.line(),
            column: error.column(),
        }));
        failure
    }
    pub fn can_restart_elevated(&self) -> bool {
        self.code == "higher_integrity" && self.target.as_ref().is_some_and(|target| {
            matches!((target.sender_integrity_level, target.window.integrity_level),
                (Some(sender), Some(required)) if sender < 0x3000 && required <= 0x3000 && required > sender)
        })
    }

    pub fn target(target: TargetInfo) -> Self {
        let code = target
            .input_block
            .as_deref()
            .unwrap_or("window_inspection_failed");
        let message = match code {
            "higher_integrity" => {
                "Windows blocks input because this app has higher privileges than klickwerk. Restart klickwerk as administrator, or reopen the target app without administrator rights. A manual text entry will not unblock subsequent clicks."
            }
            "own_window" => {
                "The action points at klickwerk itself. Bring the intended app into view, then continue or correct the instructions."
            }
            _ => {
                "Windows could not verify the target app's input permissions. Bring an accessible target window into view, or complete this step manually, then continue."
            }
        };
        Self {
            code: code.into(),
            message: message.into(),
            win32_error: target.window.inspection_error,
            target: Some(Box::new(target)),
            io_error_kind: None,
            json_error: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryEvent {
    pub recorded_at: u64,
    pub kind: String,
    pub source: String,
    pub sender_integrity_level: Option<u32>,
    pub failure: Option<InputFailure>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepDiagnostics {
    pub frame: Frame,
    pub focused_control: usize,
    pub foreground: TargetInfo,
    pub model_elapsed_ms: u64,
    pub observation_age_ms: u64,
    pub validation_elapsed_ms: Option<u64>,
    pub input_elapsed_ms: Option<u64>,
    pub targets: Vec<TargetInfo>,
    pub rejection: Option<InputFailure>,
    #[serde(default)]
    pub focused_element: Option<FocusedElement>,
    #[serde(default)]
    pub focus_inspection_error: Option<String>,
    #[serde(default)]
    pub observation: Option<ObservationReport>,
    #[serde(default)]
    pub repeat_check: Option<RepeatCheck>,
    #[serde(default)]
    pub input_completed_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusedElement {
    pub source: String,
    pub identity: Vec<i32>,
    pub bounds: [i32; 4],
    pub process_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureSample {
    pub frame_id: u64,
    pub captured_ms: u64,
    pub foreground: usize,
    pub focused_control: usize,
    pub changed: bool,
    pub input_effect_observed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationReport {
    pub reason: String,
    pub completion: String,
    pub elapsed_ms: u64,
    pub previous_input_step_id: Option<u32>,
    pub since_input_ms: Option<u64>,
    pub samples: Vec<CaptureSample>,
}
impl ObservationReport {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "reason": self.reason, "completion": self.completion, "elapsed_ms": self.elapsed_ms,
            "previous_input_step_id": self.previous_input_step_id, "since_input_ms": self.since_input_ms,
            "sample_count": self.samples.len(), "latest_sample": self.samples.last(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepeatCheck {
    pub previous_step_id: u32,
    pub previous_frame_id: u64,
    pub current_frame_id: u64,
    pub input_effect_observed: bool,
    pub outcome: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalDesktop {
    pub frame_id: Option<u64>,
    pub capture_error: Option<String>,
    pub captured_ms: u64,
    pub foreground: TargetInfo,
    pub focused_control: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelResponse {
    pub frame_id: u64,
    pub received_at: u64,
    pub response_model: Option<String>,
    pub finish_reason: Option<String>,
    pub assistant_content: Option<String>,
    pub content_truncated: bool,
    pub parse_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Handoff {
    pub method: String,
    pub foreground_before: usize,
    pub foreground_after: usize,
    pub visible: bool,
    pub minimized: bool,
    pub focused: bool,
    pub attachment_error: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privilege_checks_use_integrity_and_keep_own_and_unknown_targets_blocked() {
        assert_eq!(input_block(false, Some(0x2000), Some(0x2000)), None);
        assert_eq!(input_block(false, Some(0x3000), Some(0x3000)), None);
        assert_eq!(input_block(false, Some(0x3000), Some(0x2000)), None);
        assert_eq!(
            input_block(false, Some(0x2000), Some(0x3000)),
            Some("higher_integrity")
        );
        assert_eq!(
            input_block(true, Some(0x3000), Some(0x2000)),
            Some("own_window")
        );
        assert_eq!(
            input_block(false, None, Some(0x2000)),
            Some("window_inspection_failed")
        );
        assert_eq!(
            input_block(false, Some(0x2000), None),
            Some("window_inspection_failed")
        );
    }
}
