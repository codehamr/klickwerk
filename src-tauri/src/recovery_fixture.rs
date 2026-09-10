//! Exercises the production restart protocol without elevation or desktop input.
use super::*;
use crate::{
    action::{Action, Modifier},
    config::Settings,
    diagnostics::{TargetInfo, WindowInfo},
    guard::Frame,
};
use std::{net::TcpListener, time::Instant};

pub fn session() -> RestartSession {
    let frame = Frame {
        id: 13,
        captured_ms: 37671812,
        left: 0,
        top: 0,
        width: 3840,
        height: 2160,
        image_width: 1280,
        image_height: 720,
        foreground: 100,
    };
    let mut run = RunView { id: 3, phase: "running".into(),
        task: "Open Task Manager, filter processes by \"chr\", sort Memory descending and report the top process.".into(),
        ..RunView::default()
    };
    run.begin_attempt(
        &Settings::default(),
        1,
        None,
        "Verify the literal filter and descending order.",
    );
    for (action, description, status) in [
        (
            Action::Key {
                key: "ESC".into(),
                modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            },
            "Open Task Manager",
            "completed",
        ),
        (
            Action::AskUser {
                question: "Approve Windows administrator access to continue.".into(),
            },
            "Recover input permissions",
            "skipped",
        ),
    ] {
        let mut step = Step::action(
            run.steps.len() as u32 + 1,
            description.into(),
            action,
            &frame,
            1000,
        );
        step.status = status.into();
        run.steps.push(step);
    }
    let failure = InputFailure::target(TargetInfo {
        window: WindowInfo {
            handle: 100,
            process_id: 200,
            title: "Task Manager".into(),
            class_name: "TaskManagerWindow".into(),
            executable: "Taskmgr.exe".into(),
            integrity_level: Some(12288),
            elevated: Some(true),
            ..WindowInfo::default()
        },
        sender_integrity_level: Some(8192),
        input_block: Some("higher_integrity".into()),
    });
    run.block_for_privileges(failure, "model_handoff");
    run.finish_attempt();
    // A synthetic JPEG keeps the full evidence path active without private screenshots.
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode(&[30, 40, 50], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    Arc::make_mut(&mut run.evidence).retain(&frame, &jpeg);
    automatic_session(
        &mut run,
        "Synthetic transfer context. ".repeat(24000),
        false,
    )
    .unwrap()
}

// Deliberate delays expose inherited nonblocking sockets deterministically. Short
// reads also exercise framing with a payload larger than typical socket buffers.
struct DelayedChild {
    stream: TcpStream,
    first_write: bool,
    cancel_on_receipt: bool,
    cancel: Arc<AtomicBool>,
}
impl Read for DelayedChild {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = bytes.len().min(8192);
        self.stream.read(&mut bytes[..count])
    }
}
impl Write for DelayedChild {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.first_write {
            self.first_write = false;
            if self.cancel_on_receipt {
                self.cancel.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

pub struct Outcome {
    pub parent: Result<(), InputFailure>,
    pub child: Result<RestartSession, InputFailure>,
    pub elapsed_ms: u128,
}

pub fn round_trip(session: &RestartSession, integrity: u32, cancel_on_receipt: bool) -> Outcome {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    configure_transfer_stream(&client).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    configure_transfer_stream(&server).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let child_cancel = cancel.clone();
    let child = std::thread::spawn(move || {
        // Same fixed-length authentication preamble as the native launcher.
        std::thread::sleep(Duration::from_millis(25));
        client.write_all(&[b'a'; 32]).unwrap();
        std::thread::sleep(Duration::from_millis(25));
        client.write_all(&[b'a'; 32]).unwrap();
        receive_committed_transfer(
            &mut DelayedChild {
                stream: client,
                first_write: true,
                cancel_on_receipt,
                cancel: child_cancel,
            },
            integrity,
        )
    });
    let started = Instant::now();
    let mut token = [0; 64];
    let parent = server
        .read_exact(&mut token)
        .map_err(|error| io_failure("restart_auth_read_failed", error))
        .and_then(|()| {
            assert_eq!(token, [b'a'; 64]);
            prepare_transfer(&mut server, session)
        })
        .and_then(|()| commit_transfer(&mut server, &cancel));
    drop(server);
    Outcome {
        parent,
        child: child.join().unwrap(),
        elapsed_ms: started.elapsed().as_millis(),
    }
}

pub fn check_restored(session: &RestartSession, restored: &RestartSession) {
    assert_eq!(restored.run.id, session.run.id);
    assert_eq!(restored.run.task, session.run.task);
    assert_eq!(restored.run.phase, "recovering");
    assert!(restored.resume_after_approval);
    assert!(restored.run.recovery.is_none());
    assert_eq!(restored.run.steps.len(), session.run.steps.len() + 1);
    assert_eq!(
        serde_json::to_value(&restored.run.steps[..session.run.steps.len()]).unwrap(),
        serde_json::to_value(&session.run.steps).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&restored.run.attempts).unwrap(),
        serde_json::to_value(&session.run.attempts).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&*restored.run.evidence).unwrap(),
        serde_json::to_value(&session.evidence).unwrap()
    );
    assert_eq!(restored.memory, session.memory);
    assert_eq!(restored.refinement, session.refinement);
}

pub fn checks() {
    let mut session = session();
    let outcome = round_trip(&session, 12288, false);
    outcome.parent.unwrap();
    check_restored(&session, &outcome.child.unwrap());
    println!(
        "PASS nonblocking listener transfers actions, attempts and evidence with delayed authentication and ACK ({} ms)",
        outcome.elapsed_ms
    );

    let outcome = round_trip(&session, 12288, true);
    assert_eq!(outcome.parent.unwrap_err().code, "restart_cancelled");
    assert!(matches!(outcome.child, Err(failure) if failure.code == "restart_cancelled"));
    println!("PASS Stop during delayed receipt prevents continuation in the new instance");

    let outcome = round_trip(&session, 8192, false);
    assert_eq!(
        outcome.parent.unwrap_err().code,
        "restart_integrity_invalid"
    );
    assert!(outcome.child.is_err());
    println!("PASS child permission failure reaches the original instance");

    session.version = 1;
    let outcome = round_trip(&session, 12288, false);
    assert_eq!(outcome.parent.unwrap_err().code, "restart_version_mismatch");
    assert!(outcome.child.is_err());
    println!("PASS incompatible restart protocol reaches the original instance without commit");

    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let _silent_child = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut server, _) = listener.accept().unwrap();
    configure_transfer_stream(&server).unwrap();
    assert_eq!(
        server.read_timeout().unwrap(),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        server.write_timeout().unwrap(),
        Some(Duration::from_secs(10))
    );
    server
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let started = Instant::now();
    let error = server.read_exact(&mut [0]).unwrap_err();
    assert!(started.elapsed() >= Duration::from_millis(50));
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    let failure = io_failure("restart_ack_failed", error);
    assert!(failure.io_error_kind.is_some());
    #[cfg(windows)]
    assert!(failure.win32_error.is_some());
    println!("PASS missing receipt waits for the configured timeout and preserves the OS error");
}

#[test]
fn restart_transport_regressions() {
    checks();
}
