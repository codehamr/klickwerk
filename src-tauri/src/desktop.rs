use crate::{
    action::Action,
    config::{ConfigStore, Settings, same_origin},
    guard::{Command, Reply},
    platform::{self, capture, input::HeldInput},
    provider::{self, ModelList, Provider},
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

#[derive(Clone, Serialize, Default)]
pub struct Step {
    id: u32,
    description: String,
}
#[derive(Clone, Serialize)]
pub struct RunView {
    id: u64,
    phase: String,
    task: String,
    message: String,
    steps: Vec<Step>,
    result: String,
    question: String,
    elapsed_ms: u64,
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
        }
    }
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    settings: Settings,
    has_api_key: bool,
    config_path: String,
    config_error: Option<String>,
    run: RunView,
    platform: &'static str,
}

pub struct AppState {
    config: Mutex<ConfigStore>,
    view: Mutex<RunView>,
    active: Mutex<Option<Arc<AtomicBool>>>,
    ui_at: AtomicU64,
    next_id: AtomicU64,
    requests: Mutex<HashMap<String, oneshot::Sender<()>>>,
    speech: Mutex<Option<Arc<AtomicBool>>>,
    continuation: Mutex<String>,
}
impl AppState {
    fn snapshot(&self) -> Snapshot {
        let config = self.config.lock().unwrap();
        Snapshot {
            settings: config.settings.public(),
            has_api_key: !config.settings.api_key.is_empty(),
            config_path: config.path.to_string_lossy().into(),
            config_error: config.error.clone(),
            run: self.view.lock().unwrap().clone(),
            platform: "windows",
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
async fn discover_servers() -> Vec<provider::Discovery> {
    provider::discover().await
}
#[tauri::command]
fn stop_task(state: tauri::State<AppState>) {
    if let Some(cancel) = state.active.lock().unwrap().as_ref() {
        cancel.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
fn start_task(app: tauri::AppHandle, task: String, reply: Option<String>) -> Result<(), String> {
    start(app, task, reply, false)
}
#[tauri::command]
fn start_stop_test(app: tauri::AppHandle) -> Result<(), String> {
    start(app, "Try the emergency stop".into(), None, true)
}

fn start(
    app: tauri::AppHandle,
    task: String,
    reply: Option<String>,
    simulate: bool,
) -> Result<(), String> {
    if task.trim().is_empty() || task.len() > 32768 {
        return Err("Enter a task of at most 8,192 characters.".into());
    }
    let state = app.state::<AppState>();
    if state.speech.lock().unwrap().is_some() {
        return Err("Finish dictation before starting the task.".into());
    }
    let mut active = state.active.lock().unwrap();
    if active.is_some() {
        return Err("A task is already running.".into());
    }
    let settings = {
        let store = state.config.lock().unwrap();
        if !simulate {
            if let Some(error) = &store.error {
                return Err(error.clone());
            }
            store.settings.ready()?;
        }
        store.settings.clone()
    };
    let mut history = String::new();
    if let Some(answer) = reply {
        let previous = state.view.lock().unwrap();
        if previous.phase != "waiting"
            || previous.task != task
            || answer.trim().is_empty()
            || answer.len() > 16384
        {
            return Err(
                "This answer no longer belongs to the current task. Start a new task.".into(),
            );
        }
        history = format!(
            "{}\nAssistant question: {}\nUser answer: {}",
            state.continuation.lock().unwrap().as_str(),
            previous.question,
            answer
        );
    }
    let id = state.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(cancel.clone());
    state.ui_at.store(platform::now(), Ordering::SeqCst);
    *state.view.lock().unwrap() = RunView {
        id,
        phase: "checking".into(),
        task: task.clone(),
        message: "Preparing your emergency stop…".into(),
        ..RunView::default()
    };
    drop(active);
    emit(&app);
    tauri::async_runtime::spawn(async move {
        controller(app, settings, task, history, cancel, simulate).await;
    });
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
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| "The independent emergency stop process could not start.")?;
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
    fn send(&self, command: Command) -> Result<(), End> {
        self.sender
            .try_send(command)
            .map_err(|_| End::Stopped("The emergency stop connection is unavailable.".into()))
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
                    else if cancel.load(Ordering::SeqCst) || waiting_since.elapsed()>Duration::from_secs(5) {return Err(End::Stopped("The emergency stop startup was cancelled or timed out.".into()));}
                },
                reply=self.receiver.recv()=>return match reply{
                    Some(Reply::Stopped{reason})=>Err(End::Stopped(reason)),Some(Reply::Error{message})=>Err(End::Failed(message)),Some(value)=>{if matches!(value,Reply::Ready{..}){self.ready=true;}Ok(value)},None=>Err(End::Stopped("The emergency stop process exited. Input has been released.".into()))
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
                reply=self.receiver.recv()=>return Err(match reply{Some(Reply::Stopped{reason})=>End::Stopped(reason),Some(Reply::Error{message})=>End::Failed(message),_=>End::Stopped("The emergency stop connection ended.".into())})
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
    Done(String),
}

async fn controller(
    app: tauri::AppHandle,
    settings: Settings,
    task: String,
    mut history: String,
    cancel: Arc<AtomicBool>,
    simulate: bool,
) {
    let started = Instant::now();
    let result=async{
        let mut broker=BrokerClient::spawn(simulate).map_err(End::Failed)?;
        match broker.signal(&app,&cancel,None).await?{Reply::Ready{bounds}=>broker.bounds=bounds,_=>return Err(End::Failed("The stop process did not become ready.".into()))}
        progress(&app,"countdown","Starting in 3 seconds. Let go of your mouse and keyboard.",started);
        if let Some(window)=app.get_webview_window("main"){window.minimize().map_err(|_|End::Failed("The main window could not be minimized safely.".into()))?;}
        match broker.signal(&app,&cancel,None).await?{Reply::Armed=>(),_=>return Err(End::Failed("The stop process could not arm.".into()))}
        if simulate{
            progress(&app,"running","Stop test is running. Press Ctrl + Alt + F8, click STOP, or use your mouse or keyboard.",started);
            broker.during(&app,&cancel,async{tokio::time::sleep(Duration::from_secs(30)).await;Ok(())}).await?;
            return Err(End::Done("The safe stop test finished. No desktop input was sent.".into()));
        }
        let provider=Provider::new(&settings).map_err(End::Failed)?;
        let mut proposed=false;let mut previous_image=0u64;let mut unchanged_frames=0;
        for step in 1..=settings.max_steps{
            if started.elapsed()>Duration::from_secs(600){return Err(End::Stopped("The ten-minute task limit was reached.".into()));}
            progress(&app,"running",if proposed{"Checking the result…"}else{"Taking a look at your desktop…"},started);
            let excluded=broker.bounds;let max_edge=settings.screenshot_max_edge;
            let screen=broker.during(&app,&cancel,async move{tokio::task::spawn_blocking(move||capture::capture(step as u64,max_edge,excluded)).await.map_err(|_|"The capture worker stopped unexpectedly.".to_owned())?}).await?;
            use std::hash::{Hash,Hasher};let mut hash=std::collections::hash_map::DefaultHasher::new();screen.jpeg.hash(&mut hash);let image_hash=hash.finish();
            if image_hash==previous_image{unchanged_frames+=1;}else{unchanged_frames=0;}previous_image=image_hash;
            if unchanged_frames>=4{return Err(End::Question("The screen has not changed after several steps. Could you bring the right window into view, then tell me how to continue?".into()));}
            progress(&app,"running",if proposed{"Checking whether your task is complete…"}else{"Thinking about the next step…"},started);
            let decision=broker.during(&app,&cancel,provider.decide(&task,&history,provider::Observation{frame_id:screen.frame.id,bytes:&screen.jpeg,width:screen.frame.image_width,height:screen.frame.image_height,mime:"image/jpeg"})).await?;
            progress(&app,"running",&decision.description,started);
            match &decision.action{
                Action::Finish{summary} if proposed=>return Err(End::Done(summary.clone())),
                Action::Finish{summary}=>{history.push_str(&format!("\nCompletion proposed: {summary}. Check this against the original task in the next fresh screenshot. Finish only if visibly complete."));proposed=true;},
                Action::AskUser{question}=>return Err(End::Question(question.clone())),
                Action::Wait{duration_ms}=>{proposed=false;let duration=*duration_ms;broker.during(&app,&cancel,async move{tokio::time::sleep(Duration::from_millis(duration as u64)).await;Ok(())}).await?;},
                Action::Observe=>{proposed=false;},
                action=>{
                    proposed=false;let action=action.clone();let check_action=action.clone();
                    let mut frame=screen.frame.clone();
                    broker.during(&app,&cancel,async move{tokio::task::spawn_blocking(move||capture::unchanged(&screen,&check_action)).await.map_err(|_|"The target check failed.".to_owned())?}).await?;
                    // The model coordinates were just revalidated against the live desktop.
                    frame.captured_ms=platform::now();
                    broker.send(Command::Execute{sequence:step as u64,sent_ms:platform::now(),frame,action})?;
                    match broker.signal(&app,&cancel,Some(step as u64)).await?{Reply::Completed{sequence}if sequence==step as u64=>(),_=>return Err(End::Failed("The input acknowledgement did not match this step.".into()))}
                    broker.during(&app,&cancel,async{tokio::time::sleep(Duration::from_millis(350)).await;Ok(())}).await?;
                }
            }
            {app.state::<AppState>().view.lock().unwrap().steps.push(Step{id:step,description:decision.description.clone()});}emit(&app);
            history.push_str(&format!("\nStep {step}: {}",decision.description));
            if history.len()>12000 {let mut boundary=history.len()-10000;while !history.is_char_boundary(boundary){boundary+=1;}history=history[boundary..].to_owned();}
        }
        Err::<(),End>(End::Stopped("The task reached its step limit. Review the progress before starting again.".into()))
    }.await;
    let end = result
        .err()
        .unwrap_or_else(|| End::Done("Finished.".into()));
    {
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        let mut view = state.view.lock().unwrap();
        view.elapsed_ms = started.elapsed().as_millis() as u64;
        match end {
            End::Stopped(message) => {
                view.phase = "stopped".into();
                view.message = message;
            }
            End::Failed(message) => {
                view.phase = "error".into();
                view.message = message;
            }
            End::Question(question) => {
                view.phase = "waiting".into();
                view.message = "Control is paused while you reply.".into();
                view.question = question;
                *state.continuation.lock().unwrap() = history;
            }
            End::Done(summary) => {
                view.phase = "done".into();
                view.message = "Your task is complete.".into();
                view.result = summary;
            }
        }
        *active = None;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
    }
    emit(&app);
}

fn app_context() -> tauri::Context<tauri::Wry> {
    let mut context = tauri::generate_context!();
    if !tauri::is_dev() {
        context.config_mut().build.dev_url = None;
    }
    context
}

pub fn run() {
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
    let state = AppState {
        config: Mutex::new(ConfigStore::beside_executable()),
        view: Mutex::new(RunView::default()),
        active: Mutex::new(None),
        ui_at: AtomicU64::new(platform::now()),
        next_id: AtomicU64::new(0),
        requests: Mutex::new(HashMap::new()),
        speech: Mutex::new(None),
        continuation: Mutex::new(String::new()),
    };
    let result = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            ui_heartbeat,
            save_settings,
            list_models,
            test_connection,
            cancel_request,
            discover_servers,
            start_task,
            start_stop_test,
            stop_task,
            start_dictation,
            stop_dictation
        ])
        .on_window_event(|window, event| {
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
            }
        })
        .run(app_context());
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
        let context = app_context();
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
