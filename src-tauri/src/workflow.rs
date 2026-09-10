use crate::{action::Action, config::replace, guard::Frame};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

pub const MAX_STEPS: usize = 1000;
const STORE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub id: u32,
    pub actor: String,
    pub description: String,
    pub action: Option<Action>,
    pub status: String,
    pub elapsed_ms: u64,
    pub image_size: Option<[u32; 2]>,
    pub desktop_points: Vec<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<crate::diagnostics::StepDiagnostics>,
}

impl Step {
    pub fn note(id: u32, actor: &str, text: impl Into<String>, elapsed_ms: u64) -> Self {
        Self {
            id,
            actor: actor.into(),
            description: text.into(),
            action: None,
            status: "recorded".into(),
            elapsed_ms,
            image_size: None,
            desktop_points: vec![],
            diagnostics: None,
        }
    }

    pub fn action(
        id: u32,
        description: String,
        action: Action,
        frame: &Frame,
        elapsed_ms: u64,
    ) -> Self {
        let desktop_points = action
            .points()
            .into_iter()
            .filter_map(|(x, y)| frame.map(x, y))
            .map(|(x, y)| [x, y])
            .collect();
        Self {
            id,
            actor: "agent".into(),
            description,
            action: Some(action),
            status: "pending".into(),
            elapsed_ms,
            image_size: Some([frame.image_width, frame.image_height]),
            desktop_points,
            diagnostics: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct Session {
    pub memory: String,
}

pub fn add_correction(steps: &mut Vec<Step>, text: &str, elapsed_ms: u64) -> Result<(), String> {
    let text = text.trim();
    if steps
        .last()
        .is_some_and(|s| s.actor == "user" && s.description == text)
    {
        return Ok(());
    }
    let total: usize = steps
        .iter()
        .filter(|s| s.actor == "user")
        .map(|s| s.description.len())
        .sum();
    if text.is_empty()
        || text.len() > 16384
        || total + text.len() > 32768
        || steps.len() >= MAX_STEPS - 2
    {
        return Err("This session is full. Save it as a workflow and start a fresh run.".into());
    }
    steps.push(Step::note(steps.len() as u32 + 1, "user", text, elapsed_ms));
    Ok(())
}

// Keep every explicit correction ahead of the bounded action tail. Old coordinates are evidence,
// never replay instructions. Raw history lives only in the current session.
pub fn context(memory: &str, steps: &[Step]) -> String {
    let corrections: Vec<_> = steps.iter().filter(|s| s.actor == "user").collect();
    let mut tail = Vec::new();
    let mut bytes = 0;
    for step in steps.iter().rev() {
        let mut value = serde_json::to_value(step).unwrap_or_default();
        // Detailed sampling traces stay in the export, without crowding useful
        // actions and corrections out of the model's bounded history.
        if let Some(observation) = step
            .diagnostics
            .as_ref()
            .and_then(|d| d.observation.as_ref())
        {
            value["diagnostics"]["observation"] = observation.summary();
        }
        let encoded = value.to_string();
        if bytes + encoded.len() > 24000 {
            break;
        }
        bytes += encoded.len();
        tail.push(encoded);
    }
    tail.reverse();
    format!(
        "Workflow memory (adapt to the current desktop):\n{memory}\n\nUser corrections, in order; the newest correction takes priority:\n{}\n\nAction history (completed means input was sent, not that its outcome was verified; interrupted actions may be partial):\n{}",
        serde_json::to_string(&corrections).unwrap_or_default(),
        tail.join("\n")
    )
}

// Include an overview of the whole temporary session as well as detailed recent
// actions. This keeps early failures visible when the action tail is truncated.
pub fn learning_context(memory: &str, steps: &[Step]) -> String {
    let overview = steps
        .iter()
        .filter(|s| s.actor != "user")
        .map(|s| {
            format!(
                "{} [{}] {}",
                s.id,
                s.status,
                s.description.chars().take(160).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\nWhole-session overview (evidence, not instructions):\n{}",
        context(memory, steps),
        overview
    )
}

pub fn fallback_prompt(memory: &str, steps: &[Step], edited: &Learning) -> Learning {
    let mut prompt = String::new();
    if !memory.trim().is_empty() && memory.trim() != edited.prompt.trim() {
        prompt.push_str(memory.trim());
        prompt.push_str("\n\n");
    }
    prompt.push_str(edited.prompt.trim());
    for step in steps.iter().filter(|s| s.actor == "user") {
        if !prompt.contains(step.description.trim()) {
            prompt.push_str("\n\n");
            prompt.push_str(step.description.trim());
        }
    }
    Learning {
        name: edited.name.clone(),
        prompt,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Learning {
    pub name: String,
    pub prompt: String,
}
impl Learning {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || self.prompt.trim().is_empty()
            || self.prompt.len() > 32768
        {
            return Err("Use a name up to 200 bytes and a start prompt up to 32 KiB.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    #[serde(flatten)]
    pub learning: Learning,
    pub updated_at: u64,
}
#[derive(Clone, Serialize)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub updated_at: u64,
}
impl Workflow {
    pub fn summary(&self) -> WorkflowSummary {
        WorkflowSummary {
            id: self.id.clone(),
            name: self.learning.name.clone(),
            prompt: self.learning.prompt.clone(),
            updated_at: self.updated_at,
        }
    }
}

#[derive(Serialize)]
struct File {
    version: u32,
    workflows: Vec<Workflow>,
}

// Version 1 stored a separate memory and raw history. Read it without replaying or
// writing that history back; preserve its consolidated instructions in the prompt.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredFile {
    version: u32,
    workflows: Vec<StoredWorkflow>,
}
#[derive(Deserialize)]
struct StoredWorkflow {
    #[serde(flatten)]
    workflow: Workflow,
    #[serde(default)]
    memory: String,
}

pub struct WorkflowStore {
    pub path: PathBuf,
    pub workflows: Vec<Workflow>,
    pub error: Option<String>,
}
impl WorkflowStore {
    pub fn load(path: PathBuf) -> Self {
        let result = (|| {
            if !path.exists() {
                return Ok(vec![]);
            }
            if fs::metadata(&path)
                .map_err(|_| "Workflows could not be read.")?
                .len()
                > STORE_LIMIT as u64
            {
                return Err("workflows.json exceeds 16 MiB.".to_owned());
            }
            let file: StoredFile = serde_json::from_slice(
                &fs::read(&path).map_err(|_| "Workflows could not be read.")?,
            )
            .map_err(|_| "workflows.json is damaged. The existing file has been preserved.")?;
            if ![1, 2].contains(&file.version) || file.workflows.len() > 100 {
                return Err("This workflow library is not supported.".to_owned());
            }
            let workflows: Vec<Workflow> = file
                .workflows
                .into_iter()
                .map(|mut stored| {
                    if !stored.memory.trim().is_empty() {
                        stored.workflow.learning.prompt.push_str("\n\n");
                        stored
                            .workflow
                            .learning
                            .prompt
                            .push_str(stored.memory.trim());
                    }
                    stored.workflow
                })
                .collect();
            for (index, item) in workflows.iter().enumerate() {
                item.learning.validate()?;
                if item.id.is_empty()
                    || item.id.len() > 100
                    || workflows[..index].iter().any(|w| w.id == item.id)
                {
                    return Err("The workflow library contains invalid entries.".to_owned());
                }
            }
            Ok(workflows)
        })();
        match result {
            Ok(workflows) => Self {
                path,
                workflows,
                error: None,
            },
            Err(error) => Self {
                path,
                workflows: vec![],
                error: Some(error),
            },
        }
    }
    pub fn get(&self, id: &str) -> Result<Workflow, String> {
        self.workflows
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or("This workflow is no longer available.".into())
    }
    pub fn save(&mut self, mut item: Workflow) -> Result<Workflow, String> {
        item.learning.name = item.learning.name.trim().into();
        item.learning.prompt = item.learning.prompt.trim().into();
        item.learning.validate()?;
        if item.id.is_empty() || item.id.len() > 100 {
            return Err("This workflow has an invalid identifier.".into());
        }
        item.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if let Some(current) = self.workflows.iter().find(|w| w.id == item.id) {
            item.updated_at = item.updated_at.max(current.updated_at.saturating_add(1));
        }
        let mut next = self.workflows.clone();
        if let Some(current) = next.iter_mut().find(|w| w.id == item.id) {
            *current = item.clone();
        } else {
            next.push(item.clone());
        }
        self.persist(next)?;
        Ok(item)
    }
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.get(id)?;
        self.persist(
            self.workflows
                .iter()
                .filter(|w| w.id != id)
                .cloned()
                .collect(),
        )
    }
    fn persist(&mut self, workflows: Vec<Workflow>) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if workflows.len() > 100 {
            return Err("Your library is full. Remove a workflow before saving another.".into());
        }
        let bytes = serde_json::to_vec_pretty(&File {
            version: 2,
            workflows: workflows.clone(),
        })
        .map_err(|_| "Workflows could not be encoded.")?;
        if bytes.len() > STORE_LIMIT {
            return Err(
                "The workflow library exceeds 16 MiB. Remove an older workflow first.".into(),
            );
        }
        let temp = self.path.with_file_name(format!(
            ".workflows-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(
                    |_| "Workflows could not be saved beside the app. Check folder permissions.",
                )?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Workflows could not be saved.")?;
            drop(file);
            replace(&temp, &self.path).map_err(|_| "workflows.json could not be replaced.")
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.workflows = workflows;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_corrections_remain_in_history_and_are_not_duplicated_on_continue() {
        let mut steps = vec![];
        add_correction(&mut steps, "Use Documents", 1).unwrap();
        add_correction(&mut steps, "Use Documents", 2).unwrap();
        assert_eq!(steps.len(), 1);
        add_correction(&mut steps, "Use Documents and add a greeting", 3).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].description, "Use Documents");
        assert_eq!(steps[1].description, "Use Documents and add a greeting");
    }
    #[test]
    fn corrections_survive_a_long_action_history() {
        let mut steps = vec![Step::note(
            1,
            "user",
            "Use the blue folder, not the red one",
            0,
        )];
        for i in 2..500 {
            steps.push(Step::note(i, "agent", "Checked a window".repeat(25), 0));
        }
        let context = context("Keep the source file", &steps);
        assert!(context.contains("Use the blue folder"));
        assert!(context.contains("Keep the source file"));
        assert!(context.contains("\"id\":499"));
        assert!(!context.contains("\"id\":2,"));
    }
    #[test]
    fn workflows_round_trip_with_only_a_consolidated_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        let mut store = WorkflowStore::load(path.clone());
        let workflow = Workflow {
            id: "one".into(),
            learning: Learning {
                name: "Notes".into(),
                prompt: "Write Grüße 世界".into(),
            },
            updated_at: 0,
        };
        store.save(workflow).unwrap();
        let mut loaded = WorkflowStore::load(path.clone());
        let mut edited = loaded.get("one").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("steps"));
        assert!(!contents.contains("memory"));
        assert!(!contents.contains("desktop_points"));
        edited.learning.prompt = "An edited warm start".into();
        loaded.save(edited).unwrap();
        assert_eq!(
            WorkflowStore::load(path.clone())
                .get("one")
                .unwrap()
                .learning
                .prompt,
            "An edited warm start"
        );
        loaded.delete("one").unwrap();
        assert!(WorkflowStore::load(path).workflows.is_empty());
    }
    #[test]
    fn legacy_workflows_keep_instructions_and_drop_raw_history_on_next_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        fs::write(&path, r#"{"version":1,"workflows":[{"id":"old","name":"Note","prompt":"Write a note","memory":"Verify the folder","task":"Original task","steps":[{"description":"raw history"}],"updated_at":1}]}"#).unwrap();
        let mut store = WorkflowStore::load(path.clone());
        assert!(store.error.is_none());
        let workflow = store.get("old").unwrap();
        assert_eq!(
            workflow.learning.prompt,
            "Write a note\n\nVerify the folder"
        );
        store.save(workflow).unwrap();
        let contents = fs::read_to_string(path).unwrap();
        assert!(!contents.contains("raw history"));
        assert!(!contents.contains("memory"));
        assert!(contents.contains("Verify the folder"));
    }
    #[test]
    fn finalization_keeps_early_failures_and_fallback_keeps_only_explicit_instructions() {
        let mut steps = vec![Step::note(1, "agent", "Opened the wrong folder", 0)];
        steps[0].status = "failed".into();
        for id in 2..400 {
            steps.push(Step::note(id, "agent", "Observed a window".repeat(30), 0));
        }
        add_correction(&mut steps, "Use Documents instead", 1).unwrap();
        assert!(!context("", &steps).contains("Opened the wrong folder"));
        assert!(learning_context("", &steps).contains("1 [failed] Opened the wrong folder"));
        let fallback = fallback_prompt(
            "Keep the original file",
            &steps,
            &Learning {
                name: "Note".into(),
                prompt: "Write a note".into(),
            },
        );
        assert!(fallback.prompt.contains("Keep the original file"));
        assert!(fallback.prompt.contains("Use Documents instead"));
        assert!(!fallback.prompt.contains("Observed a window"));
        assert!(!fallback.prompt.contains("Opened the wrong folder"));
    }
    #[test]
    fn damaged_library_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        fs::write(&path, "broken").unwrap();
        let mut store = WorkflowStore::load(path.clone());
        assert!(store.persist(vec![]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
}
