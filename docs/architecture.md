# Runtime architecture

## Interface and native boundary

The React interface is embedded by Tauri. Native commands belong to the local main
window. Model responses render as text. The typed bridge uses native IPC in the
EXE and an isolated fixture adapter in the browser. Preview mode never operates
the desktop or contacts a model server.

A task retains its ID and history across corrections and answers. A new task or
saved workflow starts a fresh run. Start, settings, and workflow mutations cannot
overlap desktop control. Complete snapshots synchronize the interface; bootstrap
covers initialization. Every terminal or waiting state shows a clear handoff and
keeps refinement available, including after success. New task and workflow deletion
reset both backend session state and the composer. There is no persistent task list.

## Physical takeover

A second instance of the EXE in `--stop-broker` mode owns all task `SendInput`
calls, low-level keyboard/mouse hooks, and a hidden native window for display,
power, and session notifications. No stop bar or global shortcut is created.
The main interface shows a two-second startup notice before minimizing. Hooks
already detect takeover during this countdown; release events from the launch
shortcut are allowed until input is armed. All keys/buttons must be released
before the first agent action.

Each agent input carries `INPUT_TAG`. Other keyboard events, pointer movement,
buttons, and scrolling latch termination. Duplicate mouse-position notifications
are ignored: the OS can echo an injected position without its tag even though the
pointer has not moved. External injected input also interrupts, allowing software
keyboards and other accessibility input to take over. User events are passed on;
queued agent presses and movements are suppressed after the stop latch. Balanced
release events are still allowed. Hooks are pumped before each small input batch,
including individual Unicode scalars during long typing.

The input monitor refuses input if hooks cannot be installed, the desktop/session
changes, the parent exits, the pipe closes, or a permission expires. A stopped
process never rearms. The controller renews its heartbeat every 100 ms, including
while awaiting model replies. The monitor expires after 700 ms without a heartbeat.
Input additionally requires a 250 ms lease for the exact action sequence. A missing
UI heartbeat stops renewal after three seconds.

A random per-run shared-memory mapping tracks held input before dispatch. Either
surviving process releases held keys/buttons if the other fails. Hook callbacks do
minimal work; no model request, file write, or UI render blocks physical takeover.
An ordinary application cannot guarantee hardware input timing when Windows itself
is frozen. Wine fixtures verify implementation behavior, not Windows 11 hardware
latency or WebView2 minimized-window behavior.

## Input and coordinates

Both application and monitor request per-monitor DPI awareness before creating
windows. Capture workers and input use physical virtual-desktop pixels. Image
coordinates map through screenshot pixel centers, then to the centers of physical
pixels in the 16-bit absolute mouse grid, including negative monitor origins.
The action history includes both image size and mapped desktop points.

Pointer targets are checked against live patches immediately before dispatch.
Typing requires the same foreground window, focused control, and visually unchanged
content after model inference. Its focus is rechecked before each Unicode scalar.
A moved target is recorded as skipped and triggers a fresh observation, with a
question after three consecutive changes. Own windows, monitor gaps, protected
desktops, and elevated targets are refused. These checks cannot prove that a model
understood the visible UI correctly.

Keys accept case-insensitive names, including Windows shortcuts. Literal text uses
balanced Unicode events, preserving surrogate pairs, with eight milliseconds
between scalar batches so editors can drain their input queues. CRLF/CR normalize
to one Enter action per line break; tab characters use Tab key events. No clipboard
replacement is necessary.

Before each screenshot is sent to the model, capture samples the screen until it
has been quiet for 450 ms, after a minimum of 700 ms. Changes reset the quiet period;
a five-second deadline returns control if the desktop keeps moving. Pixel changes
are evaluated in local tiles so a small text field cannot disappear inside a global
change percentage; tiny caret changes are tolerated. Capture and waits run within
the broker's cancellable heartbeat loop. Repeating the last click/key/text on an
unchanged scene returns a question instead of duplicating input.

