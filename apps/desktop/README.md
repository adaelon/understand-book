# Understand Book Windows: local library and cloud learning

For the independent Windows developer application with an in-process local Reader/AI runtime, see [Local Debug](../local-debug/README.md). Build it with `node apps/local-debug/package.mjs` from the repository root.

Current release: **0.3.8** (2026-10-26). [Download](https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.8). Includes native menu language switching and language continuity between the bundled local reader and cloud learning, with freshly built local reading assets. The hosted Reader includes the latest knowledge-board, formula, Harness and LangSmith updates. The installer is publicly available; the existing local installation remains 0.3.7. Build, 7 native tests, 8 web tests and both public download checks passed; installation and WebView2 startup verification were blocked. See [release evidence and limitations](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/windows-release-038-20261026/README.md).

Windows starts with a bundled local library. Import copies files into the application's local-data
`local-materials` directory without contacting the server or checking a storage subscription.
The app also hosts the deployed HTTPS Reader in Tauri/WebView2; cloud learning uses the website account.

## Local files and explicit cloud copies

The local library works without sign-in or a connection. Text, Markdown, EPUB, PDF and MP4 open in
the reader; PPT/PPTX originals open in the user's system presentation application. The native menu
returns to the library even when the cloud page is unavailable. Importing, reading and signing in do
not upload files. The saved copy survives changes to or deletion of the selected original file.

In a cloud workspace, `PersonalMaterials` chooses the native local import UI on desktop. Only the
explicit **同步到云端** action checks account storage and sends the file through the existing cloud
upload/preparation API. Native chunks bound memory use for video uploads. Stable upload/task IDs
allow interrupted cloud copies to resume; completed cloud files and local originals remain separate.
Removing a local copy does not remove the original selected file or the cloud copy. Local files do
not expire with cloud plans. Source-file edits are not automatically synchronized.

Native persistence and `local-material` Range responses live in `src-tauri/src/local_materials.rs`.
Only stored material IDs resolve to local content; the source protocol works from both the bundled
reader and the configured cloud origin. `local-library-probe` exercises the real commands and reader
in a separate hidden WebView2 profile without production accounts or model calls.

## Sign-in and requests

The app opens the system browser for explicit account consent. An ephemeral IPv4 loopback callback
and S256 PKCE exchange create an independent device session. The native process places that session
in WebView2 as a Secure, HttpOnly, persistent cookie and navigates to the same-origin cloud Reader.
The browser session is not copied. The normal logout endpoint revokes only the current device session.
Restart reuses this device's WebView2 profile; cancel and timeout discard an unfinished login.

The hosted Reader uses its existing same-origin `/api` paths for images, PDF, video Range, SSE,
private presentations, uploads and downloads. Reconnection follows the existing persisted run IDs
and submission recovery. File inputs, save dialogs and reading-image exports retain native support.
Only the exact compiled cloud origin and the local bootstrap may invoke the native commands.

## Build and delivery

Developer build prerequisites: Windows x64, Rust MSVC, WebView2, pnpm and Tauri NSIS tooling.
Ordinary users need Windows and WebView2; they do not need Codex, Bun, Python or a model account.

```powershell
$env:UNDERSTAND_BOOK_CLOUD_ORIGIN = "https://understandbook.top"
pnpm -C apps/desktop package:windows
```

The configured origin must serve the multi-user Reader and the desktop authorization endpoints.
The package command validates this configuration, builds NSIS and exports `dist/UnderstandBookSetup.exe`.
Prices, course content and account provisioning are server configuration; packaging does not create them.

The distributable contains the desktop executable and bundled local reading assets. Its Cargo dependencies
exclude server/runtime, and its bundle has no AI execution, publishing, or audio-processing sidecars.
The build prepares the local reading UI, including PDF rendering. Setup does not install
or uninstall Codex plugins.

## Existing local data and operator tooling

Upgrade and uninstall leave local book libraries, private records, provider settings and manually
installed plugins untouched. The cloud app uses a separate `cloud-reader` WebView2 profile and does
not automatically upload or migrate local files. The new local library stores its own imported copies;
cloud upload is a separate explicit action. Existing 0.3.0 installations require the new installer to
gain native local storage; the web UI explains the update instead of presenting cloud upload as local.

