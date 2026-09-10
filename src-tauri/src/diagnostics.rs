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
pub struct InputFailure {
    pub code: String,
    pub message: String,
    pub target: Option<Box<TargetInfo>>,
    pub win32_error: Option<u32>,
}
impl InputFailure {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            target: None,
            win32_error: None,
        }
    }
    pub fn target(target: TargetInfo) -> Self {
        let code = target
            .input_block
            .as_deref()
            .unwrap_or("window_inspection_failed");
        let message = match code {
            "higher_integrity" => {
                "Windows blocks input to this app because it has higher privileges than klickwerk. Complete this step manually, or reopen the target normally if possible, then continue. Do not change Windows security settings."
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
        }
    }
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

#[derive(Clone, Debug, Serialize)]
pub struct TerminalDesktop {
    pub frame_id: Option<u64>,
    pub capture_error: Option<String>,
    pub captured_ms: u64,
    pub foreground: TargetInfo,
    pub focused_control: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelResponse {
    pub frame_id: u64,
    pub received_at: u64,
    pub response_model: Option<String>,
    pub finish_reason: Option<String>,
    pub assistant_content: Option<String>,
    pub content_truncated: bool,
    pub parse_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
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
