# klickwerk

A small Windows desktop assistant with a React interface and a native Rust core.
Describe a task, press **Let's do it**, and let klickwerk handle one desktop action
at a time. **Ctrl + Alt + F8** stops control from any application.

The app is rebuilt around **Tauri 2, React, TypeScript, Vite, Tailwind CSS, and
shadcn-style components using Radix primitives**. The Windows executable embeds
the interface and uses the system WebView2 Runtime. Node.js and Rust are development
tools; users do not install them.

## Run on Windows 11

1. Put `build/klickwerk.exe` in a writable folder and open it.
2. The app creates `config.cfg` **beside that EXE** on first launch and reads it on
   subsequent launches. Settings saves changes back to that exact file.
3. Open **Settings**, select **On this PC** or **Custom server**, enter the server
   address and a vision model ID. **Find automatically** checks local Ollama and
   compatible servers. **Load models** lists IDs; manual IDs also work.
4. **Test connection** sends a generated shape image. It does not capture or
   operate the desktop. Save the settings when ready.
5. Enter a task, then click **Let's do it** or press **Ctrl + Enter**. Release your
   mouse and keyboard during the three-second countdown. The main window minimizes;
   an independent native stop bar stays above normal desktop windows.
6. Press **Ctrl + Alt + F8**, click **STOP**, or use the physical mouse/keyboard to
   take over. The task remains available. A question pauses control; **Continue**
   starts a fresh countdown after your reply.

The Windows WebView2 Runtime is required and normally ships with Windows 11. The
EXE is not an installer and does not silently download a runtime. Move the app out
of `Program Files` or another read-only folder if settings cannot be saved.

The microphone uses the Windows default SAPI recognizer and default audio input.
An installed speech language and microphone permission are required. Typed prompts
remain available. Prompts and dictation can be multilingual; the interface is English.

## Settings next to the EXE

`config.cfg` uses TOML syntax; see [config.example.cfg](config.example.cfg). Relative
working directories and old `.env` files do not change its location. Builds neither
read nor copy private `.env` files, and never delete `config.cfg`.

API keys entered in Settings are saved in that file with Windows DPAPI protection.
They can be unlocked by the same Windows user on that PC. Re-enter the key after
moving the app to another account/PC. All other settings remain readable. Changing
the server origin clears the previous key. Unsaved dialog edits are discarded on
close. A malformed config is preserved; explicitly saving replacement settings
first creates a `config.invalid-<pid>.cfg` backup.

The app sends desktop screenshots and task text to the **selected model server**
while a task runs. It stores no screenshot, audio, transcript, or task history.

## Develop

Rebuild the devcontainer after this migration. It contains Node 24, Rust 1.98.1,
LLVM, and cargo-xwin. It does not need a Linux desktop, GTK/WebKit development
packages, Wine, noVNC, MinGW, or a Python application runtime. Existing coding-agent
credential volumes are retained. Dependencies and compiler caches use Linux volumes
to avoid Windows bind-mount file-lock and performance problems.

```sh
npm ci
npm run dev
```

Open **http://localhost:1420**. The browser preview is explicitly labeled and uses a
separate fixture adapter: no desktop input, screen capture, real credentials, or
model requests. Preview settings stay in browser storage. React changes update
without compiling Rust. Polling is enabled in the devcontainer for Windows mounts.

```sh
npm run check
npm run test:core
npx playwright install --with-deps chromium  # One-time browser test setup
npm test
```

Build the Windows x64 executable from the container:

```sh
XWIN_ACCEPT_LICENSE=1 npm run desktop:build
# Or: make
```

The first cross-build downloads the Microsoft CRT/SDK through cargo-xwin; the
flag accepts the SDK license. Later builds reuse the SDK and compiler caches.
The successful build is copied to `build/klickwerk.exe`. A failed compile preserves
the previous EXE and config. Close the Windows app before replacing its EXE.
The script enables `tauri/custom-protocol` to embed and serve the production UI.
Bare Cargo release builds without that feature are rejected, since they would
otherwise try to load the development server at `localhost:1420`.

On Windows, install the Rust MSVC prerequisites and run `npm run desktop:dev` for
native development or `npm run desktop:build` for a portable release. The checked-in
GitHub Actions workflow performs frontend checks and builds on Windows; it has not
been run by this local session.

## Project map

| Location | Purpose |
| --- | --- |
| `src/` | React UI, styling, typed bridge, isolated browser preview |
| `src/components/ui/` | Editable UI primitives |
| `src-tauri/src/config.rs` | Executable-relative configuration and atomic persistence |
| `src-tauri/src/provider.rs` | Bounded, cancellable compatible model transport |
| `src-tauri/src/action.rs` | Strict model action schema |
| `src-tauri/src/guard.rs` | Portable stop, lease, replay, and coordinate rules |
| `src-tauri/src/desktop.rs` | Tauri commands and task lifecycle |
| `src-tauri/src/platform/` | Native stop broker, input, capture, and SAPI |
| `tests/` | Browser behavior, accessibility, and visual checks |
| `scripts/` | Build and developer utilities |
| `build/` | EXE and visual artifacts; settings may also live here |

See [architecture](docs/architecture.md), [validation status](docs/status.md),
[Windows acceptance](docs/windows-acceptance.md), and [UI development](docs/ui-development.md).
