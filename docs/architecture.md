# Runtime architecture

## Interface and native boundary

The React application is a static Vite build embedded by Tauri. Native commands
are confined to the local main window. The WebView has a restrictive content
security policy, no remote scripts, no iframe content, and no filesystem/shell
plugin permissions. Model responses are rendered as text, never HTML.

The frontend works against a typed bridge. The browser preview lazily loads a
fixture adapter and cannot operate the desktop. Production uses Tauri IPC to the
Rust controller. UI state is pushed as complete snapshots; a bootstrap read covers
initialization. Each task is a new generation. Start requests cannot overlap and
settings remain locked until the previous run has released control.

## Independent emergency stop

The controller starts a second instance of the same EXE in `--stop-broker` mode.
This process exits through a native code path before initializing Tauri/WebView2.
It owns the always-on-top stop bar, `RegisterHotKey(Ctrl+Alt+F8)`, physical keyboard
and mouse hooks, and **all task SendInput calls**. Its window does not activate or
steal focus. The chord is shown in the bar and its accessible window title. The
bar is repositioned above normal topmost windows every 250 ms and is excluded from
capture where supported; captured images also mask its known rectangle.

The broker fails closed if the chord cannot be registered, hooks cannot be
installed, the bar disappears, the desktop/display/session changes, the parent
process exits, the command pipe closes, a command is malformed, or liveness/input
permission expires. The countdown only arms after three seconds and after all
physical keys/buttons are released. Physical takeover hooks never swallow events.
A polling check of the stop chord provides another path in addition to WM_HOTKEY.

Anonymous inherited standard pipes carry bounded JSON commands. A reader thread
can latch STOP immediately; other commands enter a bounded queue. Only the native
message-loop thread sends input. Each command is sequenced and expires after
250 ms in transit. At most one action is active. A stopped broker cannot rearm.

The controller sends a heartbeat every 100 ms while awaiting model replies,
capture, delays, and input acknowledgements. The broker stops after 700 ms without
one. Input additionally requires a 250 ms lease renewed for the exact active
sequence. A living controller cannot inadvertently authorize an abandoned input
sequence. React sends a UI heartbeat; if it is absent for three seconds the
controller stops renewing broker permission. Browser timer throttling and
minimized-window behavior require real WebView2 acceptance on the target PC.

Input is executed in small batches, with message processing between turns.
Keyboard combinations are balanced; Unicode scalars preserve surrogate pairs.
Drag actions have bounded duration and a checked path. A random per-run Windows
shared-memory mapping records held keys/buttons before dispatch. Either surviving
process can issue matching releases after the other process fails. No other input
is authorized by cleanup. Same-user malicious native processes are outside this
IPC threat model.

This is not a kernel-level emergency stop. A frozen operating system, the secure
UAC desktop, exclusive full-screen rendering, and every hardware/driver condition
cannot be guaranteed by an ordinary desktop app. The app rejects elevated target
processes and stops on observed desktop/session changes. Hook survival, mixed DPI,
exclusive full-screen, and physical hotkey latency under load still need Windows
acceptance. Wine results do not establish those properties.

## Controller and model requests

After explicit Start and broker readiness, the main window minimizes. Each step
captures the virtual desktop, rescales it to the configured maximum edge, encodes
JPEG, and asks the selected OpenAI-compatible Chat Completions endpoint for exactly
one action. Model IDs are case-sensitive. Unknown or malformed actions, extra
fields, duplicate fields, stale frame IDs, and out-of-range coordinates are refused.
No arbitrary code, shell commands, or model-defined permission changes are actions.

Coordinates map from image pixels to physical virtual-desktop coordinates using
pixel centers, including negative monitor origins. Monitor gaps, own windows, and
unknown/elevated processes are refused. Immediately before input the controller
rechecks pointer target patches; typing requires the same foreground window.
Revalidation reduces stale-target errors but does not prove that the UI is
semantically unchanged. Models can still misunderstand tasks or screenshots.

There are step and elapsed-time bounds and a repeated-frame stop condition.
A finish proposal requires a fresh screenshot and another model decision.
That second decision is a completion check, not an independent accuracy guarantee.
A question closes the broker and releases control before the user answers.
Cancellation drops pending request futures and never resumes the old generation.

Model HTTP uses TLS certificate verification, explicit HTTP(S) destinations,
no redirects, no automatic retries, and no system proxy. Responses are limited to
2 MiB; screenshots to 8 MiB and source desktops to 32 megapixels. Server error
bodies and credentials are not echoed into the UI. Discovery checks only four
IPv4/IPv6 loopback candidates. Model listing does not claim vision capability;
Test connection uses a generated shape image and validates its returned action.

## Portable settings and speech

Startup resolves `current_exe().with_file_name("config.cfg")`. Missing files are
created with defaults. Existing settings are read, validated, and kept in memory.
Saving writes and flushes a temporary file in the same directory, then uses
MoveFileExW with replacement and write-through. Failure leaves the current config
intact and produces a visible error. No AppData config fallback is used.

DPAPI protects API keys for the current Windows account; public snapshots omit
keys. The app accepts a manually entered plaintext key in the TOML file, and
protects it on the next UI save. Old `.env` files are untouched and unused.

Dictation runs in its own COM apartment using the default installed SAPI
recognizer/audio token. It is bounded to two minutes, supports stop/finalization,
and publishes interim/final text. Missing microphone or recognizer support is
reported without blocking typed tasks. No audio or transcript is written to disk.
