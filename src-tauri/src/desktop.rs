use crate::{
    action::Action,
    config::{ConfigStore, Settings, same_origin},
    guard::{Command, Reply},
    platform::{self, capture, input::HeldInput},
    provider::{self, ModelList, Provider},
    session::{RunView, SessionExport},
    workflow::{self, Learning, Session, Step, Workflow, WorkflowStore, WorkflowSummary},
};
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::{
    collections::HashMap,
    future::Future,
    io::{BufRead, Read, Write},
    process::{Child, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc as async_mpsc, oneshot};
use windows_sys::Win32::{Foundation::*, Security::Cryptography::*, System::Threading::*};

#[cfg(feature = "safety-tests")]
#[path = "desktop_tests.rs"]
pub mod safety_tests;

#[derive(Clone, Serialize)]
pub struct Snapshot {
    settings: Settings,
    has_api_key: bool,
    config_path: String,
    config_error: Option<String>,
    run: RunView,
    platform: &'static str,
    activity_busy: bool,
    locale: &'static str,
    workflows: Vec<WorkflowSummary>,
    workflow_error: Option<String>,
    can_restart_elevated: bool,
    restored_refinement: String,
    pending_resume_run_id: Option<u64>,
    update: crate::update::Status,
}

pub struct AppState {
    config: Mutex<ConfigStore>,
    view: Mutex<RunView>,
    active: Mutex<Option<Arc<AtomicBool>>>,
    ui_at: AtomicU64,
    next_id: AtomicU64,
    requests: Mutex<HashMap<String, oneshot::Sender<()>>>,
    speech: Mutex<Option<Arc<AtomicBool>>>,
    training_paused: Arc<AtomicBool>,
    session: Mutex<Session>,
    restored_refinement: Mutex<String>,
    pending_resume: Mutex<Option<u64>>,
    workflows: Mutex<WorkflowStore>,
    drafts: Mutex<HashMap<String, PreparedWorkflow>>,
    update: Mutex<crate::update::Status>,
    update_cancel: Mutex<Option<Arc<AtomicBool>>>,
}
impl AppState {
    fn fail_resume_setup(&self, run_id: u64, source: &str, message: &str) -> bool {
        let mut active = self.active.lock().unwrap();
        if active.is_some() {
            return false;
        }
        let mut view = self.view.lock().unwrap();
        if view.id != run_id || view.phase != "recovering" {
            return false;
        }
        self.pending_resume.lock().unwrap().take();
        view.phase = "error".into();
        view.message = message.into();
        view.recovery_event(
            "automatic_resume_failed",
            source,
            None,
            Some(crate::diagnostics::InputFailure::new(
                "resume_setup_failed",
                message,
            )),
        );
        // Reserve presentation so a late startup callback cannot race a new run.
        *active = Some(Arc::new(AtomicBool::new(true)));
        true
    }

    fn snapshot(&self) -> Snapshot {
        let (settings, has_api_key, config_path, config_error) = {
            let config = self.config.lock().unwrap();
            (
                config.settings.public(),
                !config.settings.api_key.is_empty(),
                config.path.to_string_lossy().into(),
                config.error.clone(),
            )
        };
        let run = self.view.lock().unwrap().clone();
        let (workflows, workflow_error) = {
            let store = self.workflows.lock().unwrap();
            (
                store.workflows.iter().map(Workflow::summary).collect(),
                store.error.clone(),
            )
        };
        Snapshot {
            settings,
            has_api_key,
            config_path,
            config_error,
            can_restart_elevated: crate::recovery::can_restart(&run),
            restored_refinement: self.restored_refinement.lock().unwrap().clone(),
            pending_resume_run_id: *self.pending_resume.lock().unwrap(),
            run,
            platform: "windows",
            activity_busy: self.active.lock().unwrap().is_some(),
            locale: if unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() }
                & 0x03ff
                == 7
            {
                "de"
            } else {
                "en"
            },
            workflows,
            workflow_error,
            update: self.update.lock().unwrap().clone(),
        }
    }
    fn draft(&self, mut settings: Settings, key: Option<String>) -> Result<Settings, String> {
        let store = self.config.lock().unwrap();
        settings.api_key = key.unwrap_or_else(|| {
            if same_origin(&settings.base_url, &store.settings.base_url) {
                store.settings.api_key.clone()
            } else {
                String::new()
            }
        });
        settings.validate()?;
        Ok(settings)
    }
}
fn emit(app: &tauri::AppHandle) {
    let _ = app.emit("state", app.state::<AppState>().snapshot());
}
fn progress(app: &tauri::AppHandle, phase: &str, message: impl Into<String>, started: Instant) {
    {
        let state = app.state::<AppState>();
        let mut view = state.view.lock().unwrap();
        view.phase = phase.into();
        view.message = message.into();
        view.elapsed_ms = started.elapsed().as_millis() as u64;
    }
    emit(app);
}

#[tauri::command]
fn bootstrap(state: tauri::State<AppState>) -> Snapshot {
    state.snapshot()
}
#[tauri::command]
fn ui_heartbeat(state: tauri::State<AppState>) {
    state.ui_at.store(platform::now(), Ordering::SeqCst);
}

fn update_owns_activity(active: &Option<Arc<AtomicBool>>, cancel: &Arc<AtomicBool>) -> bool {
    active
        .as_ref()
        .is_some_and(|owner| Arc::ptr_eq(owner, cancel))
}

#[tauri::command]
fn skip_update(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    // This lock also protects the final file replacement: skip can never race a restart.
    let mut active = state.active.lock().unwrap();
    let mut update = state.update.lock().unwrap();
    if update.phase == "installing" {
        return Err("The update is restarting klickwerk.".into());
    }
    if let Some(cancel) = state.update_cancel.lock().unwrap().as_ref() {
        cancel.store(true, Ordering::SeqCst);
        if update_owns_activity(&active, cancel) {
            *active = None;
        }
    }
    update.phase = "skipped";
    update.startup = false;
    update.message = None;
    drop(update);
    drop(active);
    emit(&app);
    Ok(())
}

#[tauri::command]
fn check_updates(app: tauri::AppHandle) -> Result<(), String> {
    begin_update(&app, false, false)
}

fn begin_update(app: &tauri::AppHandle, install: bool, updated: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut active = state.active.lock().unwrap();
    let mut status = state.update.lock().unwrap();
    let mut job = state.update_cancel.lock().unwrap();
    if job.is_some() {
        return Err("An update check is already running.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    if install {
        if active.is_some()
            || state.speech.lock().unwrap().is_some()
            || state.view.lock().unwrap().phase != "idle"
        {
            return Err("Finish the current task before updating.".into());
        }
        *active = Some(cancel.clone());
    }
    *status = crate::update::Status::new(install);
    *job = Some(cancel.clone());
    drop(job);
    drop(status);
    drop(active);
    emit(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = tokio::select! {
            biased;
            _ = crate::update::wait_cancel(&cancel) => Err(crate::update::CANCELLED.into()),
            result = perform_update(&app, &cancel, install, updated) => result,
        };
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        let mut status = state.update.lock().unwrap();
        if let Err(error) = result
            && !cancel.load(Ordering::SeqCst)
        {
            status.phase = "error";
            status.message = Some(error);
        }
        status.startup = false;
        if update_owns_activity(&active, &cancel) {
            *active = None;
        }
        state.update_cancel.lock().unwrap().take();
        drop(status);
        drop(active);
        emit(&app);
    });
    Ok(())
}

