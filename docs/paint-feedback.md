# Paint feedback: zoom, drawing order and completion

The September 12, 2026 export (`started_at: 1789197739202`, controller revision
`2026-09-quiet-recovery-v8`) records 23 model decisions in approximately 89 seconds.
The task was to create `aaa.bmp` on the Desktop with a rocket drawing. The run
ended in `done` without an input rejection or user takeover.

## What the evidence establishes

| Steps | Evidence and assessment |
| --- | --- |
| 4–5 | The model describes the canvas as small and clicks two nearby zoom controls. This explains its stated intention, but the export evicted both decision screenshots and the screenshot after the second click. The exact initial zoom and benefit of those clicks cannot be reconstructed. |
| 7–15 | Nine straight drags include several similar lines from the nose and overlapping left body lines. Descriptions promise multiple parts although each drag produces only one straight segment. |
| 16–18 | The model explicitly calls the drawing incomplete, opens Save As, enters `aaa.bmp`, and confirms saving. No separate file-type selection action is recorded; the filename alone cannot establish the saved encoding. |
| 19–21 | It adds the right fin and a single vertical flame stroke, then sends Ctrl+S. Saving again after editing was appropriate; the earlier save interrupted a short, unfinished drawing sequence. |
| 22–23 | It hides Paint with Win+D and declares completion from the Desktop. The requested destination did not require hiding the completed drawing, and an icon cannot verify its contents. |

The retained Paint image before Win+D shows a recognizable but rough red outline:
several nose/left-side strokes, a gap between the nose and body, open fin strokes,
and a single line for the flame. The evidence does not support describing the
process as optimal. It also does not independently verify the file's bytes or
whether every visible edit reached disk.

The 3840 × 2160 desktop was reduced to 960 × 540 for the model. Small controls were
hard to read, but this alone does not establish that changing Paint's zoom was
necessary or that either zoom click was correctly located. Only the most recent
12 screenshots survived. The old 24 KB model-history budget held steps 10–22
before the final decision, because each action repeated substantial native
diagnostics. This reduced access to early decisions; it is not proof of the
model's internal reason for switching to saving.
With compact records, all 23 original actions occupy 13,240 bytes inside that same
24,000-byte action budget.

## Controller changes

- Content-creation guidance orders workspace preparation, complete content,
  inspection/repair, final save, and verification. Useful checkpoints remain
  allowed, with a new save required after subsequent edits.
- Zoom must address a visible problem. The agent must identify the actual control,
  make a deliberate adjustment, check its result, and locate canvas coordinates
  again. Drawing guidance calls for suitable tools, coherent parts, joined
  outlines, and descriptions that match the single segment actually executed.
- Saving guidance checks location, name, selected format, dialogs and saving after
  the last edit. It leaves the result visible unless another final view is requested.
- The controller defers the first finish proposal. A separate request to the
  configured model reviews a newer screenshot and can return a concrete check or
  repair. Intervening actions invalidate the proposal. Reviews consume the normal
  step budget and do not add a user confirmation step.
- Compact history retains action arguments, statuses, model descriptions, app
  identity and relevant failures. Detailed geometry and timing stay in the export.
- Exports retain up to 48 observations and assistant responses, with the existing
  8 MiB base64 screenshot budget and 32 KiB per-response limit. Larger runs or
  screenshots can still evict early evidence; omission counts remain explicit.

## Verification and limits

The sanitized [action fixture](../src-tauri/fixtures/paint-feedback.json) preserves
the reported sequence with English descriptions and without desktop screenshots,
window titles, private paths or credentials. Regression tests check that all 23
actions, including setup and both saves, fit in compact history despite detailed
native metadata. Completion tests reject an initial proposal and reused/older
frames. A scripted HTTP provider requests a final save during review and confirms
that another proposal and fresh review are required afterward.

Local validation passed: 70 portable Rust tests, TypeScript/ESLint, Windows-target
Clippy with warnings denied, native monitoring and disposable-input fixtures under
Wine/Xvfb, and five Chromium tests covering English/German exports and language
preferences. The native fixtures use an isolated editor and scripted server.

These tests verify controller behavior and request construction, not live-model
drawing quality. A Windows Paint rerun must still establish that zoom choices,
geometry, final saving and format verification actually improve. Use the same
task/model/settings to compare, inspect the new `completion_review` frames, and
open the saved BMP to verify its content and format. Do not infer improvement
from the final summary or from fewer actions alone.
