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
        }
    }
}

#[derive(Clone, Default)]
pub struct Session {
    pub memory: String,
    pub prior_steps: Vec<Step>,
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
// never replay instructions. The full history remains available in the saved workflow.
pub fn context(memory: &str, steps: &[Step]) -> String {
    let corrections: Vec<_> = steps.iter().filter(|s| s.actor == "user").collect();
    let mut tail = Vec::new();
    let mut bytes = 0;
    for step in steps.iter().rev() {
        let encoded = serde_json::to_string(step).unwrap_or_default();
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Learning {
    pub name: String,
    pub prompt: String,
    pub memory: String,
}
impl Learning {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || self.prompt.trim().is_empty()
            || self.prompt.len() > 32768
            || self.memory.trim().is_empty()
            || self.memory.len() > 16000
        {
            return Err(
                "Use a name up to 200 bytes, a prompt up to 32 KiB, and instructions up to 16 KiB."
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    #[serde(flatten)]
    pub learning: Learning,
    pub task: String,
    pub steps: Vec<Step>,
    pub updated_at: u64,
}
#[derive(Clone, Serialize)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub corrections: usize,
    pub updated_at: u64,
}
impl Workflow {
    pub fn summary(&self) -> WorkflowSummary {
        WorkflowSummary {
            id: self.id.clone(),
            name: self.learning.name.clone(),
            prompt: self.learning.prompt.clone(),
            corrections: self.steps.iter().filter(|s| s.actor == "user").count(),
            updated_at: self.updated_at,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    workflows: Vec<Workflow>,
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
            let file: File = serde_json::from_slice(
                &fs::read(&path).map_err(|_| "Workflows could not be read.")?,
            )
            .map_err(|_| "workflows.json is damaged. The existing file has been preserved.")?;
            if file.version != 1 || file.workflows.len() > 100 {
                return Err("This workflow library is not supported.".to_owned());
            }
            for (index, item) in file.workflows.iter().enumerate() {
                item.learning.validate()?;
                if item.steps.len() > MAX_STEPS
                    || item.id.is_empty()
                    || item.id.len() > 100
                    || file.workflows[..index].iter().any(|w| w.id == item.id)
                {
                    return Err("The workflow library contains invalid entries.".to_owned());
                }
            }
            Ok(file.workflows)
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
        if item.id.is_empty() || item.id.len() > 100 || item.steps.len() > MAX_STEPS {
            return Err("This workflow exceeds the supported history size.".into());
        }
        item.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
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
            version: 1,
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
    fn workflows_round_trip_and_keep_edited_prompts_and_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        let mut store = WorkflowStore::load(path.clone());
        let workflow = Workflow {
            id: "one".into(),
            learning: Learning {
                name: "Notes".into(),
                prompt: "Write Grüße 世界".into(),
                memory: "Verify the selected folder".into(),
            },
            task: "Original".into(),
            steps: vec![Step::note(1, "user", "Use Documents", 0)],
            updated_at: 0,
        };
        store.save(workflow).unwrap();
        let mut loaded = WorkflowStore::load(path.clone());
        let mut edited = loaded.get("one").unwrap();
        assert_eq!(edited.steps[0].description, "Use Documents");
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
    fn damaged_library_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        fs::write(&path, "broken").unwrap();
        let mut store = WorkflowStore::load(path.clone());
        assert!(store.persist(vec![]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
}