The intent-planning controller is a public optional operator CLI in `crates/maintenance-host`:

```powershell
cargo build -p understand-book-maintenance
$env:UNDERSTAND_BOOK_DESKTOP_EXE = (Resolve-Path target/debug/understand-book-maintenance.exe).Path
```

Operators provision `UNDERSTAND_BOOK_BUILD_EXE` and (when needed) `UNDERSTAND_BOOK_MCP_BIN` independently.
Standard prebuild and external-harness registration do not require the maintenance CLI; explicit
reading-goal planning does. See [the local plugin guide](../../plugins/understand-book/README.md)
for the current revision-bound command and local data-directory selection.
The existing build-sidecar and core maintenance scripts remain available. The source plugin under
`plugins/understand-book` is documented as an explicit maintenance installation; its launchers no
longer assume the cloud Setup includes local executables. Cloud read-only Book MCP remains the
separate account-authorized remote service.

## Verification and limits

The historical **0.3.1** x64 release was published (4,899,391 bytes). Native persistence/range tests,
11 relevant web tests, type checking, the bundled reader and NSIS build pass. A real hidden WebView2
opened TXT/MD/EPUB/PDF, retained the copy after removing the original, and reloaded/read offline with
zero cloud requests. Deployed web assets and the full public installer match the candidate bytes.
See [release evidence](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/windows-local-materials-20261021/README.md).
This restores local file storage and original-source reading. AI processing of local files without
creating a persistent cloud copy is not connected yet; existing cloud preparation remains separate.
The user's existing installation was not replaced during that release; it was later upgraded to 0.3.7 and remains at that version. The published 0.3.8 installer and its verification limits are recorded above. The 0.3.0 results below are historical.

Targeted checks: `cargo check -p understand-book-desktop --bin UnderstandBook`,
`cargo test -p understand-book-desktop --bin UnderstandBook`, web `desktop-auth.test.ts`, and
`test:cloud-release` with the deployment origin configured. The server suite checks one-use PKCE,
code expiry, callback binding, account revocation and independent-session logout.

A hidden WebView2 probe (`cloud-session-probe`) has verified real Secure/HttpOnly cookie creation,
persistent expiry, restoration in a second process, and deletion in a third process. It uses a
dedicated test profile and an `.invalid` origin without contacting a production account.
`test:maintenance-plugin` verifies the retained operator protocol and source mirrors separately.
The standalone `understand-book-maintenance` test binary passes all 18 tests, including the
retained library settings, plugin state, intent command, provider status and image export behavior.
The web authorization tests also cover a workspace change during consent and an account change
during consent: switching workspaces keeps the issued code, while switching accounts stops navigation.

The 0.3.0 x64 release NSIS package was built successfully with `package:windows` and exported as
`dist/UnderstandBookSetup.exe` (3,154,625 bytes). The optimized build completed in 8 minutes 26 seconds
with temporary files under `tmp/cloud-windows-temp` on the project drive. The generated release
NSIS application file section contains only `UnderstandBook.exe` (13,337,088 bytes before installer
compression); its resource and external-binary sections are empty. The package was published on 2026-10-21 at
https://understandbook.top/downloads/UnderstandBookSetup.exe alongside cloud release `cloud-20261021`.
Public download bytes and production PKCE exchange were verified. It has not been installed over a user's app.

The 0.3.0 debug NSIS candidate was also built successfully and exported as
`dist/UnderstandBookSetup-debug.exe` (4,261,577 bytes). Its generated NSIS application file section
contains only `UnderstandBook.exe`; there are no local engine, publishing or media sidecars.
Invoking the retired local build command on that executable exits with code 2 and directs the
operator to the maintenance executable. The candidate has not been installed over a user's app.

The matching cloud server/web release is deployed. Legacy implementations
already shipped to users cannot be recalled by changing new packages. The cloud app does not run
AI work offline; existing local reading data remains available to the user's older local tools.
