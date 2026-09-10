# Validation status — 2026-09-10

The interface now uses one server connection flow, physical input takeover,
continuous correction history, and reusable workflows with editable start prompts
and model-generated memory. Browser development uses an isolated simulation.

## Completed validation

| Check | Evidence |
| --- | --- |
| Windows x64 release | `build/klickwerk.exe`, 6,033,408 bytes; current React assets embedded with `tauri/custom-protocol` |
| Frontend checks | TypeScript and ESLint pass |
| Browser behavior and accessibility | 16 Chromium/Playwright tests pass; the three workflow tests also pass after the final correction-retention change |
| Portable Rust tests | 20 pass on Linux |
| Windows release tests | 21 pass under Wine, including DPAPI persistence and verification of embedded HTML/JS/CSS without a development server |
| Windows static checks | Windows-target Clippy, including native fixtures and test targets, passes with warnings denied |
| Native input monitoring | 13 checks pass on an isolated Wine/Xvfb desktop |
| Native input integration | 8 checks pass with a fixture-owned unsaved editor and loopback scripted provider |
| Visual review | Light/dark home, settings, narrow layout, and correction view; screenshots in `build/visual/` |

Native checks cover invisible monitoring, countdown cancellation, premature input
refusal, heartbeat/pipe loss, and distinguishing tagged agent events from external
mouse/keyboard input without a reserved shortcut. Keyboard and mouse takeover
measured 14 ms and 1 ms in one isolated run. These are fixture observations, not
physical Windows hardware benchmarks or guaranteed worst-case limits.

Input integration sends a real screenshot to a scripted local provider, executes
the returned click, types Unicode and surrogate pairs, replaces text using Ctrl+A,
checks multiline input and released modifiers, and interrupts ongoing typing.
It exposed and verified a fix for untagged duplicate mouse-position notifications
that previously caused the agent to interrupt itself.

Workflow checks cover full text/coordinate context, preserved corrections when
action history is truncated for model requests, model-generated instructions,
atomic save/reload, editable prompts, cancellation, deletion, and retention of a
saved correction when the user later refines it again. Both old and new corrections
stay in one workflow. Stored model credentials are never used by test fixtures.

A Windows bind-mount lock prevented Vite from removing the existing `dist/assets`
directory. The current frontend was built in `/tmp/klickwerk-refinement-ui`, then
copied into `dist` with only its current generated assets. Browser test artifacts
were likewise redirected to `/tmp`; the repository's existing locked directories
were left in place. This does not affect the embedded production interface.
The executable's SHA-256 and size are recorded in
`build/validation/refinement-release.json`.

## Target-PC checks still required

- Windows 11/WebView2 launch and minimized-window heartbeat behavior.
- Physical keyboard/mouse takeover under load and during held input; secure
  desktop, display/session transitions, and full-screen applications.
- Mixed-DPI and negative-origin monitor layouts on actual hardware.
- Real-model accuracy, coordinate interpretation, inference latency, completion
  judgment, and the usefulness of generated workflow instructions.
- SAPI microphone permissions, installed languages, recognition quality, and
  actual audio device behavior.

The GitHub Actions workflow has not been submitted or executed in this session.
Follow [Windows acceptance](windows-acceptance.md). A successful compile, scripted
provider, browser simulation, or Wine run does not establish those target-PC results.