The Win32 implementation follows [KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput)
and [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

## Model context and reusable workflows

Every proposed action is recorded before execution, including exact text, key
combinations, pointer coordinates, image dimensions, and timestamp. Its status
becomes completed, skipped, failed, or interrupted. Completed means dispatch was
acknowledged; the next screenshot verifies the result. A takeover records that the
latest action may be mistaken or partial, without inventing a reason. Input is not
recorded after takeover; the user supplies explicit refinements in the prompt.

Each model request includes the saved start prompt, all current user corrections,
and a bounded tail of actual actions. Corrections are retained separately so
truncation cannot remove them. The model must prefer newer corrections, inspect the
current screenshot, and relocate targets. Previous raw histories are never replayed.

One decision is requested per observation. Questions and completion end desktop
control before accepting further input. After broker teardown releases held input,
the main window is restored, shown and focused. If Windows refuses foreground
activation, a taskbar attention request provides a fallback. See
[Tauri window focus](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.set_focus)
and [Windows foreground restrictions](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow).

Every workflow save first sends a text-only finalization request to the selected
model, including when editing a saved prompt or when no correction or success is
available. It includes the outcome, a whole-session overview, detailed recent
input, explicit corrections, prior instructions and user edits. Unchanged original
task text must not override newer corrections. The model returns strict JSON with
`name` and one self-contained `prompt`, covering useful preconditions, approaches,
preferences, mistakes to avoid and success checks. No new desktop input is sent.

A server-held draft token binds the result to its source run, step count and saved
workflow revision. Reset, deletion, resumption or another update prevents a stale
result from being saved. The user sees learning, persistence and confirmation as
separate states. A failed model request offers retry or an explicit fallback save
of the task, prior prompt and corrections, with no claim of successful learning.
Closing during finalization cancels it and never persists a late result.

`workflows.json` version 2 stores only ID, name, prompt and update time. Version 1
memory is merged into its prompt on load and legacy histories are discarded on the
next atomic library write. Current-session history remains in RAM until a fresh
session replaces it, allowing further refinement after saving. The library supports
100 workflows and 16 MiB total; prompts are bounded to 32 KiB. A damaged library is
preserved and surfaced as an error. Screenshots, raw user keystrokes and audio are
not recorded. Learning reuses instructions; it does not update model weights.

## Connections, settings, and speech

All compatible servers use the same URL field and transport. Legacy `provider`
config values are accepted and omitted on the next save. Bare local/IP addresses
use HTTP and bare domain names use HTTPS; explicit schemes are preserved. Standard
API roots are completed, and proxy paths survive normalization. A single returned
model is selected automatically; multiple model IDs remain a user choice. The
connection order is URL, optional key, model. Loading starts on model-field focus
or an explicit request, after credentials can be entered. Listing
does not prove vision support. Test connection validates a generated shape image.

HTTP verifies TLS certificates, refuses redirects, avoids automatic retries and
system proxies, and caps responses at 2 MiB. Screenshots are limited to 8 MiB and
source desktops to 32 megapixels. Server error bodies and credentials are not
echoed in the UI. Settings resolve relative to the EXE, use atomic replacement,
and protect API keys with Windows DPAPI. Changing origin clears the previous key.

Settings autosave text edits after a 500 ms pause and selections immediately.
Writes are serialized and drain the latest draft, so slow responses cannot overwrite
newer edits. Closing flushes pending changes and waits for completion. A failed
save keeps the dialog and draft available for retry or explicit discard of only
unsaved changes. Save responses update application preferences without replacing
the text being edited. Response timeout and step limit labels translate the number
and unit together, including their separating space.

SAPI dictation runs in its own COM apartment with the default recognizer and audio
input. It can fill a task or correction, is bounded to two minutes, and publishes
interim/final text. Missing audio support leaves typed prompts available.

UI language defaults to the Windows user's display language: German when its
primary language ID is German, otherwise English. A persisted `language` preference
supports `system`, `de` and `en`. The browser preview uses its browser locale.
Translation keys, code and diagnostics remain English; interface labels and all
suggested tasks have explicit English/German versions. Model-generated descriptions,
questions and summaries use the selected UI language. Changing language preserves
user-entered prompts. Legacy `reduce_motion` is accepted on read but no longer used
or written; the preference and motion overrides have been removed.