async fn perform_update(
    app: &tauri::AppHandle,
    cancel: &Arc<AtomicBool>,
    install: bool,
    updated: bool,
) -> Result<(), String> {
    use crate::update;
    let client = update::client()?;
    let manifest = update::check(&client, cancel).await?;
    let current = std::env::current_exe().map_err(|_| update::CHECK_FAILED)?;
    let hash = update::file_hash(&current, cancel)?;
    let state = app.state::<AppState>();
    {
        let mut status = state.update.lock().unwrap();
        if cancel.load(Ordering::SeqCst) {
            return Err(update::CANCELLED.into());
        }
        status.latest = Some(manifest.tag.clone());
        status.total = manifest.size;
        if !manifest.newer_than(update::RELEASE_TAG, &hash) {
            status.phase = if updated { "updated" } else { "current" };
            return Ok(());
        }
        status.phase = if install { "downloading" } else { "available" };
    }
    emit(app);
    if !install {
        return Ok(());
    }
    let last_progress = Mutex::new(Instant::now() - Duration::from_secs(1));
    let staged = update::download(&client, &manifest, &current, cancel, |bytes| {
        let mut last = last_progress.lock().unwrap();
        if last.elapsed() < Duration::from_millis(100) && bytes < manifest.size {
            return;
        }
        *last = Instant::now();
        let mut status = state.update.lock().unwrap();
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        status.downloaded = bytes;
        drop(status);
        emit(app);
    })
    .await?;
    // A skipped update immediately releases the composer. Never restart after that point.
    let active = state.active.lock().unwrap();
    if cancel.load(Ordering::SeqCst) || !update_owns_activity(&active, cancel) {
        return Err(update::CANCELLED.into());
    }
    state.update.lock().unwrap().phase = "installing";
    // The installing phase now rejects skip; release the lock before snapshot emission.
    drop(active);
    emit(app);
    update::install(&current, staged, platform::update::launch)?;
    app.exit(0);
    Ok(())
}
#[tauri::command]
fn save_settings(
    settings: Settings,
    key: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Snapshot, String> {
    let active = state.active.lock().unwrap();
    if active.is_some() {
        return Err("Stop the current task before changing Settings.".into());
    }
    state.config.lock().unwrap().save(settings, key)?;
    drop(active);
    Ok(state.snapshot())
}
#[tauri::command]
fn cancel_request(request_id: String, state: tauri::State<AppState>) {
    if let Some(cancel) = state.requests.lock().unwrap().remove(&request_id) {
        let _ = cancel.send(());
    }
}

async fn network<T>(
    app: tauri::AppHandle,
    id: String,
    future: impl Future<Output = Result<T, String>>,
) -> Result<T, String> {
    if id.len() > 100 {
        return Err("Invalid request identifier.".into());
    }
    let (send, receive) = oneshot::channel();
    {
        let state = app.state::<AppState>();
        let mut requests = state.requests.lock().unwrap();
        if requests.len() >= 8 {
            return Err("Too many connection requests. Wait a moment and retry.".into());
        }
        requests.insert(id.clone(), send);
    }
    let result =
        tokio::select! {result=future=>result,_=receive=>Err("Connection check cancelled.".into())};
    app.state::<AppState>().requests.lock().unwrap().remove(&id);
    result
}
#[tauri::command]
async fn list_models(
    app: tauri::AppHandle,
    settings: Settings,
    key: Option<String>,
    request_id: String,
) -> Result<ModelList, String> {
    let settings = app.state::<AppState>().draft(settings, key)?;
    let provider = Provider::new(&settings)?;
    network(app, request_id, provider.models()).await
}
#[tauri::command]
async fn test_connection(
    app: tauri::AppHandle,
    settings: Settings,
    key: Option<String>,
    request_id: String,
) -> Result<String, String> {
    let settings = app.state::<AppState>().draft(settings, key)?;
    let provider = Provider::new(&settings)?;
    network(app, request_id, provider.check()).await
}
#[tauri::command]
async fn stop_task(app: tauri::AppHandle) {
    let pending_stopped = {
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        if let Some(cancel) = active.as_ref() {
            cancel.store(true, Ordering::SeqCst);
        }
        let pending_stopped = state.pending_resume.lock().unwrap().take().is_some();
        if pending_stopped {
            let mut view = state.view.lock().unwrap();
            view.phase = "stopped".into();
            view.message = crate::recovery::RESUME_CANCELLED.into();
            view.recovery_event("automatic_resume_cancelled", "stop_button", None, None);
            *active = Some(Arc::new(AtomicBool::new(true)));
        }
        pending_stopped
    };
    emit(&app);
    if pending_stopped {
        return_control(&app).await;
    }
}

#[tauri::command]
fn start_task(
    app: tauri::AppHandle,
    task: String,
    reply: Option<String>,
    resume_run_id: Option<u64>,
    workflow_id: Option<String>,
) -> Result<(), String> {
    start_task_inner(app, task, reply, resume_run_id, workflow_id, false)
}

fn start_task_inner(
    app: tauri::AppHandle,
    task: String,
    reply: Option<String>,
    resume_run_id: Option<u64>,
    workflow_id: Option<String>,
    from_uac: bool,
) -> Result<(), String> {
    if task.trim().is_empty() || task.len() > 32768 {
        return Err("Enter a task of at most 8,192 characters.".into());
    }
    let state = app.state::<AppState>();
    if state.speech.lock().unwrap().is_some() {
        return Err("Finish dictation before starting the task.".into());
    }
    let mut active = state.active.lock().unwrap();
    if from_uac && *state.pending_resume.lock().unwrap() != resume_run_id {
        return Ok(());
    }
    if active.is_some() {
        return Err("A task is already running.".into());
    }
    if from_uac {
        let mut pending = state.pending_resume.lock().unwrap();
        if resume_run_id.is_none() || *pending != resume_run_id {
            return Ok(());
        }
        *pending = None;
    } else if state.pending_resume.lock().unwrap().is_some() {
        return Err(
            "The restored task is preparing to continue. Stop it before starting another task."
                .into(),
        );
    }
    let settings = {
        let store = state.config.lock().unwrap();
        if let Some(error) = &store.error {
            return Err(error.clone());
        }
        store.settings.ready()?;
        store.settings.clone()
    };
    let mut view = state.view.lock().unwrap();
    let first_step_id = if resume_run_id.is_some() {
        view.steps.len() as u32 + 1
    } else {
        1
    };
    let user_reply = reply.clone();
    if let Some(id) = resume_run_id {
        if view.id != id
            || view.task != task
            || (!from_uac
                && !["stopped", "waiting", "error", "done"].contains(&view.phase.as_str()))
            || (from_uac && view.phase != "recovering")
        {
            return Err(
                "This correction no longer belongs to the current task. Start a new task.".into(),
            );
        }
        if view.steps.len() >= workflow::MAX_STEPS - 2 {
            return Err(
                "This session is full. Save it as a workflow and start a fresh run.".into(),
            );
        }
        let answer = reply.unwrap_or_default();
        let elapsed = view.elapsed_ms;
        if answer.trim().is_empty() {
            if view.phase == "waiting" || view.phase == "done" {
                return Err("Enter an answer or refinement before continuing.".into());
            }
            let step_id = view.steps.len() as u32 + 1;
            view.steps.push(Step::note(step_id, "system",
                if from_uac { "Windows administrator access was approved for this task. Continue from the current desktop after a fresh observation; do not replay old input." } else { "The user explicitly chose to continue without a correction. Inspect the current desktop and verify the last action before proceeding; do not repeat input blindly." }, elapsed));
        } else {
            workflow::add_correction(&mut view.steps, &answer, elapsed)?;
        }
    } else {
        if reply.is_some() {
            return Err("A correction must refer to its original task.".into());
        }
        let mut session = Session::default();
        if let Some(id) = &workflow_id {
            let saved = state.workflows.lock().unwrap().get(id)?;
            session.memory = saved.learning.prompt;
        }
        *state.session.lock().unwrap() = session;
        *view = RunView {
            id: state.next_id.fetch_add(1, Ordering::SeqCst) + 1,
            task: task.clone(),
            workflow_id,
            ..RunView::default()
        };
    }
    state.drafts.lock().unwrap().clear();
    state.restored_refinement.lock().unwrap().clear();
    view.begin_attempt(
        &settings,
        first_step_id,
        user_reply,
        &state.session.lock().unwrap().memory,
    );
    if view.recovery.take().is_some() || !view.recovery_events.is_empty() {
        view.recovery_event(
            if from_uac {
                "automatic_resume_started"
            } else {
                "resume_requested"
            },
            if from_uac {
                "ui_ready"
            } else {
                "user_continue"
            },
            platform::window::privileges(std::process::id())
                .ok()
                .map(|p| p.0),
            None,
        );
    }
    view.phase = "checking".into();
    view.interrupted = false;
    view.question.clear();
    view.result.clear();
    view.message = "Getting ready to help…".into();
    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(cancel.clone());
    state.ui_at.store(platform::now(), Ordering::SeqCst);
    drop(view);
    drop(active);
    emit(&app);
    tauri::async_runtime::spawn(async move {
        controller(app, settings, task, cancel).await;
    });
    Ok(())
}

#[tauri::command]
async fn resume_after_restart(app: tauri::AppHandle, run_id: u64) -> Result<(), String> {
    let (task, refinement) = {
        let state = app.state::<AppState>();
        if *state.pending_resume.lock().unwrap() != Some(run_id) {
            return Ok(());
        }
        (
            state.view.lock().unwrap().task.clone(),
            state.restored_refinement.lock().unwrap().clone(),
        )
    };
    let result = start_task_inner(
        app.clone(),
        task,
        if refinement.is_empty() {
            None
        } else {
            Some(refinement)
        },
        Some(run_id),
        None,
        true,
    );
    if let Err(message) = &result
        && app
            .state::<AppState>()
            .fail_resume_setup(run_id, "ui_ready", message)
    {
        return_control(&app).await;
    }
    result
}

#[tauri::command]
async fn restart_as_administrator(
    app: tauri::AppHandle,
    run_id: u64,
    refinement: String,
) -> Result<(), String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .ok_or(crate::recovery::RESTART_FAILED)?
        .0 as usize;
    let session = {
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        if active.is_some() || state.speech.lock().unwrap().is_some() {
            return Err("Finish the current activity before restarting klickwerk.".into());
        }
        let mut view = state.view.lock().unwrap();
        if view.id != run_id {
            return Err("This session is not available for an administrator restart.".into());
        }
        let mut session = crate::recovery::RestartSession::new(
            view.clone(),
            state.session.lock().unwrap().memory.clone(),
            refinement,
        )?;
        view.recovery_event(
            "elevation_requested",
            "user_restart",
            platform::window::privileges(std::process::id())
                .ok()
                .map(|p| p.0),
            None,
        );
        session.run = view.clone();
        // Reserve the existing activity gate while UAC and the transfer are pending.
        *active = Some(Arc::new(AtomicBool::new(false)));
        session
    };
    let cancel = app
        .state::<AppState>()
        .active
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    perform_restart(app, owner, session, cancel, "user_restart").await
}

