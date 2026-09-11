# klickwerk

[Download for Windows](https://github.com/codehamr/klickwerk/releases/latest/download/klickwerk.exe) ·
[Releases](https://github.com/codehamr/klickwerk/releases) ·
[Build status](https://github.com/codehamr/klickwerk/actions/workflows/check.yml)

A small Windows desktop assistant with a React interface and a native Rust core.
Describe a task, press **Let's do it**, and let klickwerk handle one desktop action
at a time. Move your mouse or press any key to take over. Refine a task as you go,
then save the improved workflow for next time.

The app is rebuilt around **Tauri 2, React, TypeScript, Vite, Tailwind CSS, and
shadcn-style components using Radix primitives**. The Windows executable embeds
the interface and uses the system WebView2 Runtime. Node.js and Rust are development
tools; users do not install them.

## Run on Windows 11

1. Download `klickwerk.exe` from the latest release, put it in a writable folder
   and open it. At every start it checks the public GitHub release and updates
   itself with visible progress. **Skip this time** opens the existing version.
   Settings, workflows, the filename and the folder stay in place.
2. The app creates `config.cfg` **beside that EXE** on first launch and reads it on
   subsequent launches. Settings saves changes back to that exact file.
3. Open **Settings**, enter a **Server URL**, add an optional **API key**, then select a vision model. The same
   connection works for every compatible server. Addresses such as
   `localhost:11434` work directly; standard API paths are completed automatically.
   Focusing the model field loads models with the current key. Manual model IDs also work.
4. **Test connection** checks image understanding using a generated shape image.
   Settings save automatically. Close with **×**, **Escape**, or a click outside;
   pending changes are saved before returning to your task.
5. Enter a task and click **Let’s do it** or press **Ctrl + Enter**. A short notice
   explains that control starts in two seconds. The window then minimizes.
6. Move your mouse or press any key to interrupt from any application. Input stops,
   the window is restored, raised and activated, and a clear paused message returns
   control to you. Choose **Continue** as is, or enter a correction and choose
   **Refine & continue**. Success, questions, and failures also return control;
   successful tasks remain available for refinement until you choose **New task**.
7. **Save as workflow** is available before the first run, without a connected model.
   A typed or dictated prompt can be named and saved immediately. For an existing
   run, **Learn & save** combines the goal, corrections, actions and outcome.
   Failed learning offers retry or an explicit **Save current prompt** fallback.
8. Optionally choose **Show me how** beside the prompt, before or after a run.
   Start recording, switch to the target app and demonstrate the task. A floating
   recording panel offers Pause and Finish. **Ctrl + Shift + F8** toggles pause;
   **Ctrl + Shift + F9** finishes. The model then proposes an improved prompt;
   review or edit it and choose **Save workflow**. Nothing runs when saving.
9. Open a saved workflow from **Your workflows**. **Use workflow** loads its prompt
   into a fresh session. **Export workflow** shares a small JSON file; **Import
   workflow** previews a file before saving it as a separate entry. Files live in
   `workflows/` beside the EXE, with short names such as `monthly-report.json`.
   Collisions receive a numeric suffix. Delete from the card or dialog; a visible
   confirmation also clears the current prompt and temporary session.
10. The UI and suggestions automatically use German on a German Windows display
   language, otherwise English. **Settings → Preferences → Language** lets you
   select **Automatic**, **Deutsch** or **English**. Your own text is never translated
   when you change the UI language.

The Windows WebView2 Runtime is required and normally ships with Windows 11. The
EXE is not an installer and does not silently download a runtime. Move the app out
of `Program Files` or another read-only folder if settings cannot be saved.

Offline or failed updates leave the app usable. Checks during an existing task
report availability for the next start. See [public releases and portable
updates](docs/updates.md) for the release format, verification and recovery behavior.
Every successful CI run on `main` publishes the tested EXE with a unique UTC date
and time tag; download links always point to the latest completed release.

When a target such as Task Manager runs as administrator, Windows can block input
from a normally launched klickwerk. For a confirmed, recoverable permission block,
klickwerk opens the normal Windows UAC prompt once. Approve it to restart klickwerk
and continue the original task automatically from a fresh desktop observation.
The task, history and recent diagnostic evidence are kept. Declining UAC leaves
the original session paused, with manual retry and export available; it does not
repeat the permission prompt. **Stop** cancels pending continuation.

For a later manual retry, choose **Restart as administrator**, approve Windows,
then choose **Continue** in the restored session. Unsent refinement text survives
that explicit restart too. Settings and security policies are not changed. Use
the same Windows account so DPAPI-protected connection credentials remain readable.

The microphone uses the Windows default SAPI recognizer and default audio input.
An installed speech language and microphone permission are required. Typed prompts
remain available. Prompts and dictation can be multilingual; the interface supports English and German.

## Settings next to the EXE

`config.cfg` uses TOML syntax; see [config.example.cfg](config.example.cfg). Relative
working directories and old `.env` files do not change its location. Builds neither
read nor copy private `.env` files, and never delete `config.cfg`.

API keys entered in Settings are saved in that file with Windows DPAPI protection.
They can be unlocked by the same Windows user on that PC. Re-enter the key after
moving the app to another account/PC. All other settings remain readable. Changing
the server origin clears the previous key. Text edits save after a short pause;
selections save immediately. Closing Settings finishes any pending save. The status
shows when all changes are saved; failures keep your edits available for retry or
explicit discard. Replacing a malformed config first creates a
`config.invalid-<pid>.cfg` backup.

Preference menus mark the current selection and support keyboard navigation.
The model menu filters as you type and accepts exact IDs from compatible servers.
Escape closes an open menu first, then Settings.

The app sends desktop screenshots, task text, action history, and corrections to
the **selected model server** while a task runs. Preparing a workflow sends its
text history to that same server. Explicit demonstrations additionally send their
recorded events and any opted-in screenshots after you finish. Saving an unrun
prompt and importing/exporting workflows do not need a model. Learning means
reusing instructions; it does not retrain model weights. See [training and portable
workflows](docs/training.md) for capture limits and privacy behavior.

After a task finishes, fails, pauses or asks a question, **Export history (JSON)**
lets you save its recorded actions, corrections, attempts and outcomes for analysis.
The report also includes per-attempt model settings and warm-start instructions,
the current workflow, and the refinement field's current text. Export before
starting a fresh session; the app does not retain old session histories. See the
[report format and coverage](docs/session-export.md).

Action history stays in memory for the current session, including after a save so
that you can continue refining. Each `workflows/<short-name>.json` contains only a
format marker, version, name and reusable prompt. IDs come from filenames. Legacy
`workflows.json` versions 1 and 2 migrate once into this folder; the original is
preserved. The old file is no longer read once the folder exists.
Explicit JSON exports are separate files and include recent observation and pre-handoff screenshots
(up to 12 frames, bounded to 8 MiB of image data), window details, timing, recovery
decisions and bounded assistant responses.
Explicit demonstration evidence is also included in history exports when present.
Screenshots otherwise remain temporary in RAM; audio is not saved.
**New task** and workflow deletion discard
the temporary session. There is no persistent list of previous tasks.

Before each model observation, the controller normally waits at least 700 ms and
looks for 450 ms of visual quiet. After input it allows two seconds for an initial
effect; recognized window shortcuts, including Ctrl+Shift+Escape, allow eight seconds
for a foreground transition. Continuously updating views yield a fresh observation
at the deadline instead of automatically pausing. Text is paced and revalidated
against the same focused field, using Win32/UI Automation field identity and bounds
when available, otherwise the full screen. A repeated click, key or text action with
no observed effect is suppressed for one further observation of up to five seconds.
Only a repeated failure after that recovery asks the user to intervene. All waiting
remains interruptible, and visible completion still needs model verification.

## Develop

Rebuild the devcontainer after this migration. It contains Node 24, Rust 1.98.1,
LLVM, and cargo-xwin. It does not need a Linux desktop, GTK/WebKit development
packages, Wine, noVNC, MinGW, or a Python application runtime. Dependencies and
compiler caches use Linux volumes to avoid Windows bind-mount file-lock and
performance problems.

Claude Code, Codex, and Codehamr are installed natively in the image. Private volumes
scoped by `${devcontainerId}` persist `/root/.claude`, `/root/.codex`, `/root/.agents`,
and `/root/.config/anthropic` across rebuilds. Each new environment needs a separate
sign-in; logins are never copied between containers. The old `klickwerk-claude` and
`klickwerk-codex` volumes remain untouched but are no longer mounted.
Codehamr keeps configuration and prompt history in the workspace's ignored
`.codehamr/` directory; keep credentials out of Git and the image.

Codex configuration is managed personally in `/root/.codex/config.toml` in the
private volume. The image does not supply a configuration template. Agent
executables and their symlink targets live outside the data volumes, with Codex's
package under `/opt/codex`.

```sh
npm ci
npm run dev
```

Open **http://localhost:1420**. The browser preview is explicitly labeled and uses a
separate fixture adapter: no desktop input, screen capture, real credentials, or
model requests. Preview settings and explicitly saved preview workflows stay in
browser storage. React changes update without compiling Rust. Polling is enabled in the devcontainer for Windows mounts.

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
| `src-tauri/src/platform/` | Native input monitor, input, capture, and SAPI |
| `src-tauri/src/workflow.rs` | Action history, correction context, and workflow persistence |
| `tests/` | Browser behavior, accessibility, and visual checks |
| `scripts/` | Build and developer utilities |
| `build/` | EXE and visual artifacts; settings may also live here |

See [architecture](docs/architecture.md), [validation status](docs/status.md),
[Windows acceptance](docs/windows-acceptance.md), and [UI development](docs/ui-development.md).
