# Windows 11 acceptance

Use a disposable desktop with an ordinary, non-elevated account. Record Windows
and WebView2 versions, model, monitor layout/scales, and results. Linux/Wine tests
do not establish physical hardware or WebView2 acceptance.

## Startup and connection

- Put the EXE in a writable folder and launch it without a development server or
  `dist` directory. Its interface must open from embedded assets.
- Confirm `config.cfg` is created beside the EXE, regardless of working directory.
- Enter a bare local address, an HTTPS URL, and a proxy API path in the same field.
  Load models, test the connection, save, and verify settings survive restart.
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
  end without automatic resumption, and held keys/buttons must be released.
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
5. After stopping, type a correction and choose Save as workflow without first
   continuing. Verify the model includes that unsent correction in its proposed
   start prompt and memory. Cancel preparation and test server failure/retry.
6. Edit the name, prompt, and internal instructions, then save. Restart the app,
   reopen the workflow, inspect its history, and edit/save the prompt again.
7. Use that workflow in a fresh instance with windows in different positions.
   Verify learned target descriptions are adapted to the new screenshot. Refine
   and save again; old and new corrections must remain in the same workflow.
8. Begin another task. No card from the previously completed task should remain.
9. Verify read-only folders and damaged workflow/config files produce useful
   errors without overwriting existing data. Delete a saved test workflow and
   confirm it stays removed after restart.
10. Test SAPI dictation for both tasks and refinements, including permission denial,
    cancellation, device removal, and the configured Windows speech language.
