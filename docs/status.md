# Validation status — 2026-09-10

The interface now returns control explicitly after takeover, questions, success and
failure. Workflows consolidate temporary session evidence into one saved start
prompt. English/German UI detection and override, bilingual suggestions, paced text,
visual settling and stale text-input checks are implemented.

Settings now save automatically, with debounced text edits, immediate selections,
serialized writes and a close-time flush. Failed saves retain the draft for retry
or explicit discard. Timeout and step labels include spaces in both languages,
and narrow settings layouts keep the full unit and save status readable.

Settings now need no completion button. Styled preference menus and searchable
model selection retain keyboard focus and support nested Escape dismissal. Session
history can be exported as JSON at every handoff, including all recorded steps,
corrections, attempt outcomes/settings/warm starts and the current workflow.

Foreground handoff now restores, raises and activates the native window on its UI
thread, verifies actual keyboard focus, and immediately restores its topmost flag.
Input eligibility compares Windows integrity levels, with structured refusals and
manual continuation for higher-privilege targets. The model receives native window
identity/permissions and explicit filter/sort/result verification instructions.
Schema 2 reports add timing, target metadata, handoff verification and up to 12 recent
JPEG observations within an 8 MiB base64 budget, with explicit omission counts.

## Completed validation

| Check | Evidence |
| --- | --- |
| Windows x64 release | `build/klickwerk.exe`, 6,146,560 bytes, current frontend embedded with `tauri/custom-protocol` |
| Frontend checks | TypeScript and ESLint pass |
| Browser behavior and accessibility | 31 Chromium/Playwright tests pass, including axe checks for open menus and actual JSON downloads |
| Portable Rust tests | 29 pass on Linux, including integrity decisions, logged coordinate mapping, bounded image evidence, credential exclusion and atomic writes |
| Windows static checks | Windows-target Clippy for all targets and native fixtures passes with warnings denied |
| Native input monitoring | 13 checks pass under Wine/Xvfb |
| Native input integration | 13 checks pass with a fixture-owned unsaved editor, scripted loopback provider, foreground restoration and integrity inspection |
| Visual review | English light/dark/narrow screens and German home, settings, handoff and workflow dialog; `build/visual/` |

Browser tests cover URL/key/model order, focus, takeover, unchanged continuation,
refinement after success, questions and failures, finalization for all outcomes,
unsent corrections, updates, deletion of selected/current workflows, cancellation,
failed-learning fallback and stale draft refusal after reset or deletion. German
locale detection, manual override and persistence preserve user-authored text.
Autosave checks cover rapid typing, slow overlapping edits, close-time flushing,
retry/discard after failure, immediate preferences and DE/EN number/unit labels.
Menu checks cover keyboard/mouse selection, filtering, custom IDs, late model
responses and dialog focus. Export checks cover corrections, repeated failures,
per-attempt settings, saved workflows, German text, cancellation/error recovery
and stale-session refusal. Portable Rust and Windows Clippy checks were rerun.
Input monitor and integration fixtures were rerun under Wine/Xvfb; native Save As
and real Windows 11 foreground behavior still need target-PC acceptance.

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
content with the same focused field is accepted. Added checks verify target identity,
equal-integrity eligibility, minimized-window restoration, actual keyboard focus,
ordinary z-order restoration and refusal of own-window input. No personal model credentials or
user documents were used.

The browser suite uses `PLAYWRIGHT_OUTPUT_DIR=/tmp/klickwerk-handoff-final-playwright`
to avoid a previous bind-mount output lock. The first run exposed a test that filled
the background composer before the asynchronously closing Settings dialog had
finished; it now awaits dialog dismissal. All 31 tests passed on the full rerun.
The normal production build embeds the current frontend. Executable identity and
validation counts are recorded in `build/validation/handoff-diagnostics-release.json`.

SHA-256: `db7881b2470a95ce4740532c0ccac5c8373b5551e75ae994e807825b16298233`

## Target-PC checks still required

- Native Save As dialog, cancellation, Unicode destinations, existing files and
  folder permissions for session JSON export on Windows 11/WebView2.
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
