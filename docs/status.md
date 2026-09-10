# Validation status — 2026-09-09

The C/Win32 application has been replaced with a new Tauri 2/Rust application and
a React/TypeScript interface. This is an executable implementation with real
desktop input, not just a UI mockup. Browser development deliberately uses an
isolated simulated adapter.

## Completed validation

| Check | Evidence |
| --- | --- |
| Windows x64 release | Cross-compiled with cargo-xwin/MSVC; `build/klickwerk.exe` |
| Runtime imports | Static Visual C++ runtime; only Windows system DLL imports, plus the separately installed WebView2 Runtime |
| Frontend checks | TypeScript and ESLint pass |
| Rust checks | Windows-target Clippy, including test targets, passes with warnings denied |
| Portable Rust tests | 14 pass on Linux |
| Windows Rust tests | The same 14 pass as a Windows test EXE under Wine, including DPAPI config round-trip |
| Production frontend regression | 15 Windows release tests pass under Wine, including the 14 core tests and a check of the actual application context: embedded HTML, JS, CSS, production protocol, and no development URL |
| Release build guard | A Windows release without `tauri/custom-protocol` is rejected before it can replace the distributed EXE |
| UI behavior and accessibility | 12 Chromium/Playwright tests pass; axe checks cover the home view, settings dialog, and dark mode |
| Native emergency stop | 14 checks pass on an isolated Wine/Xvfb desktop |
| Native input integration | 8 checks pass with an unsaved fixture-owned editor and loopback scripted provider |
| Visual review | Light/dark home, settings, and narrow layouts inspected; screenshots in `build/visual/` |
| Configuration files | Project JSON parses; private `.env` and original image assets preserved |

Native stop checks cover the independent bar, always-on-top style, visible chord,
STOP button, cancellation during countdown, pre-arming input refusal, heartbeat
loss, pipe loss, a conflicting global shortcut, and actual global shortcut delivery
while another fixture window has focus. The test bar never steals focus.

Input checks decode the real screenshot at a local fixture server, parse a returned
action, execute a click through the actual broker, type Unicode including a
surrogate pair, use Ctrl+A and multiline replacement, verify modifier release,
interrupt long typing, and verify that no further text arrives after STOP.
No private model server, credentials, user files, or ordinary desktop were used.

The initial direct-Cargo release omitted Tauri's production asset feature and
attempted to open the development server instead of an embedded UI. The release
script now enables `tauri/custom-protocol`; the shared production application
context also clears the development URL. The new regression test reads the assets
from that same context in a Windows release test EXE, run from an empty temporary
working directory. CI now runs this test after building the Windows release.
These checks do not launch WebView2 or establish full Windows 11 acceptance.

The final logs, binary size, import list, and SHA-256 are recorded under
`build/validation/`. Reported stop timings are individual Wine fixture observations;
they are not Windows 11 hardware benchmarks or guaranteed worst-case limits.

## Target-PC checks still required

- Actual Windows 11/WebView2 launch, minimized-window heartbeat behavior, cold/warm
  startup time, memory, keyboard/screen-reader behavior, and mixed-DPI display layouts.
- Physical emergency shortcut and takeover under load, unexpected process failure
  during held input, secure desktop transitions, and exclusive full-screen behavior.
- Real model task accuracy, coordinate interpretation, and completion quality.
- SAPI microphone permission, installed speech language, dictation quality, device
  loss, and finalization on real audio hardware.
- The new devcontainer image must be rebuilt in the host editor. Docker is not
  installed/exposed in this session, so an actual image build was unavailable.
  Node, Rust, cross-compilation, browser tests, and native fixtures were run with
  the corresponding tools installed in the current container. The Dev Containers
  CLI reached Docker discovery and reported `spawn docker ENOENT`.
- The checked-in GitHub Actions workflow has not been submitted or executed.

Follow [Windows acceptance](windows-acceptance.md). The architecture intentionally
fails closed when required stop facilities are unavailable, but an ordinary user
application cannot guarantee input handling when Windows itself is frozen.
