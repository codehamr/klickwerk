# Windows 11 acceptance

Use a disposable desktop with an ordinary, non-elevated account. Record Windows
and WebView2 versions, model, monitor layout/scales, and results. Linux/Wine tests
do not establish physical hardware or WebView2 acceptance.

## Startup and connection

- Put the EXE in a writable folder and launch it without a development server or
  `dist` directory. Its interface must open from embedded assets.
- Confirm `config.cfg` is created beside the EXE, regardless of working directory.
- Enter a bare local address, an HTTPS URL, and a proxy API path in the same field.
  Enter the API key before loading models. Test the connection and verify settings
  survive restart. Check automatic German/English detection and manual override.
- Edit settings without a Save button. Verify text saves after a short pause,
  selections save immediately, and closing finishes pending changes. A read-only
  folder must leave a visible error with retry and discard options. Check spaces
  between numbers and units in response timeout and task limit in both languages.
- Confirm there is no Done button. Close via ×, Escape and an outside click.
  In an open menu, Escape closes only that menu. Test preference selection with
  arrow keys and Enter, selected markers, model filtering and custom model IDs,
  long model names, and keyboard focus remaining within Settings.
- Verify existing configurations with legacy `provider` values still load and
  that saving removes that unused field. API keys use DPAPI and do not appear in
  public snapshots; changing server origin clears the old key.
- Check keyboard focus, settings scrolling, light/dark themes, and 100%, 150%, and
  200% Windows scaling. Record cold/warm startup times.

## Input takeover

- Start a disposable task. Confirm the short startup notice remains visible for
  two seconds and the main window minimizes before capture. No stop bar appears.
- Move the mouse, type, click and scroll during the two-second countdown. Startup
  must continue; explicit Stop must still cancel. After countdown, test movement
  within 100 physical pixels, including a cursor echo after agent clicks. It must
  not interrupt. Move beyond that radius in one motion and in small successive
  motions; both must interrupt. Repeat after Continue and administrator recovery.
- Move the physical mouse beyond the tolerance during model inference, clicking,
  dragging and long typing. Repeat with ordinary keys, buttons, and scrolling,
  which must interrupt immediately without a movement threshold. Control must
  end without automatic resumption, and held keys/buttons must be released. The
  main window should restore and focus, with an obvious paused state and a focused
  refinement field. Check Continue without a correction and Refine & continue.
- Verify the agent's own clicks, Unicode text, and shortcuts do not interrupt it.
- Focus Task Manager search and other native text fields. Software-injected
  keyboard events must not claim physical takeover, including an injected event
  with no agent tag immediately after a click. Verify filtering can proceed.
  Press a physical key during the click and during typing: it must still stop
  immediately, without a timing grace period. Test explicit Stop from a software
  keyboard; injected keystrokes alone no longer trigger takeover.
- Test parent termination, UI hangs, minimized-window heartbeats, WebView failure,
  monitor changes, lock/unlock, and suspend/resume. No late action may arrive after
  termination. Measure physical takeover latency under normal and heavy load.

The isolated native fixture is available with:

```powershell
npm run test:native -- --with-disposable-input
```

It uses a loopback scripted provider and an unsaved fixture-owned editor. It checks
hidden monitoring, tagged versus external input, countdown grace and explicit
cancellation, premature input refusal, heartbeat/pipe loss, capture, clicks, multilingual/multiline typing,
Ctrl+A, and interruption during text. SendInput exercises real injected callbacks.
Non-injected keyboard events are simulated through a test-only message that calls
the production keyboard event handler on the broker thread. That message handler
is absent from normal release builds; physical hardware still needs manual checks.

The fixture also checks the production restart protocol with an accepted socket
from a nonblocking listener, fragmented authentication, delayed receipt, a large
session, Stop during receipt, child rejection and bounded timeouts. To reproduce
a paused privilege-block report through the transfer functions, add:

```powershell
npm run test:native -- --with-disposable-input --replay-export build/debug.json
```

This optional replay reads the export, checks preservation and leaves the source
unchanged. Its transport simulation does not request UAC or operate the target app.

## Coordinates and learning

1. Ask for a multilingual note in an empty editor. Check exact Unicode text, line
   breaks, normal shortcuts, Windows shortcuts, and completion.
2. Repeat with negative-origin monitors and mixed display scales. Compare the
   visible action history's screenshot and physical click coordinates with the
   actual targets. Test moving targets during slow model responses.
3. Take over after an incorrect action. Enter a refinement and continue. Verify
   the old actions and correction remain visible and reach the next model request.
   Ensure partial actions are not presented as verified success.
