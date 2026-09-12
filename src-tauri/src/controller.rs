use crate::action::Action;

pub const WORK_GUIDANCE: &str = r#"Plan toward the requested outcome before acting. For content creation, keep a short sequence in mind: prepare the workspace, create the complete content, inspect and repair it, save in the requested location and format, then verify. Continue the unfinished phase; do not switch to saving merely because several actions have elapsed. An early checkpoint is appropriate when requested or useful for lengthy work, but it is not completion. Any later edit requires another save and verification.
Change zoom only to solve a visible targeting or readability problem. A small canvas alone does not require zooming. Distinguish view zoom from document dimensions. If zoom is needed, locate the actual control and current value, make one deliberate adjustment, and verify the new value and canvas bounds before drawing. Keep the whole intended composition visible and re-locate coordinates after any view change. Do not probe nearby controls or repeatedly adjust zoom without checking the result.
For drawings, choose a simple recognizable composition and fit all its parts inside the visible canvas. Verify the active drawing tool, color and stroke size. Prefer suitable shape tools for clean geometry when available. One drag is one straight segment, not several sides or a freehand curve. Describe only the segment this action will draw. Check its endpoints and visible result before adding the next segment; repair a misplaced or duplicate stroke before continuing. Join outlines intended to be closed and include the features that make the requested object recognizable. Do not substitute isolated strokes for unfinished parts or add unrequested polish indefinitely.
Before saving, inspect the complete content against the user's request. In Save As, explicitly verify the destination folder, filename and selected file type; typing an extension alone does not verify the encoding. Handle any format or overwrite dialog according to the user's authorization. After the final edit, save and inspect the resulting document and visible save state. A sent shortcut, closed dialog, filename in the title, or desktop icon alone does not prove that the requested content and format were saved. If evidence is missing, use the app or reopen the saved result to check it. Leave the completed result visible unless the user requests a different final view; a desktop destination does not by itself require hiding the editor.
Only claim completion after checking each requested outcome against visible evidence. Distinguish the model's earlier descriptions from observations. If an autonomous check or repair is available, perform it before asking the user."#;

pub const COMPLETION_GUIDANCE: &str = r#"Completion review: the previous finish proposal was deferred by the controller, and this is a NEW desktop observation. Review the original task and latest user corrections against the actual result, rather than trusting the previous summary or completed-input labels. For created or edited files, check content completeness, the requested destination/name/type, and saving after the last edit. A desktop icon cannot establish the file's contents. If something is incomplete or unverified, return the next concrete check or repair action using the usual action schema. Use ask_user only if no autonomous check can resolve the blocker. Return finish only when the available evidence supports every requested outcome. In its summary state the verified result without claiming checks you did not perform."#;

pub const REVIEW_MESSAGE: &str =
    "Checking the result against your request with a fresh screenshot…";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionMode {
    Act,
    ReviewCompletion,
}

// A finish proposal never ends a run on its first pass. A review uses a later
// observation, and any intervening check or repair requires a new review.
#[derive(Default)]
pub struct CompletionGate {
    proposed_frame: Option<u64>,
}

impl CompletionGate {
    pub fn mode(&self) -> DecisionMode {
        if self.proposed_frame.is_some() {
            DecisionMode::ReviewCompletion
        } else {
            DecisionMode::Act
        }
    }

    pub fn defer_finish(&mut self, action: &Action, frame_id: u64) -> bool {
        if !matches!(action, Action::Finish { .. }) {
            self.proposed_frame = None;
            return false;
        }
        if self
            .proposed_frame
            .is_some_and(|previous| frame_id > previous)
        {
            self.proposed_frame = None;
            return false;
        }
        self.proposed_frame = Some(self.proposed_frame.unwrap_or(0).max(frame_id));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish() -> Action {
        Action::Finish {
            summary: "The requested file is saved.".into(),
        }
    }

    #[test]
    fn completion_requires_a_later_observation() {
        let mut gate = CompletionGate::default();
        assert_eq!(gate.mode(), DecisionMode::Act);
        assert!(gate.defer_finish(&finish(), 30));
        assert_eq!(gate.mode(), DecisionMode::ReviewCompletion);
        assert!(gate.defer_finish(&finish(), 30));
        assert!(gate.defer_finish(&finish(), 29));
        assert!(!gate.defer_finish(&finish(), 31));
    }

    #[test]
    fn checks_repairs_and_handoffs_invalidate_the_previous_proposal() {
        for action in [
            Action::Observe,
            Action::Wait { duration_ms: 500 },
            Action::Key {
                key: "S".into(),
                modifiers: vec![crate::action::Modifier::Ctrl],
            },
            Action::AskUser {
                question: "Which destination should I use?".into(),
            },
        ] {
            let mut gate = CompletionGate::default();
            assert!(gate.defer_finish(&finish(), 10));
            assert!(!gate.defer_finish(&action, 11));
            assert_eq!(gate.mode(), DecisionMode::Act);
            assert!(gate.defer_finish(&finish(), 12));
            assert!(!gate.defer_finish(&finish(), 13));
        }
    }
}
