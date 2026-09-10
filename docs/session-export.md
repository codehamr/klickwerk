# Session history export

Choose **Export history (JSON)** below the task result. It is available after
success, failure, a question or takeover, including after saving/updating the
workflow. Windows opens a native Save As dialog; the browser preview downloads a
fixture report. Cancelling leaves the session unchanged, and failed writes can be
retried. Existing files are replaced only after the complete report is written.

Export before choosing New task, deleting the workflow or closing the app. Raw
history remains temporary until explicitly exported; workflow storage still
contains only consolidated start prompts. A saved workflow without its current
session has no old action history to export.

## JSON schema version 2

| Field | Contents |
| --- | --- |
| `schema_version` | `2`; version of this report format |
| `exported_at` | Unix timestamp in milliseconds |
| `app` | Application name, version and `windows` or `preview` platform |
| `run` | Original task, session ID, current outcome, message, question/result, duration, start time, workflow association, steps and attempts |
| `run.steps` | Every recorded step in order, with actor, description, action and its arguments, status, elapsed time, image dimensions and desktop coordinates |
| `run.attempts` | Every initial start and continuation, with timestamps, step range, user reply, outcome, model/server configuration and warm-start prompt used at that time |
| `workflow` | Associated workflow and its current consolidated prompt, or `null` |
| `warm_start_prompt` | Current learned instructions retained by the session |
| `unsent_refinement` | Current text in the refinement field, or `null`; it may also have been incorporated into a saved workflow, and does not imply that any action used it |
| `run.steps[].diagnostics` | Optional capture geometry, foreground/focused control, model latency, observation age, validation/input timing, inspected targets and structured rejection |
| `run.attempts[].handoff` | Native activation method, foreground before/after, visibility, minimized state, verified focus and input-queue attachment error |
| `evidence` | Observed/omitted frame counts and recent decision screenshots as base64 JPEG, with frame IDs, physical geometry and capture timestamps |
| `coverage` | Full recorded history, bounded screenshot coverage and unretained raw model responses; the browser preview reports screenshots as unavailable |

Unlike bounded context sent to the model during a run, exported history is not
summarized or truncated. The session's existing 1,000-step capacity still applies.
Corrections appear as user steps; `attempts[].user_reply` also preserves repeated
submissions and explicit continuation without a correction. Earlier failures,
questions and successes remain in their attempt records after refinement.

`completed` means that input was sent, not that its visible outcome was verified.
Interrupted input can be partial. Suppressed duplicate actions remain `skipped`.
Wall-clock timestamps use Unix milliseconds; step elapsed times are cumulative
active-session durations, including previous attempts and excluding paused time.

Reports include user-authored task/action text and the selected server address.
Connection credentials are excluded by an explicit configuration field list.
Screenshots, window titles and entered text can themselves contain personal or
sensitive content. Recent decision screenshots stay in RAM, up to 12 frames and
8 MiB of base64 image data across the whole session, and become persistent only
when exported. Oldest frames are evicted; oversized images are omitted. The counters
make these gaps explicit. Frame IDs refer to proposed steps; a request interrupted
before returning an action also records a system step with the same frame ID.
These are observations before decisions, not a video or a guarantee of the screen
state after partial input. Screenshots are excluded from progress snapshots,
workflow files and text-only consolidation. No audio, raw HTTP exchanges or private
model reasoning are retained. The app does not upload reports; share the JSON file explicitly when
using it to diagnose or improve the application.

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
input permission decision. Higher-integrity input pauses for manual completion or
an ordinary target launch; the app does not elevate itself or change Windows policy.

The supplied version 1 Task Manager log proves that Ctrl+Shift+Escape was dispatched
and the next click was refused, but cannot identify which of the three old generic
conditions caused the refusal. The image point `(716, 13)` maps to `(1719, 32)` on a
2304 × 1536 desktop captured at 960 × 640; this mapping alone does not establish a
correct visual target. Version 2 retains the evidence needed to distinguish these
cases. Its model instructions also require verifying the active app, applied
filter, sort direction and leading row/value before reporting a result.