4. Trigger a question, answer it, and verify the new countdown retains context.
5. Save after success, failure, an unanswered question and takeover, both with and
   without an unsent correction. Each save must show learning and consolidate the
   available evidence into a single prompt. Cancel preparation and test failure,
   retry and the explicit unlearned fallback save.
6. Edit the name and start prompt, then save. Restart and verify no raw action
   history or separate memory is stored in the workflow. Edit/save the prompt again;
   another finalization must run. Check migration from a version 1 library.
7. Use a workflow with windows in different positions. Verify target descriptions
   adapt to the new screenshot. Refine and save again; useful old and new knowledge
   should be consolidated into the same workflow. Do not infer learning quality
   from scripted provider tests.
8. Completion and failure must restore/focus the app with distinct feedback and
   offer refinement, workflow saving/updating and New task. Test minimized,
   maximized and full-screen target applications; record any Windows focus denial.
9. Check read-only folders and damaged workflow/config files without overwriting
   existing data. Delete from the card and dialog, including a selected workflow
   and a workflow attached to a completed or paused run. The prompt and session
   must clear, and a late finalization result must not recreate the deleted item.
10. Test SAPI dictation for both tasks and refinements, including permission denial,
    cancellation, device removal, and the configured Windows speech language.

## Slow application rendering

- Test an editor and browser with delayed rendering after click, typing, Enter and
  navigation. Observations should wait for visual quiet and be bounded to five
  seconds; moving the mouse or pressing a key must interrupt the wait immediately.
- Change content or keyboard focus during a slow model response. Stale typing
  must be skipped and re-observed. Confirm ordinary caret blinking does not stall
  the controller, and test persistent animation/spinner behavior.
- Have a scripted provider repeat text or a click without any visible change.
  The app must ask the user before duplicating the last input. This is conservative
  detection, not proof that an arbitrary application has finished all background work.

## History export

- Export JSON after success, failure, takeover and a question, before and after
  saving the workflow. Verify actions, every correction and attempt, prior outcomes,
  per-attempt settings/warm starts, the current refinement and the associated workflow.
- Check the native Save As dialog in DE/EN, cancellation, Unicode paths, existing
  files and read-only folders. Cancellation or failure must preserve the session;
  failed replacement must preserve the previous file. The report is not uploaded.
- Confirm New task/deletion removes the old export action, and API credentials are
  absent from the report's settings. Recent screenshots are bounded and may
  contain visible personal content. Assistant response text is bounded; raw HTTP
  exchanges and private reasoning are not retained.

## Foreground handoff and target diagnostics

- Repeat takeover, model questions, success and failure while the main window is
  minimized, behind a normal app and behind a maximized/full-screen app. Check that
  it becomes visible in front and the refinement field accepts typing. Confirm the
  window is restored and temporarily stays above other windows, including a topmost
  target. Leave the app or start another run and confirm temporary topmost state
  clears. Inspect `handoff.topmost_retained` and actual keyboard focus; test the
  visible fallback when Windows denies activation instead of just taskbar flashing.
- Reproduce the Task Manager task: Ctrl+Shift+Escape, filter `chr`, sort Memory
  descending, identify the first matching row and its displayed value. Check the
  foreground metadata, search result, sort indicator and image/desktop mapping.
- Test a target at higher integrity than klickwerk: no blocked input may be sent.
  For a confirmed Medium-to-High conflict, the controller must request ordinary
  Windows UAC once after stopping the broker, without first restoring klickwerk.
  Unknown permissions and System-integrity targets must remain paused. An equally
  elevated sender and fixture target remain eligible; own windows remain rejected.
- Start the Task Manager task from a normally launched klickwerk. Approve UAC:
  the replacement app stays hidden, keeps the original task/steps/attempts/images,
  then starts its monitored countdown in the background when its UI is ready.
  Neither instance should raise its main window or flash the taskbar during this
  transition. The Windows consent dialog itself remains unchanged. No manual recovery
  button or Continue should be needed on this first recovery. Verify a NEW desktop
  capture, focus of the search field, literal `chr`, descending Memory order and
  the reported top row/value. Verify physical takeover after automatic resumption.
- Delay the restored UI beyond 15 seconds or make its setup fail. The app must
  become visible with the task paused, with no late automatic continuation. Then
  start or continue explicitly and confirm the old timeout cannot raise the app
  over that run. Verify a hidden window is shown on success, Stop and failure.
- Start another task immediately after terminal handoff. Pending activation retries
  or UI callbacks from the old run must not bring the window back over the new one.
- Repeat and cancel UAC: the original session and evidence must remain, with no
  automatic repeat prompt. Test Stop before launch, during transfer and while the
  restored UI is getting ready. No automatic input may follow a cancelled startup.
  A manual **Restart as administrator** retry must preserve unsent refinement and
  remain paused until Continue. Test spaces/non-ASCII executable paths, repeated
  commands, stale run IDs, denied launches and missing WebView/configuration.
