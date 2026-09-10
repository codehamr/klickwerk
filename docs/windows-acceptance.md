# Windows 11 acceptance

Use an ordinary, non-elevated Windows account and a disposable desktop with no
unsaved work. Record Windows build, WebView2 version, monitor scale/layout, selected
model, and test result. The development container can compile the EXE but cannot
establish real Windows 11 acceptance or physical hardware latency.

## Configuration and startup

- Copy the EXE into a new writable folder. Start it from another working directory.
  Confirm `config.cfg` is created beside the EXE and nowhere else.
- Close all development servers and disconnect the network before launch. The
  interface must still open from the EXE without a `dist` folder, Node.js, or any
  service on `localhost:1420`. A model connection is only needed for model requests.
- Save a connection, restart, and confirm exact URL/model/preferences persist.
  A saved key should have a `dpapi:` value and should not appear in UI snapshots.
- Make the folder read-only. Confirm a clear error and no hidden fallback config.
- Put malformed TOML in a test config. Confirm startup preserves it and explicit
  replacement saving makes a backup.
- Confirm the original app icon, readable light/dark appearance, keyboard focus,
  minimum window size, and settings scrolling at 100%, 150%, and 200% scale.
- Measure cold/warm time to a usable prompt and total process working set. Include
  the WebView2 child processes. No Windows startup/RAM target has been measured yet.

## Emergency stop without real task input

Settings → Preferences → **Try a safe stop test** starts the actual native broker
in simulation mode for at most 30 seconds, without screenshots or model requests.

- Confirm the stop bar is visible above another focused application and explicitly
  shows **Ctrl + Alt + F8** and **STOP**. It must not steal keyboard focus.
- In separate runs, use the chord, click STOP, move the physical mouse, and press
  an ordinary key. Confirm the run ends and cannot resume automatically.
- Reserve the chord in another application. Startup must fail with an explanation;
  no fallback chord should silently replace it.
- During a test, suspend/terminate the main app. The independent bar should close
  and stop control after the parent exits or the heartbeat expires.
- Test minimization, a busy UI thread, a crashed WebView process, monitor changes,
  locking Windows, suspend/resume, and full-screen windows. Record visibility and
  stop timing; exclusive full-screen and secure desktop are not guaranteed overlays.

The optional native fixture binary is built separately and is not distributed:

```powershell
cargo build --release --manifest-path src-tauri/Cargo.toml --features safety-tests,tauri/custom-protocol --bin klickwerk-safety-tests
.\src-tauri\target\release\klickwerk-safety-tests.exe
```

It checks real hotkey registration, native window properties, countdown
cancellation, premature input refusal, heartbeat/pipe loss, and the stop chord with
focus in a disposable fixture window. Only the balanced stop chord is injected.
Use `--with-disposable-input` **only on an isolated test desktop** to additionally
exercise a local fixture provider, capture, validated click, Unicode input,
Ctrl+A/replacement, and STOP during typing in a fixture-owned unsaved editor.

## Disposable real task

1. Start a compatible vision model server. Use **Test connection**; this is a
   generated-image test and does not establish task accuracy.
2. Open an empty text editor. Ask klickwerk to write a short multilingual greeting
   and leave it open for review. Check coordinates, exact text, completion, and STOP.
3. Repeat with a slow model response and stop before it completes. Confirm no late
   action arrives. Check physical takeover during typing and dragging and ensure
   no key or mouse button remains held.
4. Exercise a question and Continue. The bar must close while waiting for the user
   and return with a fresh countdown for the reply.
5. Check refusal to target the app, the stop bar, monitor gaps, elevated windows,
   and changed screen targets. Repeat with mixed-DPI and negative-origin monitors.
6. Validate SAPI dictation using the configured Windows speech language, microphone
   permission denial, cancellation, device removal, and offline operation.

Do not claim reliable live-model operation, physical stop latency, or full Windows
acceptance from a successful build, browser preview, scripted model, or Wine run.
