# Session history export

Choose **Export history (JSON)** below the task result. It is available after
success, failure, a question or takeover, including after saving/updating the
workflow. Windows opens a native Save As dialog; the browser preview downloads a
fixture report. Cancelling leaves the session unchanged, and failed writes can be
retried. Existing files are replaced only after the complete report is written.

Export before choosing New task, deleting the workflow or closing the app.
An administrator restart transfers the session in memory and preserves
its history and bounded evidence without requiring an export first. Raw
history remains temporary until explicitly exported; workflow storage still
contains only consolidated start prompts. A saved workflow without its current
session has no old action history to export.

## JSON schema version 3

| Field | Contents |
| --- | --- |
| `schema_version` | `3`; version of this report format |
| `exported_at` | Unix timestamp in milliseconds |
| `app` | Application name, version and `windows` or `preview` platform |
| `run` | Original task, session ID, current outcome, message, question/result, duration, start time, workflow association, steps and attempts |
| `run.training` | Optional demonstration counts, elapsed time and stop reason; no raw input or images |
| `evidence.training` | Optional explicit demonstration: options, timestamped mouse/keyboard/focus events, bounded cropped screenshots and omission counts; see [training](training.md) |
| `run.steps` | Every recorded step in order, with actor, description, action and its arguments, status, elapsed time, image dimensions and desktop coordinates |
| `run.recovery` | Current structured privilege block, including sender/target integrity; cleared when continuing or after a successful administrator restart |
| `run.attempts[].interruption` | Optional interruption event/flags/time, time since tagged agent input, pointer position and anchor; no keyboard codes or typed text |
| `run.recovery_events` | Timestamped privilege blocks, automatic/manual consent requests, failed/cancelled launches with Windows error codes, successful transfer/restoration and automatic/manual continuation |
| `run.attempts` | Every initial start and continuation, with timestamps, step range, user reply, outcome, model/server configuration and warm-start prompt used at that time |
| `workflow` | Associated workflow and its current consolidated prompt, or `null` |
| `warm_start_prompt` | Current learned instructions retained by the session |
| `unsent_refinement` | Current text in the refinement field, or `null`; it may also have been incorporated into a saved workflow, and does not imply that any action used it |
| `run.steps[].diagnostics` | Optional capture geometry, foreground/focused control, model latency, observation age, validation/input timing, inspected targets structured rejection, focused editable region, sampling trace, repeat check and input acknowledgement time |
| `run.attempts[].handoff` | Native activation method, foreground before/after, visibility, minimized state, verified focus and input-queue attachment error |
| `run.attempts[].terminal_desktop` | Foreground identity and permissions before restoring klickwerk, terminal frame ID or capture error |
| `evidence` | Observed/omitted frame and response counts; recent screenshots as base64 JPEG with purpose, frame IDs and geometry; bounded assistant text with response model, finish reason and parsing error |
| `coverage` | Full recorded history, bounded screenshot/assistant-text coverage, controller revision, capture backend and clock conventions; the browser preview reports real evidence as unavailable |

Unlike bounded context sent to the model during a run, exported history is not
summarized or truncated. The session's existing 1,000-step capacity still applies.
Corrections appear as user steps; `attempts[].user_reply` also preserves repeated
submissions and explicit continuation without a correction. Earlier failures,
questions and successes remain in their attempt records after refinement.

`completed` means that input was sent, not that its visible outcome was verified.
Interrupted input can be partial. Suppressed duplicate actions remain `skipped`.
An initial finish proposal also remains `skipped`, with rejection code
`completion_review_required`. The following `completion_review` screenshot is a
fresh observation for checking the result; its model response can finish, request
help, or continue with a check or repair. This review uses the configured model
and step budget and does not independently prove that a saved file is correct.
Wall-clock timestamps use Unix milliseconds; step elapsed times are cumulative
active-session durations, including previous attempts and excluding paused time.

