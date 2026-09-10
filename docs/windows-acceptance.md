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
- Move the physical mouse during countdown, model inference, clicking, dragging,
  and long typing. Repeat with ordinary keys, buttons, and scrolling. Control must
  end without automatic resumption, and held keys/buttons must be released. The
  main window should restore and focus, with an obvious paused state and a focused
  refinement field. Check Continue without a correction and Refine & continue.
- Verify the agent's own clicks, Unicode text, and shortcuts do not interrupt it.
- Test parent termination, UI hangs, minimized-window heartbeats, WebView failure,
  monitor changes, lock/unlock, and suspend/resume. No late action may arrive after
  termination. Measure physical takeover latency under normal and heavy load.

The isolated native fixture is available with:

```powershell
npm run test:native -- --with-disposable-input
```

It uses a loopback scripted provider and an unsaved fixture-owned editor. It checks
hidden monitoring, tagged versus external input, countdown cancellation, premature
input refusal, heartbeat/pipe loss, capture, clicks, multilingual/multiline typing,
Ctrl+A, and interruption during text. External input is simulated for this fixture;
physical hardware still needs the manual checks above.

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
  window does not stay always-on-top after handoff. Inspect the attempt's `handoff`
  result; record any OS denial instead of treating taskbar flashing as success.
- Reproduce the Task Manager task: Ctrl+Shift+Escape, filter `chr`, sort Memory
  descending, identify the first matching row and its displayed value. Check the
  foreground metadata, search result, sort indicator and image/desktop mapping.
- Test a target at higher integrity than klickwerk: no input should be sent, and
  the UI should explain the privilege mismatch with Continue/refinement available.
  The recovery card must replace an unhelpful request to type only `chr` manually:
  manual text entry does not remove the permission block for later sorting.
  An equally elevated sender and fixture target must remain eligible; own windows
  must still be rejected.
- With a higher-integrity Task Manager, choose **Restart as administrator**. Cancel
  UAC first: the original session, evidence and refinement must remain. Retry and
  approve as the same Windows user: one restored app opens, paused, with all prior
  steps, attempts, images and draft text. Continue must capture a NEW desktop and
  proceed to focus/filter/sort, rather than repeat the launch or old permission
  refusal. Verify input to the now equally elevated target and takeover monitoring.
  Test paths containing spaces/non-ASCII characters, repeat restart clicks, stale
  run IDs and denied launches. No UAC prompt may originate from a model action.
- Export before cancellation, after cancellation, after restart and after Continue.
  Inspect `recovery_events` for both integrity levels, failure source, UAC error
  1223 on cancellation, and successful restoration. Earlier attempt outcomes must
  remain unchanged. A restart transfer failure must leave the original app open.
  Test the actual UAC flow on Windows 11; Wine fixtures do not validate consent UI.
- Export after a refusal and after refinement. Inspect rejection codes, both
  integrity levels, window title/class/bounds, frame references, model and input
  timing, and handoff verification. Decode a `jpeg_base64` entry as a JPEG.
- Run beyond 12 observations: all history must remain, while older image evidence
  is evicted and `frames_omitted` increases. New task/deletion must clear evidence.
  Verify raw images are absent from workflow files and UI progress snapshots.

## Delayed launch and live filtering regression

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
