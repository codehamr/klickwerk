//! Isolated native tests. External input is injected only into disposable fixture windows.
use super::*;
use crate::{action::Action, guard::Frame};
use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleW,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

async fn next(
    broker: &mut BrokerClient,
    heartbeat: bool,
    timeout_ms: u64,
) -> Result<Reply, String> {
    let deadline = tokio::time::sleep(Duration::from_millis(timeout_ms));
    tokio::pin!(deadline);
    let mut interval = tokio::time::interval(Duration::from_millis(80));
    loop {
        tokio::select! {
            _=&mut deadline=>return Err("Timed out waiting for the native broker.".into()),
            _=interval.tick()=>{if heartbeat && broker.ready {broker.sender.try_send(Command::Heartbeat{sent_ms:platform::now(),lease_sequence:None}).map_err(|_|"The broker command pipe closed.")?;}},
            reply=broker.receiver.recv()=>{
                let reply=reply.ok_or("The native broker exited without a reply.")?;
                if let Reply::Ready{bounds}=&reply{broker.ready=true;broker.bounds = *bounds;}
                return Ok(reply);
            }
        }
    }
}
async fn ready(broker: &mut BrokerClient) -> Result<(), String> {
    match next(broker, true, 10000).await? {
        Reply::Ready { .. } => Ok(()),
        other => Err(format!("Expected Ready, received {other:?}")),
    }
}
async fn armed(broker: &mut BrokerClient) -> Result<(), String> {
    ready(broker).await?;
    match next(broker, true, 8000).await? {
        Reply::Armed => Ok(()),
        other => Err(format!("Expected Armed, received {other:?}")),
    }
}
fn check(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        println!("PASS {message}");
        Ok(())
    } else {
        Err(message.into())
    }
}
fn monitor() -> HWND {
    unsafe {
        FindWindowW(
            platform::wide("KlickwerkInputMonitor").as_ptr(),
            std::ptr::null(),
        )
    }
}
fn fixture_key(tag: usize) {
    let events: Vec<INPUT> = [false, true]
        .into_iter()
        .map(|up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_SHIFT,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: tag,
                },
            },
        })
        .collect();
    unsafe {
        assert_eq!(
            SendInput(
                events.len() as u32,
                events.as_ptr(),
                size_of::<INPUT>() as i32
            ),
            events.len() as u32
        );
    }
}
fn fixture_hardware_key() -> Result<(), String> {
    // SendInput cannot produce hardware flags. The test-only message invokes the
    // production keyboard handler on its owning thread without synthesizing a key.
    let window = monitor();
    let mut result = 0;
    if window.is_null()
        || unsafe {
            SendMessageTimeoutW(
                window,
                platform::broker::FIXTURE_KEYBOARD_EVENT,
                0,
                0,
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                500,
                &mut result,
            )
        } == 0
        || result != 1
    {
        return Err("The hardware keyboard fixture could not reach the input monitor.".into());
    }
    Ok(())
}
fn fixture_heartbeat(broker: &BrokerClient) -> Result<(), String> {
    // Synchronous screenshot comparisons also need liveness between actions.
    broker
        .sender
        .try_send(Command::Heartbeat {
            sent_ms: platform::now(),
            lease_sequence: None,
        })
        .map_err(|_| "The fixture heartbeat failed.".into())
}
fn fixture_mouse(tag: usize) {
    // Relative movement avoids Wine's cached GetCursorPos in this fixture thread.
    // Alternate direction so repeated fixture events cannot run into a screen edge.
    static LEFT: AtomicBool = AtomicBool::new(false);
    let dx = if LEFT.fetch_xor(true, Ordering::SeqCst) {
        120
    } else {
        -120
    };
    fixture_mouse_motion(dx, 0, MOUSEEVENTF_MOVE, tag);
}

fn fixture_mouse_to(position: (i32, i32), tag: usize) {
    let (left, top, width, height) = capture::layout();
    let frame = Frame {
        id: 1,
        captured_ms: 0,
        left,
        top,
        width,
        height,
        image_width: width,
        image_height: height,
        foreground: 0,
    };
    let (dx, dy) = frame
        .absolute(position.0 - left, position.1 - top)
        .expect("Fixture pointer must be on screen");
    fixture_mouse_motion(
        dx,
        dy,
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        tag,
    );
}

