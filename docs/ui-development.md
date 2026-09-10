# UI development

Run `npm run dev`, then open port 1420. The React preview needs no native build.
It uses synthetic state and never contacts a real model or controls the desktop.
The banner identifies this mode. Preview settings use the browser's local storage;
API keys are never persisted by the preview adapter.

The visual system lives in `src/styles.css`: semantic light/dark variables,
spacing, typography, component states, focus rings, reduced motion, and forced-color
support. UI components remain local and editable. Radix provides dialog focus
management, tooltips, and switch semantics. Use English UI text and identifiers.

Extend both the native bridge types and browser fixture adapter when adding a new
state. Exercise empty, loading, running, waiting, stopped, done, and error states.
The preview query `?scenario=ask` shows a question; `?scenario=error` shows a server
failure. These fixture scenarios exist only in the browser adapter.

`npm test` exercises behavior and axe accessibility checks and writes screenshots
to `build/visual/`. Tests cover dialog focus, keyboard start/stop, cancellation,
settings persistence, multilingual task preservation, completion, error recovery,
questions, the safe stop simulation, dark mode, reduced motion, and narrow layout.
On Linux, an installed `/usr/bin/chromium` can be used; otherwise run
`npx playwright install --with-deps chromium`. `PLAYWRIGHT_CHROMIUM_EXECUTABLE`
can select another test browser binary.

The container uses `VITE_USE_POLLING=true` because Windows bind mounts may not
forward filesystem change notifications. Vite's cache and Rust outputs live on
Linux filesystems. `PLAYWRIGHT_OUTPUT_DIR` can similarly relocate test traces if
Windows locks the default `test-results/` directory.

Do not infer native hotkey, input, WebView2, or Windows accessibility behavior from
browser tests. Those require the separate native fixtures and Windows acceptance.
