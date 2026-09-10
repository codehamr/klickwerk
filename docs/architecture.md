# Runtime architecture

## Interface and native boundary

The React interface is embedded by Tauri. Native commands belong to the local main
window. Model responses render as text. The typed bridge uses native IPC in the
EXE and an isolated fixture adapter in the browser. Preview mode never operates
the desktop or contacts a model server.

A task retains its ID and history across corrections and answers. A new task or
saved workflow starts a fresh run. Start, settings, and workflow mutations cannot
overlap desktop control. Complete snapshots synchronize the interface; bootstrap
covers initialization. Completed results are compact and disappear when the user
begins another task. There is no persistent list of previous completed tasks.

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
Typing requires the same foreground window. A moved target is recorded as skipped
and triggers a fresh observation, with a question after three consecutive changes.
Own windows, monitor gaps, protected desktops, and elevated targets are refused.
These checks cannot prove that a model understood the visible UI correctly.

Keys accept case-insensitive names, including Windows shortcuts. Literal text uses
balanced Unicode events, preserving surrogate pairs. CRLF/CR normalize to one
Enter action per line break, and tab characters use Tab key events. No clipboard
replacement is necessary. Navigation/action settling is 180 ms; text/movement
settling is 80 ms before the next screen check.

The Win32 implementation follows [KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput)
and [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

## Model context and reusable workflows

Every proposed action is recorded before execution, including exact text, key
combinations, pointer coordinates, image dimensions, and timestamp. Its status
becomes completed, skipped, failed, or interrupted. Completed means dispatch was
acknowledged; the next screenshot verifies the result. A takeover records that the
latest action may be mistaken or partial, without inventing a reason. Input is not
recorded after takeover; the user supplies explicit refinements in the prompt.

Each model request includes workflow memory, prior-instance evidence when relevant,
all current user corrections, and a bounded tail of actual actions. Corrections
are retained separately so action-tail truncation cannot remove them. The model
must prefer newer corrections, inspect the current screenshot, and relocate old
targets. Coordinates from previous instances are evidence, never a replay script.
The full history remains stored in the workflow even when its action tail exceeds
the per-request context budget.

One decision is requested per observation. A finish decision evaluates the current
screenshot against the task and history; no redundant second finish request is
made. Questions end desktop control before accepting an answer. Cancellation drops
pending provider requests. Limits cover steps, elapsed time, repeated frames,
action lengths, and accumulated correction text.

Preparing a workflow clones its source run, appends any unsent refinement, and asks
the same model for strict JSON: `name`, `prompt`, and `memory`. This text-only request
is cancellable and cannot send desktop input. A server-held draft token preserves
the source history while the user edits the generated instructions. Saving writes
only that reviewed draft. Editing an existing workflow preserves its history.
Updating a workflow after another run retains earlier corrections and adds the new
run. Saved workflow prompts and instructions are always editable. This is memory
and instruction reuse, not model weight training.

`workflows.json` is written beside the EXE with flush and atomic replacement. The
library supports 100 workflows, 1,000 history entries per workflow, and 16 MiB total.
A damaged library is preserved and surfaced as an error. No workflow is persisted
until the user saves. Saved text and coordinates are readable; screenshots, raw
user keystrokes, and audio are not recorded.

## Connections, settings, and speech

All compatible servers use the same URL field and transport. Legacy `provider`
config values are accepted and omitted on the next save. Bare local/IP addresses
use HTTP and bare domain names use HTTPS; explicit schemes are preserved. Standard
API roots are completed, and proxy paths survive normalization. A single returned
model is selected automatically; multiple model IDs remain a user choice. Listing
does not prove vision support. Test connection validates a generated shape image.

HTTP verifies TLS certificates, refuses redirects, avoids automatic retries and
system proxies, and caps responses at 2 MiB. Screenshots are limited to 8 MiB and
source desktops to 32 megapixels. Server error bodies and credentials are not
echoed in the UI. Settings resolve relative to the EXE, use atomic replacement,
and protect API keys with Windows DPAPI. Changing origin clears the previous key.

SAPI dictation runs in its own COM apartment with the default recognizer and audio
input. It can fill a task or correction, is bounded to two minutes, and publishes
interim/final text. Missing audio support leaves typed prompts available.