fn fixture_mouse_motion(dx: i32, dy: i32, flags: u32, tag: usize) {
    let event = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: tag,
            },
        },
    };
    unsafe {
        assert_eq!(SendInput(1, &event, size_of::<INPUT>() as i32), 1);
    }
}

fn fixture_mouse_button_or_wheel(wheel: bool) {
    let flags = if wheel {
        vec![MOUSEEVENTF_WHEEL]
    } else {
        vec![MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP]
    };
    let events: Vec<INPUT> = flags
        .into_iter()
        .map(|dw_flags| INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dwFlags: dw_flags,
                    mouseData: if wheel { 120 } else { 0 },
                    ..unsafe { std::mem::zeroed() }
                },
            },
        })
        .collect();
    unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            size_of::<INPUT>() as i32,
        );
    }
}
async fn suite() -> Result<(), String> {
    {
        let mut broker = BrokerClient::spawn(true)?;
        armed(&mut broker).await?;
        let window = monitor();
        check(
            !window.is_null() && unsafe { IsWindowVisible(window) } == 0,
            "input monitoring runs without a visible stop bar",
        )?;
        check(
            broker.bounds == [0; 4],
            "no screenshot region is hidden by a stop bar",
        )?;
        broker
            .sender
            .try_send(Command::Stop)
            .map_err(|_| "Could not close the fixture.")?;
        check(
            matches!(next(&mut broker, false, 1500).await?, Reply::Stopped { .. }),
            "internal cancellation stops the broker",
        )?;
    }
    {
        let mut broker = BrokerClient::spawn(true)?;
        armed(&mut broker).await?;
        check(
            matches!(next(&mut broker, false, 2000).await?, Reply::Stopped { reason, .. } if reason.contains("heartbeat")),
            "heartbeat loss stops an armed broker",
        )?;
    }
    {
        let mut broker = BrokerClient::spawn(true)?;
        ready(&mut broker).await?;
        fixture_mouse(0);
        fixture_hardware_key()?;
        check(
            matches!(next(&mut broker, true, 4000).await?, Reply::Armed),
            "mouse and keyboard input during countdown do not cancel startup",
        )?;
        fixture_hardware_key()?;
        check(
            matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { reason, .. } if reason.contains("mouse or keyboard")),
            "keyboard takeover starts after the countdown",
        )?;
    }
    {
        let mut broker = BrokerClient::spawn(true)?;
        ready(&mut broker).await?;
        fixture_mouse(0);
        broker
            .sender
            .try_send(Command::Stop)
            .map_err(|_| "Cannot cancel startup.")?;
        check(
            matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { .. }),
            "explicit Stop still cancels the countdown",
        )?;
    }
    {
        let mut broker = BrokerClient::spawn(true)?;
        ready(&mut broker).await?;
        let now = platform::now();
        broker
            .sender
            .try_send(Command::Execute {
                sequence: 1,
                sent_ms: now,
                frame: Frame {
                    id: 1,
                    captured_ms: now,
                    left: 0,
                    top: 0,
                    width: 100,
                    height: 100,
                    image_width: 100,
                    image_height: 100,
                    foreground: 0,
                },
                action: Action::Move { x: 1, y: 1 },
            })
            .map_err(|_| "Cannot send fixture input.")?;
        check(
            matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { .. }),
            "input before arming is refused",
        )?;
    }
    {
        let mut broker = BrokerClient::spawn(true)?;
        armed(&mut broker).await?;
        let (unused, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        broker.sender = unused;
        check(
            matches!(next(&mut broker, false, 1500).await?, Reply::Stopped { reason, .. } if reason.contains("connection")),
            "controller pipe loss stops input",
        )?;
    }
    unsafe {
        let window = CreateWindowExW(
            0,
            platform::wide("STATIC").as_ptr(),
            platform::wide("Disposable takeover fixture").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            150,
            420,
            220,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
        if window.is_null() {
            return Err("The isolated focus window could not be created.".into());
        }
        SetForegroundWindow(window);
        let result = async {
            for mouse in [false, true] {
                let mut broker = BrokerClient::spawn(true)?;
                armed(&mut broker).await?;
                check(GetForegroundWindow() == window, "the input monitor leaves the target app focused")?;
                if mouse { fixture_mouse(platform::input::INPUT_TAG); } else {
                    fixture_key(0);
                    check(next(&mut broker, true, 150).await.is_err(), "software keyboard input is tolerated without recent agent input")?;
                    fixture_key(platform::input::INPUT_TAG);
                    fixture_key(0);
                }
                check(next(&mut broker, true, 150).await.is_err(),
                    if mouse { "tagged agent input does not interrupt itself" } else { "untagged software keyboard events after agent input do not claim user takeover" })?;
                let started = Instant::now();
                if mouse { fixture_mouse(0); } else {
                    fixture_key(platform::input::INPUT_TAG);
                    fixture_hardware_key()?;
                }
                check(matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { reason, interruption: Some(event) }
                    if reason.contains("mouse or keyboard") && (mouse || (event.event == WM_KEYDOWN && event.flags == 0 && event.since_agent_input_ms.is_some()))),
                    if mouse { "external mouse movement stops the broker" } else { "hardware keyboard flags stop immediately after agent input and record the trigger" })?;
                println!("Takeover fixture latency: {} ms", started.elapsed().as_millis());
            }
            {
                let mut broker = BrokerClient::spawn(true)?;
                armed(&mut broker).await?;
                fixture_mouse_to((250, 250), platform::input::INPUT_TAG);
                for x in [251, 300, 350] {
                    fixture_mouse_to((x, 250), 0);
                    check(next(&mut broker, true, 100).await.is_err(), "small mouse movement stays within the 100 pixel tolerance")?;
                }
                fixture_mouse_to((351, 250), 0);
                check(matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { interruption: Some(event), .. }
                    if event.event == WM_MOUSEMOVE && event.anchor == Some([250, 250]) && event.position == Some([351, 250]) && event.since_agent_input_ms.is_some()),
                    "slow mouse movement beyond 100 pixels interrupts and records its trigger")?;
            }
            for wheel in [false, true] {
                let mut broker = BrokerClient::spawn(true)?;
                armed(&mut broker).await?;
                fixture_mouse_to((250, 250), platform::input::INPUT_TAG);
                fixture_mouse_button_or_wheel(wheel);
                check(matches!(next(&mut broker, true, 1500).await?, Reply::Stopped { interruption: Some(event), .. }
                    if event.event == if wheel { WM_MOUSEWHEEL } else { WM_LBUTTONDOWN }),
                    "clicks and scrolling interrupt without a movement threshold")?;
            }
            Ok::<(), String>(())
        }.await;
        DestroyWindow(window);
        result?;
    }
    println!("Native input monitoring checks passed on the isolated desktop.");
    Ok(())
}

