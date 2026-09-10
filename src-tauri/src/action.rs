use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub frame_id: u64,
    pub description: String,
    pub action: Action,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Click {
        x: i32,
        y: i32,
        button: Button,
    },
    DoubleClick {
        x: i32,
        y: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
    Drag {
        x: i32,
        y: i32,
        x2: i32,
        y2: i32,
        duration_ms: u32,
    },
    Scroll {
        x: i32,
        y: i32,
        amount: i32,
    },
    Text {
        text: String,
    },
    Key {
        key: String,
        modifiers: Vec<Modifier>,
    },
    Wait {
        duration_ms: u32,
    },
    Observe,
    AskUser {
        question: String,
    },
    Finish {
        summary: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Win,
}

impl Decision {
    pub fn parse(text: &str, frame_id: u64, width: u32, height: u32) -> Result<Self, String> {
        if text.len() > 32768 {
            return Err("The model response is too large.".into());
        }
        let value: Self = serde_json::from_str(text).map_err(|_| "The model did not return a valid desktop action. Choose a compatible vision model.")?;
        if value.frame_id != frame_id || frame_id == 0 {
            return Err("The model action refers to an outdated screenshot.".into());
        }
        if value.description.trim().is_empty() || value.description.len() > 512 {
            return Err("The model action has an invalid description.".into());
        }
        value.action.validate(width, height)?;
        Ok(value)
    }
}

impl Action {
    pub fn same_input(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Key {
                    key: a,
                    modifiers: am,
                },
                Self::Key {
                    key: b,
                    modifiers: bm,
                },
            ) => {
                key_code(a) == key_code(b)
                    && am.len() == bm.len()
                    && am.iter().all(|m| bm.contains(m))
            }
            _ => serde_json::to_value(self).ok() == serde_json::to_value(other).ok(),
        }
    }

    pub fn expects_window_transition(&self) -> bool {
        match self {
            Self::Key { key, modifiers } => {
                (key_code(key) == Some(0x1b)
                    && modifiers.len() == 2
                    && modifiers.contains(&Modifier::Ctrl)
                    && modifiers.contains(&Modifier::Shift))
                    || (key.eq_ignore_ascii_case("TAB") && modifiers.contains(&Modifier::Alt))
                    || key_code(key) == Some(0x5b)
                    || modifiers.contains(&Modifier::Win)
            }
            _ => false,
        }
    }

    pub fn is_input(&self) -> bool {
        !matches!(
            self,
            Self::Wait { .. } | Self::Observe | Self::AskUser { .. } | Self::Finish { .. }
        )
    }

    pub fn points(&self) -> Vec<(i32, i32)> {
        match *self {
            Self::Click { x, y, .. }
            | Self::DoubleClick { x, y }
            | Self::Move { x, y }
            | Self::Scroll { x, y, .. } => vec![(x, y)],
            Self::Drag { x, y, x2, y2, .. } => vec![(x, y), (x2, y2)],
            _ => vec![],
        }
    }

    pub fn validate(&self, width: u32, height: u32) -> Result<(), String> {
        if self
            .points()
            .iter()
            .any(|&(x, y)| x < 0 || y < 0 || x as u32 >= width || y as u32 >= height)
        {
            return Err("The model selected a point outside the screenshot.".into());
        }
        let valid = match self {
            Self::Drag { duration_ms, .. } => (50..=1000).contains(duration_ms),
            Self::Scroll { amount, .. } => *amount != 0 && (-10..=10).contains(amount),
            Self::Text { text } => !text.is_empty() && text.len() <= 4096 && !text.contains('\0'),
            Self::Key { key, modifiers } => {
                key_code(key).is_some()
                    && modifiers.len() <= 4
                    && modifiers
                        .iter()
                        .enumerate()
                        .all(|(i, m)| !modifiers[..i].contains(m))
            }
            Self::Wait { duration_ms } => (1..=2000).contains(duration_ms),
            Self::AskUser { question } => !question.trim().is_empty() && question.len() <= 4096,
            Self::Finish { summary } => !summary.trim().is_empty() && summary.len() <= 4096,
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err("The model action exceeded a supported limit.".into())
        }
    }
}

pub fn key_code(key: &str) -> Option<u16> {
    Some(match key.to_ascii_uppercase().as_str() {
        "ENTER" => 0x0d,
        "TAB" => 0x09,
        "ESCAPE" | "ESC" => 0x1b,
        "WIN" | "SUPER" | "META" => 0x5b,
        "INSERT" => 0x2d,
        "SPACE" => 0x20,
        "BACKSPACE" => 0x08,
        "DELETE" => 0x2e,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_uppercase() => {
            value.as_bytes()[0] as u16
        }
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_digit() => {
            value.as_bytes()[0] as u16
        }
        value if value.starts_with('F') => {
            let n = value[1..].parse::<u16>().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            0x6f + n
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_actions_reject_ambiguous_and_outdated_output() {
        let valid = r#"{"frame_id":1,"description":"Open editor","action":{"type":"click","x":10,"y":20,"button":"left"}}"#;
        assert!(Decision::parse(valid, 1, 100, 100).is_ok());
        assert!(Decision::parse(valid, 2, 100, 100).is_err());
        assert!(Decision::parse(valid, 1, 10, 100).is_err());
        for bad in [
            format!("```json\n{valid}\n```"),
            valid.replace("\"x\":10", "\"x\":10,\"x\":11"),
            valid.replace("\"x\":10", "\"x\":10,\"shell\":\"calc\""),
            format!("{valid} trailing"),
        ] {
            assert!(Decision::parse(&bad, 1, 100, 100).is_err());
        }
    }
    #[test]
    fn bounded_keys_and_text() {
        assert!(
            Action::Key {
                key: "F8".into(),
                modifiers: vec![Modifier::Ctrl, Modifier::Alt]
            }
            .validate(100, 100)
            .is_ok()
        );
        assert!(
            Action::Text {
                text: "Grüße 世界 🪷".into()
            }
            .validate(100, 100)
            .is_ok()
        );
        assert!(
            Action::Text {
                text: "x".repeat(4097)
            }
            .validate(100, 100)
            .is_err()
        );
        assert_eq!(key_code("WIN"), Some(0x5b));
        assert_eq!(key_code("Enter"), Some(0x0d));
        assert_eq!(key_code("a"), Some(0x41));
    }
}
