use crate::{action::Action, guard::Frame};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

pub const PORTABLE_LIMIT: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableWorkflow {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub prompt: String,
}
impl PortableWorkflow {
    pub fn encode(learning: &Learning) -> Result<Vec<u8>, String> {
        learning.validate()?;
        serde_json::to_vec_pretty(&Self {
            format: "klickwerk-workflow".into(),
            version: 1,
            name: learning.name.clone(),
            prompt: learning.prompt.clone(),
        })
        .map_err(|_| "The workflow could not be encoded.".into())
    }
    pub fn decode(bytes: &[u8]) -> Result<Learning, String> {
        if bytes.len() > PORTABLE_LIMIT {
            return Err("Workflow files must be smaller than 64 KiB.".into());
        }
        let file: Self = serde_json::from_slice(bytes)
            .map_err(|_| "Choose a valid klickwerk workflow JSON file.".to_owned())?;
        if file.version != 1 || file.format != "klickwerk-workflow" {
            return Err("This workflow format is not supported.".into());
        }
        let learning = Learning {
            name: file.name,
            prompt: file.prompt,
        };
        learning.validate()?;
        Ok(learning)
    }
}

pub fn slug(name: &str) -> String {
    let normalized = name
        .to_lowercase()
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss");
    let mut value = String::new();
    for word in normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(4)
    {
        if !value.is_empty() {
            value.push('-');
        }
        value.extend(word.chars().take(16));
    }
    if value.is_empty() {
        value = "workflow".into();
    }
    if [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
        "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ]
    .contains(&value.as_str())
    {
        value.push_str("-workflow");
    }
    value
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
    // The legacy library is preserved. A staged directory migration is committed
    // once, so deleted workflows cannot reappear from the old library on restart.
    pub fn load(legacy: PathBuf) -> Self {
        let path = legacy.with_file_name("workflows");
        let result = (|| -> Result<Vec<Workflow>, String> {
            if !path.exists() {
                let mut migrated = vec![];
                if legacy.exists() {
                    if fs::metadata(&legacy)
                        .map_err(|_| "Workflows could not be read.")?
                        .len()
                        > STORE_LIMIT as u64
                    {
                        return Err("workflows.json exceeds 16 MiB.".into());
                    }
                    let file: StoredFile = serde_json::from_slice(
                        &fs::read(&legacy).map_err(|_| "Workflows could not be read.")?,
                    )
                    .map_err(
                        |_| "workflows.json is damaged. The existing file has been preserved.",
                    )?;
                    if ![1, 2].contains(&file.version) || file.workflows.len() > 100 {
                        return Err("This workflow library is not supported.".into());
                    }
                    for mut stored in file.workflows {
                        if !stored.memory.trim().is_empty() {
                            stored.workflow.learning.prompt.push_str("\n\n");
                            stored
                                .workflow
                                .learning
                                .prompt
                                .push_str(stored.memory.trim());
                        }
                        stored.workflow.learning.validate()?;
                        migrated.push(stored.workflow.learning);
                    }
                }
                let staging = legacy.with_file_name(format!(
                    ".workflows-migration-{}-{}",
                    std::process::id(),
                    crate::session::timestamp()
                ));
                fs::create_dir(&staging).map_err(
                    |_| "Workflows could not be saved beside the app. Check folder permissions.",
                )?;
                let result = (|| -> Result<(), String> {
                    let mut store = Self {
                        path: staging.clone(),
                        workflows: vec![],
                        error: None,
                    };
                    for learning in migrated {
                        store.save(Workflow {
                            id: String::new(),
                            learning,
                            updated_at: 0,
                        })?;
                    }
                    fs::rename(&staging, &path)
                        .map_err(|_| "The workflow folder could not be created.".into())
                })();
                if result.is_err() {
                    let _ = fs::remove_dir_all(&staging);
                }
                result?;
            }
            if fs::symlink_metadata(&path)
                .map_err(|_| "Workflows could not be read.")?
                .file_type()
                .is_symlink()
            {
                return Err("The workflow folder must not be a symbolic link.".into());
            }
            let mut workflows = vec![];
            for entry in fs::read_dir(&path).map_err(|_| "Workflows could not be read.")? {
                let entry = entry.map_err(|_| "Workflows could not be read.")?;
                let filename = entry.file_name().to_string_lossy().into_owned();
                if !filename.ends_with(".json") {
                    continue;
                }
                let meta = entry
                    .metadata()
                    .map_err(|_| "Workflows could not be read.")?;
                if entry
                    .file_type()
                    .map_err(|_| "Workflows could not be read.")?
                    .is_symlink()
                    || !meta.is_file()
                {
                    return Err("Workflow files must be regular JSON files.".into());
                }
                if meta.len() > PORTABLE_LIMIT as u64 {
                    return Err("Workflow files must be smaller than 64 KiB.".into());
                }
                let learning = PortableWorkflow::decode(
                    &fs::read(entry.path()).map_err(|_| "Workflows could not be read.")?,
                )?;
                let id = filename.trim_end_matches(".json").to_owned();
                validate_id(&id)?;
                workflows.push(Workflow {
                    id,
                    learning,
                    updated_at: meta
                        .modified()
                        .ok()
                        .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|v| v.as_millis() as u64)
                        .unwrap_or(0),
                });
                if workflows.len() > 100 {
                    return Err("This workflow library is not supported.".into());
                }
            }
            workflows.sort_by(|a, b| {
                b.updated_at
                    .cmp(&a.updated_at)
                    .then_with(|| a.id.cmp(&b.id))
            });
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
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        item.learning.name = item.learning.name.trim().into();
        item.learning.prompt = item.learning.prompt.trim().into();
        item.learning.validate()?;
        let updating = self.workflows.iter().any(|w| w.id == item.id);
        if !updating {
            if self.workflows.len() >= 100 {
                return Err(
                    "Your library is full. Remove a workflow before saving another.".into(),
                );
            }
            let base = slug(&item.learning.name);
            item.id = base.clone();
            let mut suffix = 2;
            while self.path.join(format!("{}.json", item.id)).exists() {
                item.id = format!("{base}-{suffix}");
                suffix += 1;
            }
        }
        validate_id(&item.id)?;
        let path = self.path.join(format!("{}.json", item.id));
        let bytes = PortableWorkflow::encode(&item.learning)?;
        if updating {
            // Never replace an externally modified file or a redirected path.
            let current = self.get(&item.id)?;
            let meta = fs::symlink_metadata(&path)
                .map_err(|_| "This workflow changed. Open it again before saving.")?;
            if !meta.is_file()
                || fs::read(&path)
                    .ok()
                    .and_then(|bytes| PortableWorkflow::decode(&bytes).ok())
                    != Some(current.learning)
            {
                return Err("This workflow changed. Open it again before saving.".into());
            }
            crate::session::write_report(&path, &bytes)?;
        } else {
            // Publish a completed file without replacing a file another process
            // created after the name was suggested.
            let temporary = self.path.join(format!(
                ".workflow-{}-{}.tmp",
                std::process::id(),
                crate::session::timestamp()
            ));
            let result = (|| -> Result<(), String> {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                    .map_err(|_| "Workflows could not be saved.")?;
                file.write_all(&bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "Workflows could not be saved.")?;
                drop(file);
                publish_new(&temporary, &path).map_err(
                    |_| "Workflows could not be saved. Check the folder permissions and filename.",
                )?;
                Ok(())
            })();
            let _ = fs::remove_file(temporary);
            result?;
        }
        item.updated_at = crate::session::timestamp().max(
            self.workflows
                .iter()
                .find(|w| w.id == item.id)
                .map(|w| w.updated_at.saturating_add(1))
                .unwrap_or(0),
        );
        self.workflows.retain(|w| w.id != item.id);
        self.workflows.insert(0, item.clone());
        Ok(item)
    }
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.get(id)?;
        validate_id(id)?;
        fs::remove_file(self.path.join(format!("{id}.json")))
            .map_err(|_| "The workflow could not be deleted.")?;
        self.workflows.retain(|w| w.id != id);
        Ok(())
    }
}
// MOVEFILE_REPLACE_EXISTING is deliberately absent. This also supports FAT/exFAT
// portable drives, which do not support hard links.
#[cfg(windows)]
fn publish_new(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn publish_new(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    fs::hard_link(source, target)
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 240
        || id.contains(['/', '\\', ':', '.'])
        || id.chars().any(|c| c.is_control())
    {
        return Err("This workflow has an invalid identifier.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_import_is_strict_and_filenames_are_short_safe_and_unique() {
        let learning = Learning {
            name: "März / Bericht".into(),
            prompt: "Write Grüße 世界".into(),
        };
        let bytes = PortableWorkflow::encode(&learning).unwrap();
        assert_eq!(PortableWorkflow::decode(&bytes).unwrap(), learning);
        assert_eq!(slug(&learning.name), "maerz-bericht");
        assert_eq!(slug("../CON"), "con-workflow");
        assert_eq!(slug("../../"), "workflow");
        for invalid in [
            r#"{"format":"klickwerk-workflow","version":2,"name":"Test","prompt":"Task"}"#,
            r#"{"format":"klickwerk-workflow","version":1,"name":"Test","prompt":"Task","path":"../evil"}"#,
            r#"{"format":"klickwerk-workflow","version":1,"name":"Test","prompt":""}"#,
        ] {
            assert!(PortableWorkflow::decode(invalid.as_bytes()).is_err());
        }
        assert!(PortableWorkflow::decode(&vec![b'a'; PORTABLE_LIMIT + 1]).is_err());
        let dir = tempfile::tempdir().unwrap();
        let mut store = WorkflowStore::load(dir.path().join("workflows.json"));
        let item = Workflow {
            id: "../../escape".into(),
            learning,
            updated_at: 0,
        };
        let first = store.save(item.clone()).unwrap();
        let second = store.save(item).unwrap();
        assert_eq!(first.id, "maerz-bericht");
        assert_eq!(second.id, "maerz-bericht-2");
        assert!(!dir.path().join("escape.json").exists());
        let saved = fs::read(store.path.join("maerz-bericht.json")).unwrap();
        assert_eq!(saved, bytes);
        store.delete(&first.id).unwrap();
        assert_eq!(
            WorkflowStore::load(dir.path().join("workflows.json"))
                .workflows
                .len(),
            1
        );
    }
    #[test]
    fn external_changes_and_damaged_files_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("workflows.json");
        let mut store = WorkflowStore::load(legacy.clone());
        let saved = store
            .save(Workflow {
                id: "".into(),
                learning: Learning {
                    name: "Note".into(),
                    prompt: "Original".into(),
                },
                updated_at: 0,
            })
            .unwrap();
        fs::write(store.path.join("note.json"), "external change").unwrap();
        assert!(store.save(saved).is_err());
        let mut reloaded = WorkflowStore::load(legacy);
        assert!(reloaded.error.is_some());
        assert!(reloaded.delete("note").is_err());
        assert_eq!(
            fs::read_to_string(store.path.join("note.json")).unwrap(),
            "external change"
        );
    }
    #[test]
    fn migration_does_not_resurrect_deleted_entries_or_modify_legacy_files() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("workflows.json");
        let old = r#"{"version":2,"workflows":[{"id":"old","name":"Note","prompt":"Write a note","updated_at":1}]}"#;
        fs::write(&legacy, old).unwrap();
        let mut store = WorkflowStore::load(legacy.clone());
        store.delete("note").unwrap();
        assert!(WorkflowStore::load(legacy.clone()).workflows.is_empty());
        assert_eq!(fs::read_to_string(legacy).unwrap(), old);
    }
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
        let saved = store.save(workflow).unwrap();
        let mut loaded = WorkflowStore::load(path.clone());
        let mut edited = loaded.get(&saved.id).unwrap();
        let contents = fs::read_to_string(store.path.join(format!("{}.json", saved.id))).unwrap();
        assert!(!contents.contains("steps"));
        assert!(!contents.contains("memory"));
        assert!(!contents.contains("desktop_points"));
        edited.learning.prompt = "An edited warm start".into();
        loaded.save(edited).unwrap();
        assert_eq!(
            WorkflowStore::load(path.clone())
                .get(&saved.id)
                .unwrap()
                .learning
                .prompt,
            "An edited warm start"
        );
        loaded.delete(&saved.id).unwrap();
        assert!(WorkflowStore::load(path).workflows.is_empty());
    }
    #[test]
    fn legacy_workflows_keep_instructions_and_drop_raw_history_on_next_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workflows.json");
        fs::write(&path, r#"{"version":1,"workflows":[{"id":"old","name":"Note","prompt":"Write a note","memory":"Verify the folder","task":"Original task","steps":[{"description":"raw history"}],"updated_at":1}]}"#).unwrap();
        let mut store = WorkflowStore::load(path.clone());
        assert!(store.error.is_none());
        let workflow = store.get("note").unwrap();
        assert_eq!(
            workflow.learning.prompt,
            "Write a note\n\nVerify the folder"
        );
        store.save(workflow).unwrap();
        let contents = fs::read_to_string(store.path.join("note.json")).unwrap();
        assert!(fs::read_to_string(path).unwrap().contains("raw history"));
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
        assert!(store.delete("missing").is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
}