fn quiet_resume_checks() -> Result<(), String> {
    for background in [false, true] {
        let context = app_context(background);
        let window = context
            .config()
            .app
            .windows
            .iter()
            .find(|window| window.label == "main")
            .unwrap();
        check(
            window.visible == !background && window.focus == !background,
            if background {
                "automatic restart creates its window hidden and unfocused"
            } else {
                "ordinary startup and manual recovery keep the main window visible"
            },
        )?;
    }
    let state = AppState {
        config: Mutex::new(ConfigStore {
            path: Default::default(),
            settings: Settings::default(),
            error: None,
        }),
        view: Mutex::new(RunView {
            id: 73,
            phase: "recovering".into(),
            task: "Continue the original task".into(),
            ..RunView::default()
        }),
        active: Mutex::new(None),
        ui_at: AtomicU64::new(0),
        next_id: AtomicU64::new(73),
        requests: Mutex::new(HashMap::new()),
        speech: Mutex::new(None),
        training_paused: Arc::new(AtomicBool::new(false)),
        session: Mutex::new(Session::default()),
        restored_refinement: Mutex::new(String::new()),
        pending_resume: Mutex::new(Some(73)),
        workflows: Mutex::new(WorkflowStore {
            path: Default::default(),
            workflows: vec![],
            error: None,
        }),
        drafts: Mutex::new(HashMap::new()),
        update: Mutex::new(crate::update::Status::new(false)),
        update_cancel: Mutex::new(None),
    };
    check(
        !state.fail_resume_setup(72, "startup_timeout", "Fixture startup timeout")
            && *state.pending_resume.lock().unwrap() == Some(73),
        "an old startup timeout cannot affect another run",
    )?;
    check(
        state.fail_resume_setup(73, "startup_timeout", crate::recovery::RESUME_UI_TIMEOUT)
            && state.pending_resume.lock().unwrap().is_none()
            && state.active.lock().unwrap().is_some()
            && state.view.lock().unwrap().phase == "error",
        "background startup timeout cancels continuation and reserves the terminal handoff",
    )?;
    check(
        !state.fail_resume_setup(73, "ui_ready", "Late callback"),
        "duplicate startup failures cannot schedule another handoff",
    )?;
    *state.active.lock().unwrap() = None;
    state.view.lock().unwrap().phase = "recovering".into();
    check(
        state.fail_resume_setup(73, "ui_ready", "Invalid fixture settings"),
        "setup errors return control even after the one-use continuation was consumed",
    )?;
    *state.active.lock().unwrap() = None;
    for phase in ["running", "stopped", "done"] {
        state.view.lock().unwrap().phase = phase.into();
        if phase == "running" {
            *state.active.lock().unwrap() = Some(Arc::new(AtomicBool::new(false)));
        }
        check(
            !state.fail_resume_setup(73, "startup_timeout", "Late callback")
                && state.view.lock().unwrap().phase == phase,
            "late startup timeout leaves active, cancelled and completed tasks unchanged",
        )?;
        *state.active.lock().unwrap() = None;
    }
    Ok(())
}