async fn perform_restart(
    app: tauri::AppHandle,
    owner: usize,
    session: crate::recovery::RestartSession,
    cancel: Arc<AtomicBool>,
    source: &'static str,
) -> Result<(), String> {
    let worker_cancel = cancel.clone();
    let result = tokio::task::spawn_blocking(move || {
        platform::restart::launch(owner, session, &worker_cancel)
    })
    .await
    .unwrap_or_else(|_| {
        Err(crate::diagnostics::InputFailure::new(
            "restart_worker_failed",
            crate::recovery::RESTART_FAILED,
        ))
    });
    // Serialize the final cancellation decision and exit request with Stop.
    // The child cannot continue from receipt alone, even if this app closes later.
    let result = result.and_then(|restart| {
        let state = app.state::<AppState>();
        let _active = state.active.lock().unwrap();
        let committed = restart.commit(&cancel);
        if committed.is_ok() {
            app.exit(0);
        }
        committed
    });
    match result {
        Ok(()) => Ok(()),
        Err(failure) => {
            {
                let state = app.state::<AppState>();
                let _active = state.active.lock().unwrap();
                let mut view = state.view.lock().unwrap();
                view.phase = "stopped".into();
                view.message = failure.message.clone();
                view.recovery_event(
                    "elevation_failed",
                    source,
                    platform::window::privileges(std::process::id())
                        .ok()
                        .map(|p| p.0),
                    Some(failure.clone()),
                );
            }
            return_control(&app).await;
            Err(failure.message)
        }
    }
}

#[tauri::command]
fn import_workflow(contents: String) -> Result<Learning, String> {
    workflow::PortableWorkflow::decode(contents.as_bytes())
}