Reports include user-authored task/action text and the selected server address.
Connection credentials are excluded by an explicit configuration field list.
Screenshots, window titles and entered text can themselves contain personal or
sensitive content. Recent decision screenshots stay in RAM, up to 48 frames and
8 MiB of base64 image data across the whole session, and become persistent only
when exported. Oldest frames are evicted; oversized images are omitted. The counters
make these gaps explicit. Frame IDs are unique capture IDs, independent of step IDs.
Sampling traces record intermediate frame IDs/times, foreground handles, focus,
changes and observed input effects; their JPEGs are not retained. Retained images
are labeled `model_observation`, `completion_review` or `before_handoff`. A failed/interrupted model
request records a system step with its observation. Terminal capture is attempted
with a two-second timeout after input release and before restoring klickwerk;
its failure is explicit. These images are not a video or proof of task success. Screenshots are excluded from progress snapshots,
workflow files and ordinary text-only consolidation. Explicit demonstration screenshots are stored separately and sent when consolidating that demonstration. The latest 48 assistant message texts are retained, each limited to 32 KiB at UTF-8
boundaries, with explicit truncation and eviction counts. Parsing failures retain
the malformed text as well. The configured API key is redacted from retained
response text/metadata. No audio, raw HTTP exchanges or private model reasoning
are retained. The app does not upload reports; share the JSON file explicitly when
using it to diagnose or improve the application.

Revision `2026-09-artifact-review-v9` increases the frame/response count from 12 to
48 while keeping the 8 MiB screenshot byte budget. Short runs can retain their
setup, view changes and final result together. Earlier schema-3 reports remain
readable; their `coverage` strings describe the old limits. Model context now uses
compact action records, app identity and relevant observation/rejection results;
full physical geometry, target lists and timing remain in the export.

Windows uses the [Common Item Dialog](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog)
with JSON filtering, normal overwrite confirmation, and cancellation detection.
Portable tests verify history preservation, credential exclusion and atomic file
replacement; browser tests parse actual downloaded reports. The native dialog's
appearance, cancellation and folder permissions still need target-PC acceptance.

## Diagnosing a blocked target

Each inspected target includes the root HWND, process ID, executable basename,
window title/class/bounds, numeric integrity level and elevation flag. The sender's
integrity level and inspection errors distinguish `higher_integrity`, `own_window`
and `window_inspection_failed`. Native checks run again in the broker before input
and between batches. `rejection.code` also identifies changed observations/focus,
monitor gaps and incomplete input. Completed input still requires visual verification.
The numeric levels are Windows mandatory integrity RIDs; elevation alone is not an
input permission decision. Higher-integrity input triggers one native Windows consent request when administrator
integrity can resolve it. Approval restores and continues the task; rejection
leaves it paused with manual recovery available. The model cannot operate UAC or
request arbitrary elevated commands, and the app does not change Windows policy.

## Quiet administrator recovery

The export with `started_at: 1789122117214` (revision
`2026-09-injected-keyboard-v7`) ends in `done` without interruption. It records
literal `chr` input, a Memory-column click and the result Google Chrome (26),
approximately 7.8 GB. The user also confirms that the task worked better.

The first attempt ends at a Medium-to-High integrity block, approximately 11.7
seconds into the task. Its handoff reports `visible: true`, `minimized: false`,
`topmost_retained: true`, `focused: false` and attachment error 5. The target remains
the reported native foreground, so the report establishes an app window raised over
the task, not successful keyboard activation of that old instance. The controller
then automatically restarts with administrator permission. The new instance's
window previously used normal visible/focused startup and another visible countdown.
This sequence explains the intermediate appearance reported by the user; the export
is not a continuous trace of every activation during process startup.

Revision `2026-09-quiet-recovery-v8` decides on automatic recovery before presenting
the app. It skips that intermediate handoff and creates the replacement window
hidden and unfocused. Its monitored countdown and subsequent control run in the
background. Failed or cancelled recovery, failed UI setup and the final task outcome
still bring the app back. A 15-second readiness timeout prevents a background startup
from remaining invisible indefinitely. The activity gate stays reserved through
terminal activation retries; callbacks whose receivers timed out are discarded.