pub fn run() -> i32 {
    crate::recovery::fixture::checks();
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(async {
        platform::training::fixture_checks()?;
        quiet_resume_checks()?;
        suite().await?;
        if std::env::args().any(|arg| arg == "--with-disposable-input") {
            input_suite().await?;
        }
        Ok::<(), String>(())
    });
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("FAIL {error}");
            1
        }
    }
}

unsafe extern "system" fn editor_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_APP => {
                SetFocus(if wparam == 0 {
                    window
                } else {
                    GetDlgItem(window, 1)
                });
                0
            }
            WM_SIZE => {
                let edit = GetDlgItem(window, 1);
                if !edit.is_null() {
                    MoveWindow(
                        edit,
                        0,
                        0,
                        (lparam & 0xffff) as i32,
                        (((lparam >> 16) & 0xffff) as i32 - 64).max(1),
                        1,
                    );
                }
                let values = GetDlgItem(window, 2);
                if !values.is_null() {
                    MoveWindow(
                        values,
                        0,
                        (((lparam >> 16) & 0xffff) as i32 - 64).max(1),
                        (lparam & 0xffff) as i32,
                        64,
                        1,
                    );
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(window);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

pub fn editor_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--disposable-editor") {
        return false;
    }
    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
        let instance = GetModuleHandleW(std::ptr::null());
        let class = platform::wide("KlickwerkDisposableEditor");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(editor_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: windows_sys::Win32::Graphics::Gdi::GetSysColorBrush(
                windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW,
            ),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            platform::wide("Disposable input fixture — no files are saved").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            80,
            180,
            600,
            360,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        let edit = CreateWindowExW(
            0,
            platform::wide("EDIT").as_ptr(),
            platform::wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32,
            0,
            0,
            580,
            320,
            window,
            1usize as _,
            instance,
            std::ptr::null(),
        );
        CreateWindowExW(
            0,
            platform::wide("STATIC").as_ptr(),
            platform::wide("Process values: 100 MB").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            0,
            256,
            580,
            64,
            window,
            2usize as _,
            instance,
            std::ptr::null(),
        );
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
        SetFocus(edit);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}

async fn execute_fixture(
    broker: &mut BrokerClient,
    sequence: u64,
    mut frame: Frame,
    action: Action,
) -> Result<(), String> {
    frame.captured_ms = platform::now();
    broker
        .sender
        .try_send(Command::Execute {
            sequence,
            sent_ms: platform::now(),
            frame,
            action,
        })
        .map_err(|_| "The input fixture pipe closed.")?;
    let mut timer = tokio::time::interval(Duration::from_millis(80));
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _=&mut deadline=>return Err("The disposable input fixture timed out.".into()),
            _=timer.tick()=>broker.sender.try_send(Command::Heartbeat{sent_ms:platform::now(),lease_sequence:Some(sequence)}).map_err(|_|"Input heartbeat failed.")?,
            result=broker.receiver.recv()=>return match result{Some(Reply::Completed{sequence:ack})if ack==sequence=>Ok(()),other=>Err(format!("Input fixture was refused: {other:?}"))}
        }
    }
}

