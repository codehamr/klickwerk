# Demonstrations and portable workflows

A task can be saved at any point: before execution, after success or failure,
after takeover, with an unsent refinement, or with an optional demonstration.
Typing and dictation feed the same task field. Saving does not execute the task.

Choose **Show me how**, review the recording options and start. Work in the target
application. Use the floating recording panel, **Ctrl + Shift + F8** to pause, or
**Ctrl + Shift + F9** to finish. Finishing stops observation before requesting model
analysis. The improved prompt remains editable; **Save workflow** persists only
that prompt and its name. Cancel leaves the demonstration in the current session
for another attempt. Re-recording replaces the previous demonstration, retaining
agent history and explicit corrections. Starting a new task discards the evidence.
A model failure preserves the session and offers retry or an explicit prompt-only
fallback; it never claims to have learned from missing analysis.

## Capture and interpretation

The Windows recorder uses a dedicated low-level hook thread. Callbacks enqueue
bounded events and return immediately; the worker performs grouping, metadata
inspection and optional screenshot encoding. Neither path calls SendInput or the
agent's execution broker. Hooks ignore injected input and klickwerk's own windows.
Global shortcuts are registered only during recording and are released afterward.
The recording activity reserves the same gate as normal execution. Closing the
app, losing the UI heartbeat, switching to a protected desktop or reaching a limit
ends recording. Hook/panel setup failure ends the activity without enabling control.

Events include physical pointer down/up coordinates, wheel axis and deltas,
shortcuts, translated text, foreground application/title/bounds and native focused
control identity. Down/up pairs preserve the evidence needed to infer clicks,
double clicks and drags. There is no high-volume pointer-movement stream.
Consecutive text in the same target is grouped, but backspace/selection/paste remain
separate events. Input is evidence, not proof of a final field value or success.
Foreground polling and delayed metadata can miss very brief focus transitions;
the model must infer cautiously and verify targets against the current desktop.

Literal text is optional and limited to recognized visible non-password Win32
EDIT/RichEdit fields, checked at the event and again before translation. Unknown
or custom fields produce omission markers, never literal text in the report.
Clipboard contents are never read. IME, speech entered into another app, dead-key
composition and custom browser/WinUI fields are not reconstructed reliably; include
necessary wording in the task or refinement. ToUnicodeEx uses flag 4 so it does not
modify the user's keyboard composition state. Pause before entering sensitive
information even in an ordinary text field. Window titles can also contain data.

Screenshots are off by default. Opting in permits small crops of the active
window, delayed at least 500 ms after activity and spaced at least 1.5 seconds
apart. The recorder panel is masked. Known native password fields are skipped;
other visible sensitive data can still appear. These images are contextual evidence,
not guaranteed before/after pairs or exact click-time captures. They go to the
configured model only after the recording ends, together with the event log.

Limits are 15 minutes, 600 grouped events and approximately 96 KiB of event JSON.
Reaching the event budget stops recording instead of silently discarding its
beginning. The queue has 256 slots; overload stops with explicit omission counts.
Up to 8 screenshots fit within 4 MiB of base64 image data, each scaled to at most
960 pixels on its longest edge. The UI shows counts; raw evidence remains in RAM.
**Export history (JSON)** explicitly includes it under `evidence.training`.

The report has version 1, recording options, events (`id`, `elapsed_ms`, `window`,
`type` and type-specific fields), screenshots (`after_event_id`, `elapsed_ms`,
physical crop bounds, image dimensions and JPEG base64), omission counts and the
stop reason. Event times are relative monotonic milliseconds. Pause/resume events
mark gaps. The finalization prompt treats all demonstration content as untrusted
evidence relative to the original task and explicit corrections. It requests
semantic instructions, preconditions and success checks rather than replaying
coordinates or saving raw logs as instructions. No model weights are changed.

## Files and sharing

Each `workflows/<short-name>.json` beside the EXE uses this format:

```json
{
  "format": "klickwerk-workflow",
  "version": 1,
  "name": "Monthly report",
  "prompt": "Prepare the monthly report. Verify the month and destination before saving."
}
```

The app suggests short names and displays the filename. Lowercase words use
hyphens; German umlauts become ae/oe/ue and ß becomes ss. Reserved device names are
avoided, and collisions receive `-2`, `-3`, etc. Updating a workflow preserves its
existing filename. Files contain no API key, recordings, screenshots or history.

**Import workflow** validates a file up to 64 KiB and opens it for review. It is
saved as a separate workflow; an imported title cannot overwrite another entry or
specify a filesystem path. **Export workflow** exports the saved prompt without
model calls or desktop actions. An edited dialog must be saved before export.
Files can also be copied into the folder while the app is closed, then loaded on
startup. Up to 100 workflows are supported. Keep the EXE in a writable folder.

Legacy `workflows.json` versions 1 and 2 migrate once through a staging directory;
legacy memory is merged into the prompt. The original library is preserved and is
no longer loaded after the new folder exists. Malformed files and externally
changed workflows are preserved, with errors surfaced instead of overwrites.

## Validation boundaries

Core tests use temporary directories and local HTTP stubs. Browser tests simulate
all training activity and never contact a real model. The native fixture inspects
disposable controls and verifies heartbeat shutdown, hotkey release and panel
teardown without sending desktop input. Run it only on an isolated Windows/Wine
desktop. These tests do not establish SAPI behavior, recording quality across
third-party apps, accessibility providers, or the quality of a particular model's
consolidation. Follow the Windows acceptance checks with harmless sample data.

Implementation references: Microsoft's [low-level keyboard callback](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc),
[ToUnicodeEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-tounicodeex),
and [RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey).
