# Validation status — 2026-09-10

The interface now returns control explicitly after takeover, questions, success and
failure. Workflows consolidate temporary session evidence into one saved start
prompt. English/German UI detection and override, bilingual suggestions, paced text,
visual settling and stale text-input checks are implemented.

Settings now save automatically, with debounced text edits, immediate selections,
serialized writes and a close-time flush. Failed saves retain the draft for retry
or explicit discard. Timeout and step labels include spaces in both languages,
and narrow settings layouts keep the full unit and save status readable.

## Completed validation

| Check | Evidence |
| --- | --- |
| Windows x64 release | `build/klickwerk.exe`, 6,035,968 bytes, current frontend embedded with `tauri/custom-protocol` |
| Frontend checks | TypeScript and ESLint pass |
| Browser behavior and accessibility | 23 Chromium/Playwright tests pass, including axe checks |
| Portable Rust tests | 24 pass on Linux |
| Windows static checks | Windows-target Clippy for all targets and native fixtures passes with warnings denied |
| Native input monitoring | 13 checks pass under Wine/Xvfb |
| Native input integration | 10 checks pass with a fixture-owned unsaved editor and scripted loopback provider |
| Visual review | English light/dark/narrow screens and German home, settings, handoff and workflow dialog; `build/visual/` |

Browser tests cover URL/key/model order, focus, takeover, unchanged continuation,
refinement after success, questions and failures, finalization for all outcomes,
unsent corrections, updates, deletion of selected/current workflows, cancellation,
failed-learning fallback and stale draft refusal after reset or deletion. German
locale detection, manual override and persistence preserve user-authored text.
Autosave checks cover rapid typing, slow overlapping edits, close-time flushing,
retry/discard after failure, immediate preferences and DE/EN number/unit labels.
The settings update changes only the frontend; the Rust, Clippy and native fixture
results below are retained from the preceding session-flow validation.

Rust tests cover workflow migration and atomic persistence without raw history,
correction retention, a whole-session finalization overview, explicit-instruction
fallback, and bounded visual settling that tolerates caret changes while detecting
local text changes. Provider fixtures verify actual input, corrections, outcome and
edited task context without a screenshot in the finalization request.

Native fixtures verify invisible input monitoring, tagged versus external input,
countdown cancellation, heartbeat/pipe loss and takeover. The integration fixture
sends a real screenshot to a scripted provider, clicks its disposable editor,
types Unicode and multiline text, checks released modifiers and interrupts long
typing. It also verifies that changed content rejects stale typing while unchanged
content with the same focused field is accepted. No personal model credentials or
user documents were used.

A previous bind-mount lock prevented Playwright from removing `test-results`.
The autosave suite uses
`PLAYWRIGHT_OUTPUT_DIR=/tmp/klickwerk-autosave-tests-final` instead. The normal
production build completed successfully. The executable hash and validation counts
are recorded in `build/validation/settings-autosave-release.json`.

SHA-256: `0e97f54c3222c4aa01e8a66d7218d9a86d5199acdbc262f898e5726098dfacaa`

## Target-PC checks still required

- Real Windows 11/WebView2 launch, minimized-window heartbeats and foreground focus
  after each handoff, including full-screen applications and Windows focus denial.
- Physical mouse/keyboard takeover under load and during held input; secure desktop,
  display/session transitions, mixed DPI and negative monitor origins.
- Real-model accuracy, task completion judgment and the quality of consolidated
  workflow prompts. Scripted tests do not establish these results.
- Slow application rendering, persistent animations and delayed input on target
  applications. A quiet screen cannot prove all background work has completed.
- SAPI microphone permissions, installed languages, recognition and audio devices.

The GitHub Actions workflow was not run in this session. Follow
[Windows acceptance](windows-acceptance.md). Wine and browser checks do not establish
physical Windows hardware or WebView2 acceptance.