fn shortcut_validation_suite(window: HWND) -> Result<(), String> {
    use crate::action::Modifier;
    use windows_sys::Win32::Graphics::Gdi::{
        RDW_ALLCHILDREN, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow,
    };
    let launch = Action::Key {
        key: "ESC".into(),
        modifiers: vec![Modifier::Ctrl, Modifier::Shift],
    };
    let redraw = || unsafe {
        RedrawWindow(
            window,
            std::ptr::null(),
            std::ptr::null_mut(),
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
        );
    };
    unsafe {
        SendMessageW(window, WM_APP, 0, 0);
    }
    redraw();
    let before = capture::capture(1, 1280, [0; 4])?;
    check(
        before.focused_control == window as usize && before.focused_element.is_none(),
        "shortcut regression reproduces an unidentified editable region",
    )?;
    unsafe {
        SetWindowTextW(
            GetDlgItem(window, 2),
            platform::wide("Process values: 999 MB, refreshed during inference").as_ptr(),
        );
    }
    redraw();
    let after = capture::capture(2, 1280, [0; 4])?;
    check(
        capture::changed(&before, &after),
        "shortcut regression reproduces changing screen content",
    )?;
    check(
        capture::unchanged(&before, &launch).is_ok(),
        "Task Manager launch accepts repainting with unchanged native focus",
    )?;
    check(
        !capture::input_effect(&before, &after, &launch),
        "repainting alone does not confirm Task Manager launched",
    )?;
    for action in [
        Action::Text { text: "chr".into() },
        Action::Key {
            key: "F".into(),
            modifiers: vec![Modifier::Ctrl],
        },
        Action::Key {
            key: "F4".into(),
            modifiers: vec![Modifier::Alt],
        },
    ] {
        check(
            capture::unchanged(&before, &action).is_err(),
            "content-dependent keyboard input still rejects a changed observation",
        )?;
    }
    unsafe {
        SendMessageW(window, WM_APP, 1, 0);
    }
    check(
        capture::unchanged(&before, &launch).is_err(),
        "desktop shortcuts reject a changed keyboard target",
    )?;
    redraw();
    let mut field = capture::capture(3, 1280, [0; 4])?;
    unsafe {
        SetWindowTextW(
            GetDlgItem(window, 1),
            platform::wide("Changed field during inference").as_ptr(),
        );
    }
    redraw();
    check(
        capture::unchanged(&field, &launch).is_ok(),
        "desktop shortcuts do not depend on focused text contents",
    )?;
    check(
        capture::unchanged(
            &field,
            &Action::Key {
                key: "A".into(),
                modifiers: vec![Modifier::Ctrl],
            },
        )
        .is_err(),
        "select-all still rejects changed field contents",
    )?;
    field.frame.left -= 1;
    check(
        capture::unchanged(&field, &launch).is_err(),
        "desktop shortcuts reject changed monitor layouts",
    )?;
    unsafe {
        SetWindowTextW(GetDlgItem(window, 1), platform::wide("").as_ptr());
        SetWindowTextW(
            GetDlgItem(window, 2),
            platform::wide("Process values: 100 MB").as_ptr(),
        );
    }
    redraw();
    println!("Desktop shortcut validation: 11 checks passed. No desktop shortcut was dispatched.");
    Ok(())
}

