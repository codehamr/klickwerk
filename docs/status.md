# Validation status — 2026-09-11

## Demonstration training and portable workflows

The current `build/klickwerk.exe` includes explicit demonstration recording,
reviewable model consolidation, unrun prompt saving without a model, and portable
workflow import/export. Files now live independently in `workflows/` beside the
EXE; legacy libraries migrate without changing the original. See
[training and portable workflows](training.md).

Validation: 58 portable Rust tests pass, including bounded demonstration evidence,
strict import parsing, filename collisions, migration, external-file preservation
and local-stub requests with optional training images. The 44 Chromium cases have
been verified (43 passed together; the existing StrictMode recovery bootstrap
fixture passed separately after restarting Vite to clear HMR module identities).
The six new UI cases cover unrun saves, recording controls, review before saving,
failure/retry/fallback, portable round trips, invalid files, German labels,
cancellation and stale drafts. TypeScript/ESLint, rustfmt, and Windows-target
Clippy with warnings denied pass. English and German dialog layouts were inspected,
including dark mode at 760 × 640; the training notice contrast was improved.

The isolated Wine/Xvfb native suite passes its recording checks without desktop
input: recognized/password/unknown field filtering, physical coordinates, heartbeat
shutdown, hook/panel/hotkey teardown, and temporary portable-file creation, collision
handling and update/reload. The existing broker/recovery suite also runs on that
isolated desktop. No real model, user desktop or user files are used by these tests.
Third-party application capture, real WebView2/SAPI behavior and actual model
consolidation quality still require harmless target-PC acceptance. Literal recording
intentionally omits custom/unknown text fields; see the capture limitations.

Release: `build/klickwerk.exe`, 6,638,080 bytes. SHA-256:
`7eb1bba9b994b40c0cf544e7cfa38089b55f8f975a3050dca2c927afc2079ef5`.

## Previous quiet recovery follow-up

