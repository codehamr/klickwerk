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