async fn input_suite() -> Result<(), String> {
    use base64::Engine;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut editor = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--disposable-editor")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|_| "Could not start the disposable editor.")?;
    let result=async{
        let deadline=Instant::now()+Duration::from_secs(5);
        let window=loop{
            let value=unsafe{FindWindowW(platform::wide("KlickwerkDisposableEditor").as_ptr(),std::ptr::null())};
            // Finding the HWND is not enough: the child may still be creating
            // controls and about to claim focus. Wait for its message loop.
            if !value.is_null() && unsafe { IsWindowVisible(value) } != 0
                && unsafe { SendMessageTimeoutW(value, WM_NULL, 0, 0, SMTO_ABORTIFHUNG | SMTO_BLOCK, 200, std::ptr::null_mut()) } != 0 { break value; }
            if Instant::now()>deadline{return Err("The disposable editor did not open.".into());}
            tokio::time::sleep(Duration::from_millis(30)).await;
        };
        let mut pid=0;unsafe{GetWindowThreadProcessId(window,&mut pid);}
        check(pid==editor.id(),"input fixture targets only its newly created child process")?;
        let target = platform::window::inspect(window as usize, std::process::id());
        check(target.window.process_id == editor.id() && target.window.class_name == "KlickwerkDisposableEditor" && target.window.integrity_level.is_some() && target.input_block.is_none(), "native diagnostics identify the target and allow equal-integrity input")?;
        unsafe {
            let own = CreateWindowExW(0, platform::wide("STATIC").as_ptr(), platform::wide("Disposable handoff fixture").as_ptr(), WS_OVERLAPPEDWINDOW, 150, 200, 400, 250, std::ptr::null_mut(), std::ptr::null_mut(), GetModuleHandleW(std::ptr::null()), std::ptr::null());
            if own.is_null() { return Err("Could not create the handoff fixture.".into()); }
            SetForegroundWindow(window);
            platform::window::minimize_for_control(own as usize)?;
            platform::window::minimize_for_control(own as usize)?;
            check(IsWindowVisible(own) == 0 && IsIconic(own) == 0 && GetForegroundWindow() == window,
                "preparing a background restart keeps its window hidden and the target focused")?;
            let background_handoff = platform::window::handoff(own as usize);
            check(background_handoff.visible && background_handoff.focused,
                "terminal handoff restores a hidden background restart window")?;
            platform::window::minimize_for_control(own as usize)?;
            platform::window::minimize_for_control(own as usize)?;
            check(IsIconic(own) != 0 && GetWindowLongPtrW(own, GWL_EXSTYLE) & WS_EX_TOPMOST as isize == 0,
                "starting control minimizes a visible window once and clears temporary topmost state")?;
            ShowWindow(own, SW_MINIMIZE);
            SetForegroundWindow(window);
            let report = platform::window::handoff(own as usize);
            if !report.focused { println!("Fixture handoff: {report:?}"); }
            let kept_topmost = GetWindowLongPtrW(own, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0;
            let own_block = platform::window::inspect(own as usize, std::process::id()).input_block;
            platform::window::release_handoff_topmost(own as usize);
            let no_topmost = GetWindowLongPtrW(own, GWL_EXSTYLE) & WS_EX_TOPMOST as isize == 0;
            ShowWindow(own, SW_HIDE);
            SetForegroundWindow(window);
            platform::window::raise_for_handoff(own as usize);
            platform::window::raise_for_handoff(own as usize);
            platform::window::handoff_focus_changed(own as usize, false);
            check(IsWindowVisible(own) != 0 && GetWindowLongPtrW(own, GWL_EXSTYLE) & WS_EX_TOPMOST as isize != 0,
                "handoff keeps the app visibly above other windows even without keyboard activation")?;
            SetForegroundWindow(own);
            platform::window::handoff_focus_changed(own as usize, true);
            SetForegroundWindow(window);
            platform::window::handoff_focus_changed(own as usize, false);
            check(GetWindowLongPtrW(own, GWL_EXSTYLE) & WS_EX_TOPMOST as isize == 0,
                "leaving the app or starting again clears temporary topmost state after repeated raises")?;
            DestroyWindow(own);
            check(report.visible && !report.minimized && report.focused && report.foreground_after == own as usize, "handoff restores a minimized window and verifies foreground keyboard focus")?;
            check(kept_topmost && report.topmost_retained && no_topmost && own_block.as_deref() == Some("own_window"), "handoff stays on top until released and agent input still rejects its own window")?;
            SetForegroundWindow(window);
        }
        shortcut_validation_suite(window)?;
        let mut broker=BrokerClient::spawn(false)?;armed(&mut broker).await?;
        let screen=capture::capture(1,1280,broker.bounds)?;
        let edit=unsafe{GetDlgItem(window,1)};let mut rect:RECT=unsafe{std::mem::zeroed()};unsafe{GetWindowRect(edit,&mut rect);}
        let x=((rect.left+60-screen.frame.left) as i64*screen.frame.image_width as i64/screen.frame.width as i64) as i32;
        let y=((rect.top+50-screen.frame.top) as i64*screen.frame.image_height as i64/screen.frame.height as i64) as i32;
        let expected=serde_json::json!({"frame_id":1,"description":"Focus the disposable editor","action":{"type":"click","x":x,"y":y,"button":"left"}}).to_string();
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|_|"Fixture server could not start.")?;
        let settings=Settings{base_url:format!("http://{}/v1",listener.local_addr().unwrap()),model:"fixture-only".into(),..Settings::default()};
        let server=tokio::spawn(async move{
            let (mut socket,_)=listener.accept().await.unwrap();let mut bytes=Vec::new();let mut buf=[0u8;8192];
            let body_start=loop{let n=socket.read(&mut buf).await.unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);if let Some(index)=bytes.windows(4).position(|p|p==b"\r\n\r\n"){break index+4;}assert!(bytes.len()<65536);};
            let headers=String::from_utf8_lossy(&bytes[..body_start]);
            assert!(headers.starts_with("POST /v1/chat/completions "));
            let length=headers.lines().find_map(|line|line.to_ascii_lowercase().strip_prefix("content-length:").map(|v|v.trim().parse::<usize>().unwrap())).unwrap();
            assert!(length<8*1024*1024);
            while bytes.len()<body_start+length{let n=socket.read(&mut buf).await.unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);}
            let body:serde_json::Value=serde_json::from_slice(&bytes[body_start..body_start+length]).unwrap();
            let image=body.pointer("/messages/1/content/1/image_url/url").and_then(|v|v.as_str()).unwrap().split_once(',').unwrap().1;
            let image=base64::engine::general_purpose::STANDARD.decode(image).unwrap();
            assert!(image::load_from_memory(&image).is_ok());
            let response=serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":expected}}]}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).as_bytes()).await.unwrap();
        });
        let provider=Provider::new(&settings)?;
        let decision=provider.decide("Focus the disposable editor","",provider::Observation{frame_id:1,bytes:&screen.jpeg,width:screen.frame.image_width,height:screen.frame.image_height,mime:"image/jpeg"});
        tokio::pin!(decision);let mut timer=tokio::time::interval(Duration::from_millis(80));
        let action=loop{tokio::select!{result=&mut decision=>break result?,_=timer.tick()=>{broker.sender.try_send(Command::Heartbeat{sent_ms:platform::now(),lease_sequence:None}).map_err(|_|"The fixture heartbeat failed.")?;}}};
        server.await.map_err(|_|"The fixture did not receive a valid screenshot request.")?;
        check(action.frame_id==1,"fixture provider receives a decodable screenshot and returns a validated action")?;
        capture::unchanged(&screen,&action.action)?;
        execute_fixture(&mut broker,1,screen.frame.clone(),action.action).await?;
        fixture_key(0);
        check(next(&mut broker,true,150).await.is_err(), "untagged software keys after a broker click leave control running")?;
        check(unsafe{GetForegroundWindow()}==window,"broker click focuses the disposable editor")?;
        let before_typing=capture::capture(2,1280,broker.bounds)?;
        let mut frame=screen.frame.clone();frame.foreground=window as usize;
        execute_fixture(&mut broker,2,frame.clone(),Action::Text{text:"Grüße 世界 🪷".into()}).await?;
        tokio::time::sleep(Duration::from_millis(80)).await;
        let read=||{let mut buffer=vec![0u16;8192];let count=unsafe{SendMessageW(edit,WM_GETTEXT,buffer.len(),buffer.as_mut_ptr() as isize)};String::from_utf16_lossy(&buffer[..count.max(0) as usize])};
        check(read()=="Grüße 世界 🪷","real input preserves Unicode, including surrogate pairs")?;
        check(capture::unchanged(&before_typing,&Action::Text{text:"duplicate".into()}).is_err(),"text input rejects content changed during model inference")?;
        let current=capture::capture(3,1280,broker.bounds)?;
        fixture_heartbeat(&broker)?;
        check(capture::unchanged(&current,&Action::Text{text:"next".into()}).is_ok(),"text input accepts an unchanged screen and focused field")?;
        check(current.focused_element.as_ref().is_some_and(|field| field.source == "win32_edit" && field.process_id == editor.id()), "capture identifies the actual editable region")?;
        unsafe { SetWindowTextW(GetDlgItem(window, 2), platform::wide("Process values: 999 MB, refreshed while typing").as_ptr()); }
        tokio::time::sleep(Duration::from_millis(60)).await;
        let live = capture::capture(4,1280,broker.bounds)?;
        fixture_heartbeat(&broker)?;
        let global_changed = capture::changed(&current, &live);
        let field_changed = capture::input_effect(&current, &live, &Action::Text { text: "chr".into() });
        if !global_changed || field_changed {
            println!("Live field fixture: global_changed={global_changed}, field_changed={field_changed}, before={:?}, after={:?}, foreground_before={}, foreground_after={}", current.focused_element, live.focused_element, current.frame.foreground, live.frame.foreground);
        }
        check(global_changed && !field_changed, "unrelated repainting is not proof that text arrived")?;
        check(capture::unchanged(&current, &Action::Text { text: "chr".into() }).is_ok(), "an unchanged focused field accepts input while the rest of the app updates")?;
        fixture_heartbeat(&broker)?;
        unsafe { MoveWindow(edit, 0, 0, 450, 250, 1); }
        check(capture::unchanged(&current, &Action::Text { text: "chr".into() }).is_err(), "moving or resizing the editable target invalidates the observation")?;
        unsafe { MoveWindow(edit, 0, 0, rect.right - rect.left, rect.bottom - rect.top, 1); }
        execute_fixture(&mut broker,3,frame.clone(),Action::Key{key:"A".into(),modifiers:vec![crate::action::Modifier::Ctrl]}).await?;
        execute_fixture(&mut broker,4,frame.clone(),Action::Text{text:"Line one\nLine two".into()}).await?;
        tokio::time::sleep(Duration::from_millis(60)).await;
        check(read().replace("\r\n", "\n")=="Line one\nLine two","balanced Ctrl+A and multiline text replace the selected content")?;
        check(unsafe{GetAsyncKeyState(VK_CONTROL as i32)}>=0,"controller releases the Ctrl modifier")?;
        frame.captured_ms=platform::now();
        broker.sender.try_send(Command::Execute{sequence:5,sent_ms:platform::now(),frame,action:Action::Text{text:"x".repeat(3000)}}).map_err(|_|"Cannot begin bounded typing fixture.")?;
        tokio::time::sleep(Duration::from_millis(5)).await;
        fixture_hardware_key()?;
        check(matches!(next(&mut broker,false,1500).await?,Reply::Stopped{interruption:Some(event),..} if event.flags == 0),"hardware keyboard flags interrupt a long typing action")?;
        let after=read();tokio::time::sleep(Duration::from_millis(150)).await;
        check(after==read()&&after.len()<3018,"no further text arrives after takeover")?;
        println!("Disposable input fixture: 23 checks passed. Only a fixture server and an unsaved test editor were used.");Ok::<(),String>(())
    }.await;
    let _ = editor.kill();
    let _ = editor.wait();
    result
}