Revision `2026-09-quiet-recovery-v8` fixes the intermediate window appearance in
the latest successful Task Manager run. The old instance raised its window before
deciding to restart automatically, and the elevated replacement then started with
a visible, focused main window. Automatic recovery now skips terminal presentation
and starts the replacement hidden and unfocused. Final outcomes still restore the
app in front. See the [export diagnosis](session-export.md#quiet-administrator-recovery).

Hidden and already minimized windows are left alone when control starts. Terminal
state is published before presentation, the activity gate stays reserved through
foreground retries, and timed-out window callbacks are discarded. No repeated
taskbar attention is requested. A startup failure or 15-second UI-readiness timeout
cancels automatic continuation and brings the paused/error state into view.
The new timeout message has English and German UI text.

Validation: 52 portable Rust tests, 38 Chromium tests, TypeScript/ESLint and
Windows-target Clippy with warnings denied pass. Wine/Xvfb passes 70 checks:
9 startup/recovery checks, 22 input-monitor checks, 23 disposable editor/handoff
checks, 11 shortcut checks and 5 restart transport checks. These cover hidden
startup configuration, stale/duplicate timeout rejection, the failed setup path,
preserved target focus while preparing a hidden window, and terminal restoration
of that window. Windows UAC and WebView2 startup with competing application focus
still need target-PC acceptance; the native fixture does not run a real WebView.

The Windows x64 release is `build/klickwerk-v8.exe`; `build/klickwerk.exe` was also
updated to the same build. The standard build encountered EACCES while removing
the already empty `dist/assets` directory. Rebuilding the frontend with Vite's
`emptyOutDir: false` reused that directory, then the normal production Rust build
completed with the new frontend embedded. Artifact identity, validation results
and the feedback export hash are recorded in
`build/validation/quiet-recovery-release.json` with adjacent test/build logs.

## Earlier injected keyboard follow-up

Revision `2026-09-injected-keyboard-v7` fixes the newly identified takeover trigger:
an untagged `WM_KEYDOWN` with `LLKHF_INJECTED` arrived in the same millisecond as
the agent's search-field click. The earlier controller treated every untagged key
as user input. Windows-marked injected keyboard events now pass through without
claiming takeover, regardless of their tag or timing. Physical keys still stop
control immediately after countdown, including while agent input is running.
The two-second countdown, 100-pixel mouse tolerance and foreground handoff remain
in effect. See the [export diagnosis](session-export.md#injected-keyboard-regression).

The new native regression fails with the old controller and passes with the fix.
It exercises real SendInput callbacks after tagged input and an actual broker
click, followed by successful text entry. Hardware keyboard flags are supplied
through a test-only message to the same production event handler; this does not
emulate a physical keyboard driver. The fixture now also sends heartbeats between
successive screenshot comparisons, avoiding a test-side liveness timeout.

Validation: all 52 portable Rust tests, TypeScript/ESLint and Windows-target Clippy
with warnings denied pass. Wine/Xvfb passes 22 input-monitor checks, 20 disposable
editor/handoff checks, 11 shortcut checks and 5 restart transport checks. No UI code
changed in this follow-up; the preceding 38 Chromium checks remain the UI baseline.
Actual Windows 11 physical input, software keyboards and live-model Task Manager
filter/sort/result completion still require target-PC acceptance.

The Windows x64 release is `build/klickwerk-v7.exe`; `build/klickwerk.exe` was also
updated to the same build. Artifact identity, validation results and the feedback export hash are recorded in
`build/validation/injected-keyboard-release.json` with adjacent test/build logs.

## Earlier input and foreground follow-up

Revision `2026-09-input-handoff-v6` follows a run that successfully launched Task
Manager and resumed with administrator permission, then stopped immediately after
focusing search. The older report lacks the underlying interruption event.
The controller now ignores mouse/keyboard interruption during the two-second
countdown and tolerates mouse movement within a 100-pixel radius afterward.
Larger movement, clicks, scrolling and keyboard input still stop control. Stop and
connection/desktop guards remain active during startup. English/German UI copy and
browser preview follow the same behavior. Exports record the actual interruption
trigger without keyboard codes or typed content.

Terminal handoff now retains temporary topmost state and retries activation,
including a visible fallback when keyboard focus is denied. Leaving the app or
starting another run clears the temporary state. See the
[export diagnosis](session-export.md#input-interruption-and-foreground-follow-up).

Validation: all 50 portable Rust tests, 38 Chromium tests, TypeScript/ESLint and
Windows-target Clippy with warnings denied pass. Wine/Xvfb passes 21 input-monitor
checks, 19 disposable editor/handoff checks, 11 shortcut validation checks and 5
restart transport checks. The native tests cover the 100/101-pixel boundary,
successive small movements, immediate buttons/wheel/keyboard, countdown grace,
explicit Stop, interruption metadata, visibility without activation and temporary
topmost cleanup. Actual Windows 11 UAC, competing application focus behavior and
live-model Task Manager filter/sort/result completion remain target-PC checks.

The Windows x64 release is `build/klickwerk-update.exe`. Replacing the existing
`build/klickwerk.exe` returned EACCES, so it was preserved and the completed build
was copied to this alternate filename. Artifact identity, validation results and the feedback export hash are recorded in
`build/validation/input-handoff-release.json` with adjacent test/build logs.

## Earlier desktop shortcut follow-up

Revision `2026-09-desktop-shortcuts-v5` fixes the September 11 run rejecting all
three Ctrl+Shift+Escape proposals before any input was dispatched. Background
repainting triggered full-screen keyboard content validation because no editable
field was identified. Explicitly recognized desktop navigation shortcuts now check
native target identity, focus, geometry and layout independently of pixels. Literal
text and app-specific shortcuts retain content checks; input permissions, takeover,
post-launch observation and duplicate prevention still apply.

The new native regression reproduces the original failure with the old controller
and a repainting disposable window with no identified editable region; it passes
with the fix. Validation: all 48 portable Rust tests, TypeScript/ESLint,
Windows-target Clippy with warnings denied, and the Wine/Xvfb native suite pass
(11 new shortcut checks, 17 disposable editor checks,
13 input monitoring checks and 5 restart transport checks). The shortcut fixture
tests production preflight validation without dispatching global shortcuts. Actual
Windows 11 live-model launch, `chr` filtering, descending Memory sort and top-row
reporting still require a run on the target PC. See the
[export diagnosis](session-export.md#desktop-shortcut-validation-regression) and
[Windows acceptance](windows-acceptance.md#delayed-launch-and-live-filtering-regression).

The rebuilt Windows x64 release is `build/klickwerk.exe`; artifact identity,
validation results and the feedback export hash are recorded in
`build/validation/desktop-shortcuts-release.json` with adjacent test/build logs.

## Earlier restart transport follow-up

Revision `2026-09-restart-transport-v4` fixes the accepted restart socket inheriting
nonblocking mode on Windows. The latest export records two `restart_ack_failed`
failures before any filter input. A native probe reproduced error 10035 within
10 ms; resetting that same socket to blocking mode received the delayed ACK at
251 ms. Both restart paths now use the corrected stream setup. Errors preserve
I/O kind, Windows code and bounded child decode/restore diagnostics in the export.

Validation: 46 portable Rust tests, all 36 Chromium tests, TypeScript/ESLint and
Windows-target Clippy with warnings denied pass. The Wine/Xvfb native suite passes
5 restart transport checks, 13 input monitoring checks and 17 disposable editor
checks. The original `build/debug.json` also passes an offline transfer replay
through the production protocol in the Windows fixture (301 ms), retaining the
task, actual actions, attempts, screenshots and saved instructions.

The release artifact is `build/klickwerk.exe`; its identity and validation are
recorded in `build/validation/restart-transport-release.json`. The standard native
suite now covers restart sockets, which the earlier validation omitted. The replay
supplies the expected child integrity and does not launch UAC or a WebView. Actual
Windows 11 process replacement and live-model Task Manager filter/sort/result
completion remain target-PC acceptance checks. See the
[export diagnosis](session-export.md#restart-transport-regression).

## Earlier consent recovery follow-up

Revision `2026-09-consent-and-resume-v3` fixes the latest exported Task Manager
run stopping at a confirmed Medium-to-High integrity conflict without requesting
an administrator restart. The controller now requests ordinary Windows consent
once, transfers the original session, and continues after the restored UI is ready.
Stop and declined consent keep control paused; previous failures are historical
context for the new capture, not reasons to repeat the launch or permission refusal.

Validation for this change: 42 portable Rust tests, all 36 Chromium tests,
TypeScript/ESLint and Windows-target Clippy with warnings denied pass. The native
input suite also passed under Wine/Xvfb (13 monitoring and 17 disposable editor
checks). Added tests cover same-task/evidence retention, one automatic request,
Stop before/during/after transfer preparation, missing commits, old protocol peers,
duplicate UI notifications and actual restored bootstrap under React StrictMode.
The normal Windows x64 release is rebuilt at `build/klickwerk.exe`; its identity is
recorded in `build/validation/consent-recovery-release.json`.

Real Windows 11 UAC/WebView2 startup and live-model filter/sort/result completion
still require target-PC acceptance. Wine/editor fixtures and browser mocks do not
establish those results. See [Windows acceptance](windows-acceptance.md) and the
[export diagnosis](session-export.md#consent-and-continuation-regression).

## Earlier handoff diagnostics baseline

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