New exports identify `background_resume_startup`. An intermediate recovery attempt
has `handoff: null` until control actually returns to the user. The historical
`before_handoff` image label still identifies the terminal capture for that attempt.
Final handoff
still records visibility, focus and temporary topmost state. No interruption policy,
administrator permission check or input lease is relaxed by this change.

## Injected keyboard regression

The next export (`started_at: 1789115766569`, revision `2026-09-input-handoff-v6`)
identifies the previously unknown trigger. The search-field click completed at
monotonic time `99468015`. The interruption has the same timestamp, event 256
(`WM_KEYDOWN`), flags 16 (`LLKHF_INJECTED`), no mouse position and
`since_agent_input_ms: 0`. Both processes already have High integrity. The handoff
succeeds and keeps the app topmost. No filter text or sorting was attempted.

This is a software-injected keyboard event, not evidence of a physical key press.
The export cannot identify the injecting process or key. Increasing the pointer
tolerance could not address this trigger. Revision `2026-09-injected-keyboard-v7`
excludes Windows-marked injected keyboard events from takeover detection regardless
of their extra-info tag or time since the agent's last input. It still passes them
to the rest of the hook chain and target app. Non-injected keyboard events retain
immediate takeover, including during clicks and typing. Countdown grace, 100-pixel
mouse tolerance, explicit Stop and foreground handoff remain in effect.

The native regression sends an untagged injected key immediately after tagged
agent input, and again after a real broker click into the disposable editor.
The old controller stops; the corrected controller accepts subsequent text input.
The fixture separately simulates non-injected input at the event-handler boundary
to verify immediate stopping and input release. It does not emulate a keyboard
driver or establish live Windows 11 Task Manager completion.

## Input interruption and foreground follow-up

The next schema 3 export (`started_at: 1789112714924`, revision
`2026-09-desktop-shortcuts-v5`) confirms a completed Task Manager launch and a
successful automatic administrator restart. Steps 5 and 6 click Processes and the
search field with equal High integrity. The terminal screenshot shows the focused
search field, still empty. The run stops about 31 ms after the search click with
the generic mouse/keyboard takeover reason; no filter text or sort was attempted.
The report does not identify the event, so physical input cannot be distinguished
from a small cursor movement or an untagged OS echo. The first handoff also shows
an activation denial (`focused: false`, attachment error 5).

Revision `2026-09-input-handoff-v6` ignores input interruption during the two-second
countdown, tolerates pointer movement within a 100-pixel radius afterward, and
keeps clicks, scrolling and keyboard input immediate. `attempts[].interruption`
now records the actual event type/flags and timing; mouse events include physical
position and the fixed movement anchor. Keyboard codes and typed content are not
retained. Event values distinguish movement (512), button presses (for example
513 for left down), wheel (522), and key down/up (256/257 or 260/261). Flags retain
Windows injection information without treating external injected input as trusted.

Every terminal handoff restores and temporarily raises the app above other windows,
with bounded activation retries. `handoff.topmost_retained` identifies temporary
topmost state, which clears when the user leaves the app or starts another run.
Visibility and keyboard focus remain separate reported facts, including when
Windows denies activation. The latest run does not establish filter/sort completion.

## Desktop shortcut validation regression

The September 11 schema 3 export (`started_at: 1789111836474`) records two physical
takeovers during startup, followed by three correctly parsed Ctrl+Shift+Escape
decisions at steps 5, 7 and 9. All three were skipped with `observation_changed`;
none reached the input broker. The native foreground and focused control stayed
`10507192` (Code.exe), no editable region was identified, and both sender and target
had Medium integrity (8192). Each observation reached its five-second deadline on
a changing desktop. Retained screenshots show a video preview among the background
windows and no Task Manager. After three refusals the controller asked the user
to stabilize the target. Filtering, sorting and result reading were never attempted.

The launch was incorrectly subject to the same full-screen content check as literal
typing when no editable region was identified. Revision
`2026-09-desktop-shortcuts-v5` distinguishes explicitly recognized desktop navigation
shortcuts from content-dependent keyboard input. Background repainting no longer
invalidates those shortcuts, while native focus, target geometry, display layout,
permissions and takeover checks remain required. The existing post-launch wait and
repeat guard still require evidence of a window transition before another launch.
The native regression fixture reproduces this refusal with a changing disposable
window and no identified editable field, without dispatching a global shortcut.