- Export after cancellation, restoration/Stop and continuation. Inspect
  `recovery_events` for both integrity levels, request source, Windows error 1223,
  transfer failure stage, restoration, parent exit, `background_resume_startup`
  and `automatic_resume_started`. An intermediate automatic recovery attempt must
  have no `handoff`; a final return to the user must record the actual handoff.
  Earlier attempt outcomes and evidence must remain. A failed transfer keeps the
  original app open. Test actual UAC on Windows 11; Wine does not validate consent UI.
- Test a slow administrator restart with a large screenshot history. A delayed
  receipt must not cause an immediate `restart_ack_failed`/10035 failure. For any
  actual failure, inspect `io_error_kind`, `win32_error` and optional
  `json_error.category`/`line`/`column` in the exported recovery event. Child decode
  and version failures must reach the original instance without a commit.
- Export after a refusal and after refinement. Inspect rejection codes, both
  integrity levels, window title/class/bounds, frame references, model and input
  timing, and handoff verification. Decode a `jpeg_base64` entry as a JPEG.
- Run beyond 12 observations: all history must remain, while older image evidence
  is evicted and `frames_omitted` increases. New task/deletion must clear evidence.
  Verify raw images are absent from workflow files and UI progress snapshots.

## Delayed launch and live filtering regression

- Leave a video, live preview or updating terminal visible with Task Manager closed
  and no identifiable editable field focused. Ctrl+Shift+Escape must be dispatched
  despite unrelated repainting during model inference; `observation_changed` must
  not repeatedly block the launch. Repeat with an editable field focused and its
  contents updating. Foreground/focus/layout changes must still reject stale input,
  and app-specific shortcuts and text must retain their content checks.
- Start with Task Manager closed. Use the reported task in German and English,
  including literal `chr`, Memory descending, and the top process/value. Test cold
  starts under load and already-open windows. Confirm only one launch shortcut is
  sent while the initial window is delayed, followed by a fresh observation.
- If a scripted model repeats an unchanged shortcut, verify the action is skipped,
  the controller observes again, and then either continues from the new window or
  pauses after another unchanged repetition. Move the mouse during the recovery
  wait and confirm immediate takeover with no resumed input.
- Keep process updates enabled. Focus the search field, type `chr`, verify the
  displayed filter and remaining rows, then verify descending Memory sort before
  reporting the first row/value. Updates elsewhere must not invalidate a reliably
  identified unchanged field. Moving/changing that field must still reject input.
- Export schema 3. Check sampling timestamps, reason/deadline, unique frame IDs,
  repeat decision, focused field source/bounds, input completion time, bounded model
  response and terminal frame/window before handoff. Confirm permission failures
  remain explicit. Wine/editor fixtures do not verify Windows 11 Task Manager's
  specific WinUI accessibility provider; complete this test on the target PC.

## Explicit demonstration and portable workflow acceptance

Use an isolated Windows desktop and disposable sample text; do not demonstrate
mail sending, purchases, deletion or changes to real user files.

- Save typed and dictated prompts before their first run with no model connected.
  Restart and verify the exact prompt loads from `workflows/<short-name>.json`.
- Demonstrate in a disposable editor. Verify the floating panel stays visible,
  never sends input, and supports Pause/Finish and both global shortcuts. Switch
  apps and inspect pointer down/up, scroll, focus and grouped text in an explicit
  session export. Check negative monitor coordinates and mixed DPI layouts.
- Use a disposable password field and custom text field. Literal text must be
  omitted. Verify clipboard contents are not read. Pause before sample sensitive
  steps and verify the gap; resume and finish without starting agent execution.
- Enable screenshots using only sample data. Inspect delayed foreground crops,
  image/event associations, panel masking and size/count limits. Check behavior
  when the final click immediately precedes Finish.
- Disconnect the UI, close the app, and occupy a recording shortcut before startup.
  Recording must end/refuse cleanly; hooks and shortcuts must be released.
- Finish with a compatible model: the improved prompt must be reviewable before
  saving. Edit it, cancel it, retry analysis after a provider failure, and explicitly
  save the fallback. The workflow file must contain no raw events or screenshots.
- Import a valid file twice: separate names must be created. Reject malformed,
  unknown-version and oversized files. Export and reimport without a model.
  Verify collisions, read-only folders, and external edits do not overwrite data.
- Copy a legacy library into an isolated app folder. Verify one-time migration
  preserves the original and does not resurrect a deleted workflow on restart.
- Check both English and German UI, training from a paused/completed run, the
  latest unsent refinement, and model adaptation with windows moved afterward.