// Offline reproduction reads a caller-selected export but never starts control,
// launches the target app, requests consent or writes to the original export.
pub fn recovery_replay_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some("--recovery-replay") {
        return false;
    }
    let path = std::env::args().nth(2).expect("Provide the export path");
    let file = std::fs::File::open(path).expect("Read export");
    assert!(
        file.metadata().expect("Inspect export").len() <= 64 * 1024 * 1024,
        "The replay export exceeds 64 MiB"
    );
    let export: serde_json::Value = serde_json::from_reader(file).expect("Decode export");
    let mut run: RunView = serde_json::from_value(export["run"].clone()).expect("Decode run");
    run.evidence = Arc::new(
        serde_json::from_value::<crate::session::Evidence>(export["evidence"].clone())
            .expect("Decode evidence"),
    );
    // Reproduce the first recovery request from this paused session, not its retries.
    run.recovery_events
        .retain(|event| event.kind == "privilege_blocked");
    let memory = export["warm_start_prompt"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let session =
        crate::recovery::automatic_session(&mut run, memory, false).expect("Create restart");
    let outcome = crate::recovery::fixture::round_trip(&session, 12288, false);
    outcome.parent.expect("Transfer and commit session");
    crate::recovery::fixture::check_restored(&session, &outcome.child.expect("Restore session"));
    println!(
        "PASS exported session survives native TCP transfer with delayed receipt ({} ms)",
        outcome.elapsed_ms
    );
    true
}