Successful preflight validation is not evidence that the complete Task Manager task
succeeded. Verify literal `chr`, descending Memory order, and the first row/value
on Windows 11 after launching the rebuilt EXE.

## Earlier Task Manager reports

The supplied version 1 Task Manager log proves that Ctrl+Shift+Escape was dispatched
and the next click was refused, but cannot identify which of the three old generic
conditions caused the refusal. The image point `(716, 13)` maps to `(1719, 32)` on a
2304 × 1536 desktop captured at 960 × 640; this mapping alone does not establish a
correct visual target. Version 2 retains the evidence needed to distinguish these
cases. Its model instructions also require verifying the active app, applied
filter, sort direction and leading row/value before reporting a result.

## Delayed Task Manager launch regression

The reported schema 2 run (`started_at: 1789044137675`) contains one completed
Ctrl+Shift+Escape action and one skipped repetition. No text/filter/sort action was
attempted. Both retained screenshots show the desktop; native foreground metadata
still identifies Explorer's taskbar (`262198`). Frame 2 was captured approximately
0.7 seconds after the first input acknowledgement, estimated from the observation
age plus validation/input durations; schema 2 has no exact input completion timestamp.
The handoff later reports a different foreground (`3606230`) and activation error 5,
but contains no identity/permissions for that window. A delayed launch is consistent
with this evidence and the user's report, while elevation cannot be inferred from
that error alone.

The controller previously equated a quiet desktop with a completed launch and
paused immediately when the model proposed the same shortcut. Schema 3 accompanies
bounded launch waiting, fresh observation before a duplicate pause, and continued
observation of dynamic views. For this task, the model must still focus the visible
search field, enter literal `chr`, verify the filter, sort Memory descending, and
read the first matching row with its displayed memory value. No process is ended.

For the next report, inspect `observation.reason`, `completion`, `samples`,
`since_input_ms`, `input_completed_ms`, `repeat_check`, `focused_element`, and
`rejection`. Compare the decision image to the `before_handoff` image and terminal
window permissions. `input_effect_observed` means a change was detected; it does not
assert that the requested app/filter/sort succeeded. A higher-integrity Task Manager
still requires manual input or an ordinary target launch. This feedback changes the
controller and reusable instructions; it does not train model weights.

## September privilege regression

The follow-up export confirms the delayed-launch fix worked: Task Manager became
the foreground window about 734 ms after the shortcut, and the controller used
a fresh observation. Taskmgr.exe had integrity 12288 (High), while klickwerk had
8192 (Medium); `input_block` was `higher_integrity`. The model's manual-filter
request could not solve the subsequent blocked sort. The controller now provides
structured recovery for both model handoffs and native input refusals.
`coverage.controller_revision` identifies `2026-09-privilege-recovery-v2`.
Cancellation is represented as `elevation_failed` with failure code
`elevation_cancelled` and Windows error 1223; launch and transfer errors have their
own codes. `elevation_restored` records the verified new sender integrity. Earlier
steps keep their original permissions and attempted actions for comparison.


## Consent and continuation regression

The schema 3 export from revision `2026-09-privilege-recovery-v2`
(`started_at: 1789047667414`) records a successful Task Manager launch and a
subsequent `ask_user`. Task Manager has High integrity (12288), while the sender
has Medium integrity (8192). Its recovery history contains only `privilege_blocked`
from `model_handoff`: there was no administrator restart request to diagnose.
The final image shows Task Manager with an empty search field and CPU sorting.
The handoff has `focused: false` and attachment error 5, so the manual recovery
card was not reliably brought in front. This run did not attempt filter entry.

Revision `2026-09-consent-and-resume-v3` removes the manual button/Continue dependency
from the first recoverable privilege block. Normal successful automatic recovery
records this sequence, retaining earlier attempts and evidence:

1. `privilege_blocked` includes the original sender and target permissions.
2. `elevation_requested`, source `controller_privilege_recovery`, means the native
   coordinator scheduled the ordinary Windows permission request.
3. `elevation_restored` means the receiving process verified High integrity and
   decoded the original session.
4. `restart_parent_exited` confirms committed transfer and exit of the old instance.
5. `automatic_resume_started`, source `ui_ready`, marks a fresh attempt after the
   restored UI is ready. Subsequent step diagnostics contain fresh permissions,
   screenshots, focus and actual input acknowledgements.

`automatic_elevation_skipped` records `user_cancelled`, `already_requested` or
`not_recoverable`. `elevation_failed` retains a structured failure: `elevation_cancelled`
with Windows error 1223, `elevation_launch_failed`, `restart_child_exited`,
`restart_child_timeout`, `restart_auth_failed`, `restart_transfer_write_failed`,
`restart_ack_failed`, `restart_commit_failed` or `restart_cancelled` distinguish
consent, process startup, transfer and explicit Stop. Setup failures have their own
listener/token/executable/socket/accept/worker codes. `automatic_resume_cancelled`
and `automatic_resume_failed` identify problems after restoration. Event timestamps
allow measuring these gaps separately from completed action timings. Tokens and
credentials are never included in events.

Core tests exercise original-task/evidence retention, consent retry suppression,
cancelled/taken-over sessions, a real loopback committed transfer, cancellation
during receipt and missing/invalid commits. Browser tests exercise UI readiness,
duplicate notifications, both languages and Stop during pending continuation.
These fixtures do not establish real Windows UAC/WebView2 or live-model Task Manager
completion; follow the target-PC acceptance steps.

## Restart transport regression

The next export (`started_at: 1789053976585`, controller revision
`2026-09-consent-and-resume-v3`) shows Task Manager at High integrity (12288) and
klickwerk at Medium integrity (8192). The automatic request now happens, but fails
214 ms later with `restart_ack_failed`. A manual retry fails at the same stage
after 174 ms. Neither `elevation_restored` nor `automatic_resume_started` appears,
and no filter entry is attempted. This is a transfer failure before continuation.

The exported session successfully decoded on both Linux and the Windows fixture's
main thread. A native socket probe under Wine reproduced a concrete transport bug:
accepting from the nonblocking listener and setting read/write timeouts still left
the accepted socket nonblocking. Reading the delayed receipt returned Windows error
10035 (`WouldBlock`) after 10 ms. Resetting that same socket to blocking mode allowed
the pending receipt to arrive at 251 ms. This matches the
[Winsock inheritance contract](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept).
The original export discarded the OS error, so it cannot independently prove that
10035 was the exact error on the user's PC.

Revision `2026-09-restart-transport-v4` explicitly resets the stream mode before
authentication and transfer. It preserves the final cancellation commit and the
UI-ready continuation gate. Socket I/O failures now retain `io_error_kind` and
`win32_error`; JSON failures retain `json_error.category`, `line` and `column`.
These optional fields keep older exports readable. Stage codes distinguish header
and payload reads/writes, authentication reads, receipt, rejection decoding and
commit. A bounded negative receipt also reports child restoration failures such
as `restart_decode_failed`, `restart_version_mismatch`, `restart_session_invalid`
and `restart_integrity_invalid` to the original instance's recovery history.
No parser message, authentication token or credential is added to diagnostics.

The standard native suite now includes restart transport checks, closing a gap
in the previous validation: its portable loopback test used a blocking listener,
and the native tests exercised desktop input but not restart sockets. An optional
offline replay can use a caller-selected feedback export through the actual
transfer functions. It verifies task, actions, attempts, evidence and memory
retention, without launching Task Manager, sending input or requesting elevation:

```sh
npm run test:native -- --with-disposable-input --replay-export build/debug.json
```

The replay models the first recovery request and preserves the source file. It
accepts paused, recoverable privilege-block exports up to 64 MiB; actual transfers
retain their 32 MiB limit. The fixture supplies the child integrity value, so real
Windows consent, process replacement, WebView2 startup and live-model completion
still require target-PC acceptance.
