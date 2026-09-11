# Public releases and portable updates

The public source and release repository is
[codehamr/klickwerk](https://github.com/codehamr/klickwerk).
Users download just [klickwerk.exe](https://github.com/codehamr/klickwerk/releases/latest/download/klickwerk.exe).
No GitHub account, access token, installer, background service or scheduled task
is needed. Windows 11 x64 and the system WebView2 Runtime are required.

## What users see

Every desktop start checks the public GitHub release, including renamed EXEs,
copies in other folders and local builds. A normal startup briefly shows the
check, then downloads a newer version with visible progress and restarts itself.
**Skip this time** immediately opens the existing version and cancels that update.
Cancellation is available until the short final replacement begins.

The new version opens with a brief success message. `config.cfg` and `workflows/`
stay beside the original EXE and are never copied, uploaded or removed by the
updater. The original filename and directory are preserved. No administrator
prompt is used for updates. Use a writable folder for the portable app.

Offline checks time out after five seconds; failed checks or downloads return to
the usable app with a quiet explanation. The footer offers **Check for updates**.
A check requested after startup only reports availability; it never restarts over
an unsent prompt, recording or task. The next normal start installs the update.
Administrator recovery also checks availability without interrupting the restored
session. An update handoff checks again, but cannot chain automatic restarts.

## Release contract

`.github/workflows/check.yml` checks the frontend, runs browser tests, checks and
tests the Rust core on Windows, and builds the portable EXE with its UI embedded.
Pushes to `main` in this repository, and manual runs on `main`, publish a release
only after both test/build jobs pass. Pull requests and other branches only check
and build. The Windows artifact contains exactly the binary built and tested by
that run; the publication job does not rebuild it.

Tags use UTC `YYYY-MM-DD-HHMMSS-RUN-ATTEMPT`, for example
`2026-09-11-123456-12-1`. The date is readable and repeated same-day builds have
different tags. CI embeds the tag in the executable. Published releases are never
deleted or overwritten to update a daily tag.

The release has two small, directly downloadable assets:

| Asset | Purpose |
| --- | --- |
| `klickwerk.exe` | The portable Windows x64 application |
| `klickwerk-update.json` | Version, immutable tag, filename, byte size and SHA-256 |

The release script creates a draft, uploads both assets, checks GitHub's reported
sizes and SHA-256 digests, and only then publishes it as latest. Failed uploads
leave a draft and the previous public release intact. It checks the current main
commit before creation and again before publication so an outdated build does
not replace the latest release. Only the publication job receives `contents: write`.
The Actions token is never embedded in either asset.

## Download and replacement

The application fetches the small manifest directly from
`https://github.com/codehamr/klickwerk/releases/latest/download/klickwerk-update.json`.
It does not use GitHub's rate-limited REST API for startup checks. No application
settings, prompts, screenshots, keys or workflow content are sent to GitHub.

The manifest pins the binary to its immutable release tag, avoiding a race with
a newer release. HTTPS redirects are limited to GitHub and its release asset
hosts. The client bounds the manifest to 16 KiB and the EXE to 64 MiB, rejects
unexpected filenames or formats, streams the download beside the current EXE,
and verifies its size, SHA-256 and Windows x64 PE header before replacement.
Release builds also reject older or equal release dates; matching hashes never
download again. Trust rests on the public GitHub repository and HTTPS. SHA-256
checks integrity; it is not a separate publisher signature or Authenticode signing.

On Windows, the running image is renamed into a unique `.klickwerk-update-*`
directory on the same volume. The verified image takes the original path. A
failed promotion or process launch restores the original. The new process waits
for the old process to exit before taking the single-instance mutex, then removes
its predecessor after creating its own application window. If rollback itself is
blocked by an external file lock or the computer loses power during replacement,
the old image remains recoverable as `previous.exe` in that hidden staging
directory. Close the app before manually recovering it. Abrupt process termination
can leave a staging directory; successful downloads and normal cancellations
clean up their own files without sweeping unrelated directories.

## Safe verification

`npm run test:core` covers strict manifests, redirect origins, version ordering,
bounded HTTP responses, broken downloads, cancellation, rollback and portable
data preservation. Its HTTP fixtures use only loopback servers and temporary
files. On Windows, a copied test executable verifies replacement of a running,
renamed image; it never starts the desktop controller or generates desktop input.
`node --test scripts/release.test.mjs` checks the asset/manifest publication contract.

Browser tests simulate startup, progress, success, offline use and cancellation,
including German and a narrow dark window. Preview scenarios such as
`/?update=downloading`, `/?update=success` and `/?update=error` never contact GitHub
or replace an executable. Do not launch the production desktop app as a test of
automation; its startup updater is intentionally live.