#[tauri::command]
async fn export_workflow(app: tauri::AppHandle, id: String) -> Result<Option<String>, String> {
    let (bytes, filename, german) = {
        let state = app.state::<AppState>();
        let active = state.active.lock().unwrap();
        if active.is_some() {
            return Err("Finish the current activity before exporting workflows.".into());
        }
        let workflow = state.workflows.lock().unwrap().get(&id)?;
        (
            workflow::PortableWorkflow::encode(&workflow.learning)?,
            format!("{}.json", workflow::slug(&workflow.learning.name)),
            state.config.lock().unwrap().settings.ui_language() == "German",
        )
    };
    let owner = app
        .get_webview_window("main")
        .ok_or("The main window is unavailable.")?
        .hwnd()
        .map_err(|_| "The main window is unavailable.")?
        .0 as usize;
    tokio::task::spawn_blocking(move || {
        let Some(path) = platform::export::choose_workflow_path(owner, &filename, german)? else {
            return Ok(None);
        };
        crate::session::write_report(&path, &bytes)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|_| "The export worker stopped unexpectedly.".to_owned())?
}

#[tauri::command]
fn pause_training(app: tauri::AppHandle, paused: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_none()
        || !["training", "training_paused"].contains(&state.view.lock().unwrap().phase.as_str())
    {
        return Err("No demonstration is being recorded.".into());
    }
    state.training_paused.store(paused, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn start_training(
    app: tauri::AppHandle,
    task: String,
    run_id: Option<u64>,
    workflow_id: Option<String>,
    correction: Option<String>,
    options: crate::training::Options,
) -> Result<(), String> {
    if task.trim().is_empty() || task.len() > 32768 {
        return Err("Enter a task of at most 8,192 characters.".into());
    }
    let state = app.state::<AppState>();
    let mut active = state.active.lock().unwrap();
    if active.is_some()
        || state.speech.lock().unwrap().is_some()
        || state.pending_resume.lock().unwrap().is_some()
    {
        return Err("Finish the current activity before recording a demonstration.".into());
    }
    let mut view = state.view.lock().unwrap();
    if let Some(id) = run_id {
        if view.id != id
            || view.task != task
            || !["stopped", "done", "error", "waiting"].contains(&view.phase.as_str())
        {
            return Err("This task is no longer available to save.".into());
        }
        if let Some(text) = correction.filter(|t| !t.trim().is_empty()) {
            let elapsed = view.elapsed_ms;
            workflow::add_correction(&mut view.steps, &text, elapsed)?;
        }
    } else {
        let memory = workflow_id
            .as_ref()
            .map(|id| {
                state
                    .workflows
                    .lock()
                    .unwrap()
                    .get(id)
                    .map(|w| w.learning.prompt)
            })
            .transpose()?
            .unwrap_or_default();
        *view = RunView {
            id: state.next_id.fetch_add(1, Ordering::SeqCst) + 1,
            task: task.trim().into(),
            workflow_id,
            started_at: crate::session::timestamp(),
            ..RunView::default()
        };
        *state.session.lock().unwrap() = Session { memory };
    }
    // Re-recording replaces the previous demonstration, retaining actual run history.
    Arc::make_mut(&mut view.evidence).training = None;
    view.training = Some(crate::training::Summary::default());
    view.phase = "training".into();
    view.message = "Show the task in your apps. Finish with Ctrl + Shift + F9.".into();
    view.interrupted = false;
    view.question.clear();
    view.result.clear();
    view.recovery = None;
    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(cancel.clone());
    state.training_paused.store(false, Ordering::SeqCst);
    let paused = state.training_paused.clone();
    let german = state.config.lock().unwrap().settings.ui_language() == "German";
    state.drafts.lock().unwrap().clear();
    // Reserve a new revision even when the same run is recorded again.
    state.next_id.fetch_add(1, Ordering::SeqCst);
    drop(view);
    drop(active);
    emit(&app);
    tauri::async_runtime::spawn(async move {
        let worker_app = app.clone();
        let report = tokio::task::spawn_blocking(move || {
            platform::training::record(
                options,
                german,
                cancel,
                paused,
                || {
                    platform::now()
                        .saturating_sub(worker_app.state::<AppState>().ui_at.load(Ordering::SeqCst))
                        < 5000
                },
                |summary, paused| {
                    {
                        let state = worker_app.state::<AppState>();
                        let mut view = state.view.lock().unwrap();
                        view.training = Some(summary);
                        view.phase = if paused {
                            "training_paused"
                        } else {
                            "training"
                        }
                        .into();
                    }
                    emit(&worker_app);
                },
            )
        })
        .await;
        {
            let state = app.state::<AppState>();
            let mut view = state.view.lock().unwrap();
            view.phase = "stopped".into();
            match report {
                Ok(report) => {
                    view.message = report.stop_reason.clone();
                    if report.events.is_empty() && report.stop_reason != "Demonstration recorded." {
                        view.phase = "error".into();
                        view.training = None;
                    } else {
                        view.training = Some(report.summary());
                    }
                    Arc::make_mut(&mut view.evidence).training = Some(report);
                }
                Err(_) => {
                    view.message = "The demonstration recorder stopped unexpectedly.".into();
                    view.training = None;
                }
            }
        }
        // This is a recording handoff, not another outcome of the last agent
        // attempt. Do not overwrite that attempt's execution diagnostics.
        let _ = handoff(&app).await;
        *app.state::<AppState>().active.lock().unwrap() = None;
        emit(&app);
    });
    Ok(())
}

#[tauri::command]
fn get_workflow(id: String, state: tauri::State<AppState>) -> Result<Workflow, String> {
    state.workflows.lock().unwrap().get(&id)
}

#[tauri::command]
async fn export_session(
    app: tauri::AppHandle,
    run_id: u64,
    refinement: Option<String>,
) -> Result<Option<String>, String> {
    let (report, german) = {
        let state = app.state::<AppState>();
        let active = state.active.lock().unwrap();
        if active.is_some() {
            return Err("Pause the task before exporting its history.".into());
        }
        let run = state.view.lock().unwrap().clone();
        let workflow = run
            .workflow_id
            .as_ref()
            .and_then(|id| state.workflows.lock().unwrap().get(id).ok());
        let memory = state.session.lock().unwrap().memory.clone();
        let german = state.config.lock().unwrap().settings.ui_language() == "German";
        (
            SessionExport::new(run, run_id, workflow, memory, refinement)?,
            german,
        )
    };
    let window = app
        .get_webview_window("main")
        .ok_or("The main window is unavailable.")?;
    let owner = window
        .hwnd()
        .map_err(|_| "The main window is unavailable.")?
        .0 as usize;
    tokio::task::spawn_blocking(move || {
        let Some(path) = platform::export::choose_path(owner, &report.filename(), german)? else {
            return Ok(None);
        };
        report.write(&path)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|_| "The export worker stopped unexpectedly.".to_owned())?
}

#[derive(Serialize)]
struct WorkflowDraft {
    token: String,
    workflow: Workflow,
    warning: Option<String>,
}

#[derive(Clone)]
struct PreparedWorkflow {
    run_id: Option<u64>,
    source_steps: usize,
    source_workflow: Option<(String, u64)>,
    correction: Option<String>,
    source_training_events: usize,
    generation: u64,
    workflow: Workflow,
}

fn validate_draft(state: &AppState, draft: &PreparedWorkflow) -> Result<(), String> {
    if state.next_id.load(Ordering::SeqCst) != draft.generation {
        return Err("This session changed. Save again to include the latest changes.".into());
    }
    if let Some(run_id) = draft.run_id {
        let view = state.view.lock().unwrap();
        if view.id != run_id
            || view.workflow_id.as_ref() != draft.source_workflow.as_ref().map(|(id, _)| id)
            || view.steps.len() != draft.source_steps
            || view.training.as_ref().map(|t| t.events).unwrap_or(0) != draft.source_training_events
            || !["stopped", "done", "waiting", "error"].contains(&view.phase.as_str())
        {
            return Err("This session changed. Save again to include the latest changes.".into());
        }
    }
    if let Some((id, updated_at)) = &draft.source_workflow
        && state.workflows.lock().unwrap().get(id)?.updated_at != *updated_at
    {
        return Err("This workflow changed. Open it again before saving.".into());
    }
    Ok(())
}

#[tauri::command]
async fn prepare_workflow(
    app: tauri::AppHandle,
    run_id: Option<u64>,
    id: Option<String>,
    correction: Option<String>,
    learning: Learning,
    request_id: String,
    consolidate: Option<bool>,
) -> Result<WorkflowDraft, String> {
    learning.validate()?;
    let correction = correction.filter(|s| !s.trim().is_empty());
    let (mut draft, task, memory, steps, outcome, settings, training) = {
        let state = app.state::<AppState>();
        let active = state.active.lock().unwrap();
        if active.is_some() {
            return Err("Take over before saving this workflow.".into());
        }
        let view = state.view.lock().unwrap().clone();
        if run_id.is_some_and(|id| id != view.id)
            || (run_id.is_some()
                && !["stopped", "done", "waiting", "error"].contains(&view.phase.as_str()))
        {
            return Err("This task is no longer available to save.".into());
        }
        let saved_id = if run_id.is_some() {
            view.workflow_id.clone()
        } else {
            id
        };
        let saved = saved_id
            .as_ref()
            .map(|id| state.workflows.lock().unwrap().get(id))
            .transpose()?;
        let source_steps = if run_id.is_some() {
            view.steps.len()
        } else {
            0
        };
        let mut steps = if run_id.is_some() {
            view.steps.clone()
        } else {
            vec![]
        };
        if let Some(text) = &correction {
            workflow::add_correction(&mut steps, text, view.elapsed_ms)?;
        }
        let memory = if run_id.is_some() {
            state.session.lock().unwrap().memory.clone()
        } else {
            saved
                .as_ref()
                .map(|w| w.learning.prompt.clone())
                .unwrap_or_default()
        };
        let task = if run_id.is_some() {
            view.task.clone()
        } else {
            saved
                .as_ref()
                .map(|w| w.learning.prompt.clone())
                .unwrap_or_else(|| learning.prompt.clone())
        };
        let outcome = if run_id.is_some() {
            format!(
                "Phase: {}\nMessage: {}\nResult: {}\nOpen question: {}\nUser takeover: {}",
                view.phase, view.message, view.result, view.question, view.interrupted
            )
        } else {
            "Editing saved instructions; no new run has been performed.".into()
        };
        let draft = PreparedWorkflow {
            run_id,
            source_steps,
            source_training_events: if run_id.is_some() {
                view.training.as_ref().map(|t| t.events).unwrap_or(0)
            } else {
                0
            },
            generation: state.next_id.load(Ordering::SeqCst),
            source_workflow: saved.as_ref().map(|w| (w.id.clone(), w.updated_at)),
            correction,
            workflow: Workflow {
                id: saved_id.unwrap_or_else(|| {
                    format!("workflow-{}-{}", platform::now(), run_id.unwrap_or(0))
                }),
                learning: learning.clone(),
                updated_at: 0,
            },
        };
        let settings = state.config.lock().unwrap().settings.clone();
        let training = if run_id.is_some() {
            view.evidence.training.clone()
        } else {
            None
        };
        (draft, task, memory, steps, outcome, settings, training)
    };
    let result = if consolidate.unwrap_or(true) {
        match Provider::new(&settings) {
            Ok(provider) => {
                network(
                    app.clone(),
                    request_id.clone(),
                    provider.learn_with_training(
                        &task,
                        &memory,
                        &steps,
                        &outcome,
                        &learning,
                        training.as_ref(),
                    ),
                )
                .await
            }
            Err(error) => Err(error),
        }
    } else {
        Ok(workflow::fallback_prompt("", &steps, &learning))
    };
    let warning = match result {
        Ok(learned) => {
            draft.workflow.learning = learned;
            None
        }
        Err(error) => {
            draft.workflow.learning = workflow::fallback_prompt(&memory, &steps, &learning);
            draft.workflow.learning.validate()?;
            Some(error)
        }
    };
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_some() {
        return Err("The task resumed. Take over before saving.".into());
    }
    validate_draft(&state, &draft)?;
    let workflow = draft.workflow.clone();
    let mut drafts = state.drafts.lock().unwrap();
    if drafts.len() >= 8 {
        drafts.clear();
    }
    drafts.insert(request_id.clone(), draft);
    Ok(WorkflowDraft {
        token: request_id,
        workflow,
        warning,
    })
}

#[tauri::command]
fn save_workflow(
    app: tauri::AppHandle,
    token: String,
    learning: Option<Learning>,
) -> Result<Workflow, String> {
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_some() {
        return Err("Take over before editing workflows.".into());
    }
    let mut source = state
        .drafts
        .lock()
        .unwrap()
        .get(&token)
        .cloned()
        .ok_or("This workflow draft has expired. Prepare it again.")?;
    validate_draft(&state, &source)?;
    if let Some(learning) = learning {
        learning.validate()?;
        source.workflow.learning = learning;
    }
    let saved = state.workflows.lock().unwrap().save(source.workflow)?;
    if source.run_id.is_some() {
        let mut view = state.view.lock().unwrap();
        if let Some(correction) = source.correction {
            let elapsed = view.elapsed_ms;
            workflow::add_correction(&mut view.steps, &correction, elapsed)?;
        }
        view.workflow_id = Some(saved.id.clone());
        state.session.lock().unwrap().memory = saved.learning.prompt.clone();
    }
    state.drafts.lock().unwrap().remove(&token);
    drop(active);
    emit(&app);
    Ok(saved)
}

fn clear_session(state: &AppState) {
    state.next_id.fetch_add(1, Ordering::SeqCst);
    state.pending_resume.lock().unwrap().take();
    state.restored_refinement.lock().unwrap().clear();
    *state.view.lock().unwrap() = RunView::default();
    *state.session.lock().unwrap() = Session::default();
    state.drafts.lock().unwrap().clear();
}

#[tauri::command]
fn reset_session(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_some() || state.speech.lock().unwrap().is_some() {
        return Err("Finish the current activity before starting a fresh session.".into());
    }
    clear_session(&state);
    drop(active);
    emit(&app);
    Ok(())
}

#[tauri::command]
fn delete_workflow(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_some() || state.speech.lock().unwrap().is_some() {
        return Err("Finish the current activity before deleting workflows.".into());
    }
    state.workflows.lock().unwrap().delete(&id)?;
    clear_session(&state);
    drop(active);
    emit(&app);
    Ok(())
}

#[tauri::command]
fn start_dictation(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let active = state.active.lock().unwrap();
    if active.is_some() {
        return Err("Stop the task before using the microphone.".into());
    }
    let mut speech = state.speech.lock().unwrap();
    if speech.is_some() {
        return Err("Dictation is already running.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *speech = Some(cancel.clone());
    drop(speech);
    drop(active);
    std::thread::spawn(move || {
        platform::speech::dictate(cancel, |update| {
            let _ = app.emit("speech", update);
        });
        *app.state::<AppState>().speech.lock().unwrap() = None;
    });
    Ok(())
}
#[tauri::command]
async fn stop_dictation(app: tauri::AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        if let Some(cancel) = state.speech.lock().unwrap().as_ref() {
            cancel.store(true, Ordering::SeqCst);
        }
    }
    for _ in 0..30 {
        if app.state::<AppState>().speech.lock().unwrap().is_none() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Err("The microphone is still closing. Wait a moment before starting a task.".into())
}

struct BrokerClient {
    child: Child,
    sender: mpsc::SyncSender<Command>,
    receiver: async_mpsc::UnboundedReceiver<Reply>,
    held: HeldInput,
    bounds: [i32; 4],
    ready: bool,
    created: Instant,
}
impl BrokerClient {
    fn spawn(simulate: bool) -> Result<Self, String> {
        let mut random = [0u8; 16];
        if unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                random.as_mut_ptr(),
                16,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        } != 0
        {
            return Err("The safety channel could not be secured.".into());
        }
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let mapping = format!("Local\\klickwerk-{}-{suffix}", std::process::id());
        let held = HeldInput::open(&mapping, true)?;
        let mut child = std::process::Command::new(
            std::env::current_exe().map_err(|_| "The app executable could not be located.")?,
        )
        .args([
            "--stop-broker",
            &std::process::id().to_string(),
            &mapping,
            if simulate { "simulate" } else { "live" },
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if cfg!(feature = "safety-tests") {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| "The independent input monitor process could not start.")?;
        let mut input = child
            .stdin
            .take()
            .ok_or("The safety input channel is unavailable.")?;
        let output = child
            .stdout
            .take()
            .ok_or("The safety response channel is unavailable.")?;
        let (sender, commands) = mpsc::sync_channel::<Command>(4);
        std::thread::spawn(move || {
            while let Ok(command) = commands.recv() {
                let Ok(mut bytes) = serde_json::to_vec(&command) else {
                    break;
                };
                bytes.push(b'\n');
                if input.write_all(&bytes).and_then(|_| input.flush()).is_err() {
                    break;
                }
            }
        });
        let (replies, receiver) = async_mpsc::unbounded_channel();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(output);
            loop {
                let mut line = Vec::new();
                if !matches!(std::io::Read::by_ref(&mut reader).take(65537).read_until(b'\n',&mut line),Ok(n)if n>0&&n<=65536)
                {
                    break;
                }
                let Ok(reply) = serde_json::from_slice::<Reply>(&line) else {
                    break;
                };
                if replies.send(reply).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            sender,
            receiver,
            held,
            bounds: [0; 4],
            ready: false,
            created: Instant::now(),
        })
    }
    fn stopped(
        app: &tauri::AppHandle,
        reason: String,
        interruption: Option<crate::diagnostics::InputInterruption>,
    ) -> End {
        if let Some(interruption) = interruption {
            app.state::<AppState>()
                .view
                .lock()
                .unwrap()
                .record_interruption(interruption);
        }
        End::Stopped(reason)
    }

    fn send(&self, command: Command) -> Result<(), End> {
        self.sender
            .try_send(command)
            .map_err(|_| End::Stopped("The input monitor connection is unavailable.".into()))
    }
    fn heartbeat(
        &self,
        app: &tauri::AppHandle,
        cancel: &AtomicBool,
        lease: Option<u64>,
    ) -> Result<(), End> {
        if self.created.elapsed() > Duration::from_secs(600) {
            return Err(End::Stopped(
                "The ten-minute task limit was reached.".into(),
            ));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(End::Stopped(
                "Stopped. Your mouse and keyboard are yours again.".into(),
            ));
        }
        if platform::now().saturating_sub(app.state::<AppState>().ui_at.load(Ordering::SeqCst))
            > 3000
        {
            return Err(End::Stopped(
                "Stopped because the app interface stopped responding.".into(),
            ));
        }
        self.send(Command::Heartbeat {
            sent_ms: platform::now(),
            lease_sequence: lease,
        })
    }
    async fn signal(
        &mut self,
        app: &tauri::AppHandle,
        cancel: &AtomicBool,
        lease: Option<u64>,
    ) -> Result<Reply, End> {
        let mut timer = tokio::time::interval(Duration::from_millis(100));
        let waiting_since = Instant::now();
        loop {
            tokio::select! {
                _=timer.tick()=>{
                    if self.ready { self.heartbeat(app,cancel,lease)?; }
                    else if cancel.load(Ordering::SeqCst) || waiting_since.elapsed()>Duration::from_secs(5) {return Err(End::Stopped("The input monitor startup was cancelled or timed out.".into()));}
                },
                reply=self.receiver.recv()=>return match reply{
                    Some(Reply::Stopped{reason, interruption})=>Err(Self::stopped(app, reason, interruption)),Some(Reply::Error{message})=>Err(End::Failed(message)),Some(value)=>{if matches!(value,Reply::Ready{..}){self.ready=true;}Ok(value)},None=>Err(End::Stopped("The input monitor process exited. Input has been released.".into()))
                }
            }
        }
    }
    async fn during<T>(
        &mut self,
        app: &tauri::AppHandle,
        cancel: &AtomicBool,
        future: impl Future<Output = Result<T, String>>,
    ) -> Result<T, End> {
        tokio::pin!(future);
        let mut timer = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                result=&mut future=>{self.heartbeat(app,cancel,None)?;return result.map_err(End::Failed);},
                _=timer.tick()=>self.heartbeat(app,cancel,None)?,
                reply=self.receiver.recv()=>return Err(match reply{Some(Reply::Stopped{reason, interruption})=>Self::stopped(app, reason, interruption),Some(Reply::Error{message})=>End::Failed(message),_=>End::Stopped("The input monitor connection ended.".into())})
            }
        }
    }
}
impl Drop for BrokerClient {
    fn drop(&mut self) {
        let _ = self.sender.try_send(Command::Stop);
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                self.held.release();
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.held.release();
    }
}
enum End {
    Stopped(String),
    Failed(String),
    Question(String),
    Blocked(crate::diagnostics::InputFailure, &'static str),
    Done(String),
}

fn record(app: &tauri::AppHandle, step: Step) {
    app.state::<AppState>()
        .view
        .lock()
        .unwrap()
        .steps
        .push(step);
    emit(app);
}
fn action_status(app: &tauri::AppHandle, status: &str) {
    if let Some(step) = app
        .state::<AppState>()
        .view
        .lock()
        .unwrap()
        .steps
        .last_mut()
    {
        step.status = status.into();
    }
    emit(app);
}
fn action_diagnostics(
    app: &tauri::AppHandle,
    update: impl FnOnce(&mut crate::diagnostics::StepDiagnostics),
) {
    let state = app.state::<AppState>();
    let mut view = state.view.lock().unwrap();
    if let Some(diagnostic) = view
        .steps
        .last_mut()
        .and_then(|step| step.diagnostics.as_mut())
    {
        update(diagnostic);
    }
}

async fn handoff(app: &tauri::AppHandle) -> crate::diagnostics::Handoff {
    let mut report = handoff_once(app).await;
    for _ in 0..2 {
        if report.focused || report.method == "unavailable_input_desktop" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        report = handoff_once(app).await;
    }
    report
}

async fn handoff_once(app: &tauri::AppHandle) -> crate::diagnostics::Handoff {
    let (sender, receiver) = oneshot::channel();
    let handle = app.clone();
    let scheduled = app.run_on_main_thread(move || {
        if sender.is_closed() {
            return;
        }
        let report = handle.get_webview_window("main").and_then(|window| {
            let hwnd = window.hwnd().ok()?;
            let report = platform::window::handoff(hwnd.0 as usize);
            Some(report)
        });
        let _ = sender.send(report);
    });
    if scheduled.is_ok()
        && let Ok(Ok(Some(report))) = tokio::time::timeout(Duration::from_secs(2), receiver).await
    {
        return report;
    }
    crate::diagnostics::Handoff {
        method: "ui_thread_unavailable".into(),
        foreground_before: 0,
        foreground_after: 0,
        visible: false,
        minimized: false,
        focused: false,
        attachment_error: None,
        topmost_retained: false,
    }
}

async fn return_control(app: &tauri::AppHandle) {
    // Publish the terminal view before showing it. Keep the activity reservation
    // until all foreground retries finish so they cannot surface over a new run.
    emit(app);
    let report = handoff(app).await;
    {
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        state.view.lock().unwrap().record_handoff(report);
        *active = None;
    }
    emit(app);
}

async fn prepare_control_window(app: &tauri::AppHandle) -> Result<(), String> {
    let (sender, receiver) = oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if sender.is_closed() {
            return;
        }
        let result = (|| {
            let window = handle
                .get_webview_window("main")
                .ok_or("The main window is unavailable.")?;
            let hwnd = window
                .hwnd()
                .map_err(|_| "The main window is unavailable.")?;
            let _ = window.request_user_attention(None);
            platform::window::minimize_for_control(hwnd.0 as usize)
        })();
        let _ = sender.send(result);
    })
    .map_err(|_| "The main window is unavailable.")?;
    match tokio::time::timeout(Duration::from_secs(2), receiver).await {
        Ok(Ok(result)) => result,
        _ => Err("The main window could not be minimized.".into()),
    }
}

fn run_context(app: &tauri::AppHandle) -> String {
    let state = app.state::<AppState>();
    let steps = state.view.lock().unwrap().steps.clone();
    let session = state.session.lock().unwrap();
    let context = workflow::context(&session.memory, &steps);
    drop(session);
    let view = state.view.lock().unwrap();
    match view.evidence.training.as_ref() {
        Some(training) => format!("{context}\n\n{}", training.context()),
        None => context,
    }
}

struct LastInput {
    action: Action,
    before: capture::Capture,
    step_id: u32,
    completed_ms: u64,
}

async fn stable_capture(
    broker: &mut BrokerClient,
    app: &tauri::AppHandle,
    cancel: &AtomicBool,
    next_frame_id: &mut u64,
    max_edge: u32,
    last_input: Option<&LastInput>,
    recovery: bool,
) -> Result<(capture::Capture, crate::diagnostics::ObservationReport), End> {
    use crate::{
        diagnostics::{CaptureSample, ObservationReport},
        settling::{Settler, Status},
    };
    let started = platform::now();
    let transition = last_input.is_some_and(|last| last.action.expects_window_transition());
    let effect_wait = if recovery {
        5000
    } else if transition {
        8000
    } else if last_input.is_some() {
        2000
    } else {
        0
    };
    let mut report = ObservationReport {
        reason: if recovery {
            "duplicate_input_recovery"
        } else if transition {
            "window_transition"
        } else if last_input.is_some() {
            "input_effect"
        } else {
            "initial_observation"
        }
        .into(),
        completion: String::new(),
        elapsed_ms: 0,
        previous_input_step_id: last_input.map(|last| last.step_id),
        since_input_ms: None,
        samples: vec![],
    };
    let mut previous: Option<capture::Capture> = None;
    let mut settling = Settler::awaiting_effect(started, effect_wait);
    loop {
        let excluded = broker.bounds;
        let id = *next_frame_id;
        *next_frame_id += 1;
        let screen = broker
            .during(app, cancel, async move {
                tokio::task::spawn_blocking(move || capture::capture(id, max_edge, excluded))
                    .await
                    .map_err(|_| "The capture worker stopped unexpectedly.".to_owned())?
            })
            .await?;
        let changed = previous
            .as_ref()
            .is_some_and(|before| capture::changed(before, &screen));
        let effect = last_input
            .is_some_and(|last| capture::input_effect(&last.before, &screen, &last.action));
        report.samples.push(CaptureSample {
            frame_id: id,
            captured_ms: screen.frame.captured_ms,
            foreground: screen.frame.foreground,
            focused_control: screen.focused_control,
            changed,
            input_effect_observed: effect,
        });
        let status = settling.observe(platform::now(), changed, effect);
        if status != Status::Waiting {
            report.elapsed_ms = platform::now().saturating_sub(started);
            report.since_input_ms =
                last_input.map(|last| platform::now().saturating_sub(last.completed_ms));
            report.completion = match status {
                Status::Ready => "quiet",
                Status::NoEffect => "no_effect_before_deadline",
                _ => "still_changing_at_deadline",
            }
            .into();
            // A live process table may never become globally quiet. The latest
            // frame remains useful; target and focus validation still gates input.
            return Ok((screen, report));
        }
        previous = Some(screen);
        broker
            .during(app, cancel, async {
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(())
            })
            .await?;
    }
}

async fn controller(
    app: tauri::AppHandle,
    settings: Settings,
    task: String,
    cancel: Arc<AtomicBool>,
) {
    let elapsed = app.state::<AppState>().view.lock().unwrap().elapsed_ms;
    let started = Instant::now()
        .checked_sub(Duration::from_millis(elapsed))
        .unwrap_or_else(Instant::now);
    let mut next_frame_id = app
        .state::<AppState>()
        .view
        .lock()
        .unwrap()
        .evidence
        .frames
        .last()
        .map_or(1, |f| f.frame.id + 1);
    let mut terminal_excluded = [0; 4];
    let result = async {
        let mut broker = BrokerClient::spawn(false).map_err(End::Failed)?;
        match broker.signal(&app, &cancel, None).await? {
            Reply::Ready { bounds } => { broker.bounds = bounds; terminal_excluded = bounds; },
            _ => return Err(End::Failed("The input monitor did not become ready.".into())),
        }
        progress(&app, "countdown", "Starting in 2 seconds. Input interruption begins after the countdown.", started);
        match broker.signal(&app, &cancel, None).await? {
            Reply::Armed => (),
            _ => return Err(End::Failed("The input monitor could not start.".into())),
        }
        progress(&app, "running", "Taking a look at your desktop…", started);
        broker.during(&app, &cancel, prepare_control_window(&app)).await?;
        broker.during(&app, &cancel, async { tokio::time::sleep(Duration::from_millis(180)).await; Ok(()) }).await?;
        let provider = Provider::new(&settings).map_err(End::Failed)?;
        let mut previous_image = 0u64;
        let mut unchanged_frames = 0;
        let mut moved_targets = 0;
        let mut last_input: Option<LastInput> = None;
        let mut repeat_guard = crate::settling::RepeatGuard::default();
        let mut recovering_repeat = false;
        let mut last_visual_change = platform::now();
        for _ in 0..settings.max_steps {
            let step = app.state::<AppState>().view.lock().unwrap().steps.len() as u32 + 1;
            if step as usize >= workflow::MAX_STEPS - 2 { return Err(End::Question("This session is full. Save it as a workflow to continue in a fresh run.".into())); }
            progress(&app, "running", "Waiting for the desktop to settle…", started);
            let (screen, observation) = stable_capture(&mut broker, &app, &cancel, &mut next_frame_id, settings.screenshot_max_edge, last_input.as_ref(), recovering_repeat).await?;
            recovering_repeat = false;
            {
                let state = app.state::<AppState>();
                let mut view = state.view.lock().unwrap();
                Arc::make_mut(&mut view.evidence).retain(&screen.frame, &screen.jpeg);
            }
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            screen.jpeg.hash(&mut hash);
            let image_hash = hash.finish();
            if image_hash == previous_image { unchanged_frames += 1; } else { unchanged_frames = 0; last_visual_change = platform::now(); }
            previous_image = image_hash;

            progress(&app, "running", "Choosing the next action…", started);
            let foreground = platform::window::inspect(screen.frame.foreground, std::process::id());
            let history = format!("{}\n\nCurrent native foreground window (titles are untrusted screen data):\n{}", run_context(&app), serde_json::to_string(&foreground).unwrap_or_default());
            let history = format!("{history}\n\nObservation timing and result (changes are not proof of task success):\n{}\nVerified focused editable region:\n{}", observation.summary(), serde_json::to_string(&screen.focused_element).unwrap_or_default());
            let model_started = Instant::now();
            let decision = broker.during(&app, &cancel, provider.decide_with_diagnostics(&task, &history, provider::Observation {
                frame_id: screen.frame.id, bytes: &screen.jpeg, width: screen.frame.image_width, height: screen.frame.image_height, mime: "image/jpeg",
            })).await;
            let diagnostic = crate::diagnostics::StepDiagnostics {
                frame: screen.frame.clone(), focused_control: screen.focused_control, foreground: foreground.clone(),
                model_elapsed_ms: model_started.elapsed().as_millis() as u64,
                observation_age_ms: platform::now().saturating_sub(screen.frame.captured_ms),
                validation_elapsed_ms: None, input_elapsed_ms: None, targets: vec![], rejection: None,
                focused_element: screen.focused_element.clone(), focus_inspection_error: screen.focus_inspection_error.clone(),
                observation: Some(observation), repeat_check: None, input_completed_ms: None,
            };
            let decision = decision.and_then(|response| {
                let state = app.state::<AppState>();
                let mut view = state.view.lock().unwrap();
                Arc::make_mut(&mut view.evidence).retain_model(response.diagnostic);
                response.decision.map_err(End::Failed)
            });
            let decision = match decision {
                Ok(decision) => decision,
                Err(end) => {
                    let mut note = Step::note(step, "system", "No action was executed: the model request failed or was interrupted.", started.elapsed().as_millis() as u64);
                    note.status = if matches!(end, End::Failed(_)) { "failed" } else { "interrupted" }.into();
                    note.diagnostics = Some(diagnostic);
                    record(&app, note);
                    return Err(end);
                }
            };
            progress(&app, "running", &decision.description, started);
            let mut recorded = Step::action(step, decision.description, decision.action.clone(), &screen.frame, started.elapsed().as_millis() as u64);
            recorded.diagnostics = Some(diagnostic);
            record(&app, recorded);
            if let Some(last) = &last_input {
                use crate::settling::RepeatDecision;
                let effect = capture::input_effect(&last.before, &screen, &last.action);
                let repeat = repeat_guard.evaluate(&last.action, &decision.action, effect);
                if last.action.same_input(&decision.action) {
                    action_diagnostics(&app, |d| d.repeat_check = Some(crate::diagnostics::RepeatCheck {
                        previous_step_id: last.step_id, previous_frame_id: last.before.frame.id, current_frame_id: screen.frame.id,
                        input_effect_observed: effect,
                        outcome: match repeat { RepeatDecision::Allow => "allowed_after_change", RepeatDecision::ObserveAgain => "suppressed_for_fresh_observation", RepeatDecision::Pause => "paused_after_reobservation" }.into(),
                    }));
                }
                if repeat != RepeatDecision::Allow {
                    let reason = if repeat == RepeatDecision::ObserveAgain {
                        "The repeated input was not sent. Waiting for the app, then checking a fresh screenshot."
                    } else {
                        "The last input still has no visible result after waiting and checking again. Check the app, then tell me how to continue."
                    };
                    action_diagnostics(&app, |d| d.rejection = Some(crate::diagnostics::InputFailure::new("duplicate_input_without_effect", reason)));
                    action_status(&app, "skipped");
                    if repeat == RepeatDecision::Pause { return Err(End::Question(reason.into())); }
                    record(&app, Step::note(step + 1, "system", reason, started.elapsed().as_millis() as u64));
                    progress(&app, "running", reason, started);
                    recovering_repeat = true;
                    continue;
                }
            }
            if unchanged_frames >= 4 && platform::now().saturating_sub(last_visual_change) >= 20000
                && !matches!(decision.action, Action::Finish { .. } | Action::AskUser { .. }) {
                let reason = "The screen has not changed after several steps. Bring the right window into view, then tell me how to continue.";
                action_diagnostics(&app, |d| d.rejection = Some(crate::diagnostics::InputFailure::new("no_visual_progress", reason)));
                action_status(&app, "skipped");
                return Err(End::Question(reason.into()));
            }
            match &decision.action {
                Action::Finish { summary } => {
                    action_status(&app, "completed");
                    return Err(End::Done(summary.clone()));
                }
                Action::AskUser { question } => {
                    action_status(&app, "recorded");
                    if foreground.input_block.as_deref() == Some("higher_integrity") {
                        let failure = crate::diagnostics::InputFailure::target(foreground);
                        action_diagnostics(&app, |d| d.rejection = Some(failure.clone()));
                        return Err(End::Blocked(failure, "model_handoff"));
                    }
                    return Err(End::Question(question.clone()));
                }
                Action::Wait { duration_ms } => {
                    let duration = *duration_ms;
                    broker.during(&app, &cancel, async move { tokio::time::sleep(Duration::from_millis(duration as u64)).await; Ok(()) }).await?;
                    action_status(&app, "completed");
                }
                Action::Observe => action_status(&app, "completed"),
                action => {
                    let action = action.clone();
                    let check_action = action.clone();
                    let mut frame = screen.frame.clone();
                    let validation_started = Instant::now();
                    match platform::input::inspect_action(&frame, &action, std::process::id()) {
                        Ok(targets) => action_diagnostics(&app, |d| d.targets = targets),
                        Err(failure) => {
                            action_diagnostics(&app, |d| { d.validation_elapsed_ms = Some(validation_started.elapsed().as_millis() as u64); d.rejection = Some(failure.clone()); });
                            action_status(&app, "skipped");
                            moved_targets += 1;
                            if matches!(failure.code.as_str(), "own_window" | "foreground_changed") && moved_targets < 3 { continue; }
                            if failure.code == "higher_integrity" { return Err(End::Blocked(failure, "input_validation")); }
                            return Err(End::Stopped(failure.message));
                        }
                    }
                    // Revalidation failure is recoverable: observe again without clicking a moved target.
                    let (screen, checked) = broker.during(&app, &cancel, async move {
                        tokio::task::spawn_blocking(move || { let checked = capture::unchanged(&screen, &check_action); Ok((screen, checked)) }).await
                            .map_err(|_| "The target check failed.".to_owned())?
                    }).await?;
                    action_diagnostics(&app, |d| d.validation_elapsed_ms = Some(validation_started.elapsed().as_millis() as u64));
                    if let Err(reason) = checked {
                        action_diagnostics(&app, |d| d.rejection = Some(crate::diagnostics::InputFailure::new("observation_changed", &reason)));
                        action_status(&app, "skipped");
                        record(&app, Step::note(step + 1, "system", reason, started.elapsed().as_millis() as u64));
                        moved_targets += 1;
                        if moved_targets >= 3 { return Err(End::Question("The target keeps changing. Bring it into view and tell me when to continue.".into())); }
                        continue;
                    }
                    moved_targets = 0;
                    // The target and layout were just checked against the live physical desktop.
                    frame.captured_ms = platform::now();
                    let input_started = Instant::now();
                    broker.send(Command::Execute { sequence: step as u64, sent_ms: platform::now(), frame, action: action.clone() })?;
                    match broker.signal(&app, &cancel, Some(step as u64)).await? {
                        Reply::Completed { sequence } if sequence == step as u64 => (),
                        Reply::Rejected { sequence, diagnostic } if sequence == step as u64 => {
                            action_diagnostics(&app, |d| { d.input_elapsed_ms = Some(input_started.elapsed().as_millis() as u64); d.rejection = Some(diagnostic.clone()); });
                            action_status(&app, "interrupted");
                            if diagnostic.code == "higher_integrity" { return Err(End::Blocked(diagnostic, "input_broker")); }
                            return Err(End::Stopped(diagnostic.message));
                        }
                        _ => return Err(End::Failed("The input acknowledgement did not match this step.".into())),
                    }
                    let completed_ms = platform::now();
                    action_diagnostics(&app, |d| { d.input_elapsed_ms = Some(input_started.elapsed().as_millis() as u64); d.input_completed_ms = Some(completed_ms); });
                    action_status(&app, "completed");
                    last_input = Some(LastInput { action, before: screen, step_id: step, completed_ms });
                    repeat_guard = crate::settling::RepeatGuard::default();
                }
            }
        }
        Err::<(), End>(End::Stopped("The task reached its step limit. Refine the instructions to continue.".into()))
    }.await;
    // The broker has released input. Capture before restoring klickwerk so the
    // export can reveal a window that appeared during the final model request.
    let max_edge = settings.screenshot_max_edge;
    let terminal_capture = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::task::spawn_blocking(move || {
            capture::capture(next_frame_id, max_edge, terminal_excluded)
        }),
    )
    .await;
    let mut terminal_desktop = crate::diagnostics::TerminalDesktop {
        frame_id: None,
        capture_error: None,
        captured_ms: platform::now(),
        foreground: platform::window::inspect(
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } as usize,
            std::process::id(),
        ),
        focused_control: capture::focused_control(),
    };
    match terminal_capture {
        Ok(Ok(Ok(screen))) => {
            terminal_desktop.frame_id = Some(screen.frame.id);
            terminal_desktop.captured_ms = screen.frame.captured_ms;
            terminal_desktop.foreground =
                platform::window::inspect(screen.frame.foreground, std::process::id());
            terminal_desktop.focused_control = screen.focused_control;
            let state = app.state::<AppState>();
            let mut view = state.view.lock().unwrap();
            Arc::make_mut(&mut view.evidence).retain_for(
                &screen.frame,
                &screen.jpeg,
                "before_handoff",
            );
        }
        Ok(Ok(Err(error))) => terminal_desktop.capture_error = Some(error),
        _ => {
            terminal_desktop.capture_error =
                Some("Terminal capture timed out or its worker stopped.".into())
        }
    }
    app.state::<AppState>()
        .view
        .lock()
        .unwrap()
        .record_terminal_desktop(terminal_desktop);
    let end = result
        .err()
        .unwrap_or_else(|| End::Done("Finished.".into()));
    let mut automatic = None;
    {
        let state = app.state::<AppState>();
        let _active = state.active.lock().unwrap();
        let mut view = state.view.lock().unwrap();
        view.elapsed_ms = started.elapsed().as_millis() as u64;
        if let Some(step) = view
            .steps
            .last_mut()
            .filter(|step| step.status == "pending")
        {
            step.status = if matches!(end, End::Failed(_)) {
                "failed"
            } else {
                "interrupted"
            }
            .into();
        }
        match end {
            End::Stopped(message) => {
                view.interrupted = message == "Stopped because you used the mouse or keyboard.";
                view.phase = "stopped".into();
                view.message = if view.interrupted {
                    "You took over. The agent is paused. Continue when ready, or describe what to change.".into()
                } else {
                    message
                };
                let note = if view.interrupted {
                    "User took over with mouse or keyboard input. Treat the latest attempted action as a possible mistake. Wait for explicit permission to continue or a correction; do not assume why it was wrong."
                } else {
                    "The run stopped. Check the last action and the current desktop before continuing."
                };
                let step = Step::note(view.steps.len() as u32 + 1, "system", note, view.elapsed_ms);
                view.steps.push(step);
            }
            End::Failed(message) => {
                view.phase = "error".into();
                let note = Step::note(
                    view.steps.len() as u32 + 1,
                    "system",
                    format!("Run failed: {message}"),
                    view.elapsed_ms,
                );
                view.steps.push(note);
                view.message = message;
            }
            End::Question(question) => {
                view.phase = "waiting".into();
                view.message = "Control is paused while you reply.".into();
                view.question = question;
            }
            End::Blocked(failure, source) => view.block_for_privileges(failure, source),
            End::Done(summary) => {
                view.phase = "done".into();
                view.message = "Your task is complete.".into();
                view.result = summary;
            }
        }
        view.finish_attempt();
        if view.recovery.is_some() {
            match crate::recovery::automatic_session(
                &mut view,
                state.session.lock().unwrap().memory.clone(),
                cancel.load(Ordering::SeqCst),
            ) {
                Ok(session) => automatic = Some(session),
                Err(reason) => {
                    view.recovery_event("automatic_elevation_skipped", &reason, None, None)
                }
            }
        }
    }
    emit(&app);
    if let Some(session) = automatic {
        let owner = app
            .get_webview_window("main")
            .and_then(|window| window.hwnd().ok())
            .map_or(0, |handle| handle.0 as usize);
        let _ = perform_restart(app, owner, session, cancel, crate::recovery::AUTO_SOURCE).await;
    } else {
        return_control(&app).await;
    }
}

fn app_context(background_resume: bool) -> tauri::Context<tauri::Wry> {
    let mut context = tauri::generate_context!();
    if background_resume {
        for window in &mut context.config_mut().app.windows {
            if window.label == "main" {
                window.visible = false;
                window.focus = false;
            }
        }
    }
    if !tauri::is_dev() {
        context.config_mut().build.dev_url = None;
    }
    context
}

pub fn run() {
    let (updated_token, restored) = match platform::update::receive_if_requested()
        .and_then(|token| platform::restart::receive_if_requested().map(|session| (token, session)))
    {
        Ok(value) => value,
        Err(error) => {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    platform::wide(&error).as_ptr(),
                    platform::wide("klickwerk").as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                );
            }
            return;
        }
    };
    // Single-instance ownership protects both the portable config and the desktop controller.
    let name = platform::wide("Local\\KlickwerkDesktopV2");
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                platform::wide(
                    "klickwerk is already running. Open its existing window from the taskbar.",
                )
                .as_ptr(),
                platform::wide("klickwerk").as_ptr(),
                0,
            );
        }
        return;
    }
    let _mutex = platform::Handle(mutex);
    let config = ConfigStore::beside_executable();
    let workflows = WorkflowStore::load(config.path.with_file_name("workflows.json"));
    // Recovery preserves a live session. Update handoffs check again without a restart loop.
    let install_update = restored.is_none() && updated_token.is_none();
    let (mut view, memory, refinement, pending_resume) = restored.map_or_else(
        || (RunView::default(), String::new(), String::new(), None),
        |session| {
            let pending = session.resume_after_approval.then_some(session.run.id);
            (session.run, session.memory, session.refinement, pending)
        },
    );
    if pending_resume.is_some() {
        view.recovery_event("background_resume_startup", "window_lifecycle", None, None);
    }
    let next_id = view.id;
    let state = AppState {
        config: Mutex::new(config),
        view: Mutex::new(view),
        active: Mutex::new(None),
        ui_at: AtomicU64::new(platform::now()),
        next_id: AtomicU64::new(next_id),
        requests: Mutex::new(HashMap::new()),
        speech: Mutex::new(None),
        training_paused: Arc::new(AtomicBool::new(false)),
        session: Mutex::new(Session { memory }),
        restored_refinement: Mutex::new(refinement),
        pending_resume: Mutex::new(pending_resume),
        workflows: Mutex::new(workflows),
        drafts: Mutex::new(HashMap::new()),
        update: Mutex::new(crate::update::Status::new(install_update)),
        update_cancel: Mutex::new(None),
    };
    let result = tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            if let (Some(token), Ok(executable)) = (&updated_token, std::env::current_exe()) {
                crate::update::cleanup(&executable, token);
            }
            let _ = begin_update(app.handle(), install_update, updated_token.is_some());
            if let Some(run_id) = pending_resume {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    if handle.state::<AppState>().fail_resume_setup(
                        run_id,
                        "startup_timeout",
                        crate::recovery::RESUME_UI_TIMEOUT,
                    ) {
                        return_control(&handle).await;
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            skip_update,
            check_updates,
            ui_heartbeat,
            save_settings,
            list_models,
            test_connection,
            cancel_request,
            start_task,
            restart_as_administrator,
            resume_after_restart,
            get_workflow,
            import_workflow,
            export_workflow,
            start_training,
            pause_training,
            export_session,
            prepare_workflow,
            save_workflow,
            delete_workflow,
            reset_session,
            stop_task,
            start_dictation,
            stop_dictation
        ])
        .on_window_event(|window, event| {
            if let Ok(hwnd) = window.hwnd() {
                match event {
                    tauri::WindowEvent::Focused(focused) => {
                        platform::window::handoff_focus_changed(hwnd.0 as usize, *focused)
                    }
                    tauri::WindowEvent::Destroyed => {
                        platform::window::release_handoff_topmost(hwnd.0 as usize)
                    }
                    _ => (),
                }
            }
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                let state = window.state::<AppState>();
                if let Some(cancel) = state.active.lock().unwrap().as_ref() {
                    cancel.store(true, Ordering::SeqCst);
                }
                if let Some(cancel) = state.speech.lock().unwrap().as_ref() {
                    cancel.store(true, Ordering::SeqCst);
                }
                if let Some(cancel) = state.update_cancel.lock().unwrap().as_ref() {
                    cancel.store(true, Ordering::SeqCst);
                }
            }
        })
        .run(app_context(pending_resume.is_some()));
    if let Err(error) = result {
        let message = platform::wide(&format!(
            "klickwerk could not open.\n\n{error}\n\nIf WebView2 is missing, install the Microsoft Edge WebView2 Runtime and try again."
        ));
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                platform::wide("klickwerk").as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}

#[cfg(all(test, not(dev)))]
mod release_tests {
    use super::app_context;

    #[test]
    fn production_frontend_is_embedded_and_has_no_dev_server() {
        let context = app_context(false);
        assert!(!tauri::is_dev(), "The release must use the asset protocol.");
        assert!(context.config().build.dev_url.is_none());
        let assets = context.assets();
        let index = assets
            .get(&"index.html".into())
            .expect("The release must contain its HTML entry point.");
        let html = std::str::from_utf8(&index).unwrap();
        assert!(html.contains("<title>klickwerk</title>"));
        assert!(html.contains("id=\"root\""));
        assert!(!html.contains("/@vite/client"));
        assert!(!html.contains("localhost:1420"));

        let references: Vec<_> = html
            .split('"')
            .filter(|value| value.starts_with("/assets/"))
            .collect();
        assert!(references.iter().any(|path| path.ends_with(".js")));
        assert!(references.iter().any(|path| path.ends_with(".css")));
        for path in references {
            let content = assets
                .get(&path.into())
                .unwrap_or_else(|| panic!("The release is missing {path}."));
            assert!(!content.is_empty(), "The embedded asset {path} is empty.");
        }
    }
}
