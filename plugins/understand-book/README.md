# Understand Book local build and maintenance plugin

This source package connects an external Codex harness to the public Build Engine and optional
local Reader tools. The current packaged launchers use Windows `cmd.exe`; cloud Windows Setup
does not install these tools. Cloud reading uses the account's separately authorized remote
read-only Book MCP connection.

Provision only the tools used by the selected workflow:

| Workflow | Required tool |
| --- | --- |
| Register the executor role in an explicitly chosen Codex scope | The packaged registration script and agent template; no maintenance host |
| Standard book/paper prebuild, resume, or course build | `UNDERSTAND_BOOK_BUILD_EXE`, pointing to the deterministic Build Engine |
| Plan around an explicit reading goal and manage its local private artifacts | The Build Engine plus `UNDERSTAND_BOOK_DESKTOP_EXE`, pointing to the optional `understand-book-maintenance.exe` |
| Read a local book through Book MCP | `UNDERSTAND_BOOK_MCP_BIN`, pointing to the separately built Book MCP executable |

The public source tree contains every local tool above. From its root, build the needed tools:

```powershell
node scripts/desktop/build-sidecar.mjs
# Keep the entire dist/build-engine/<native-target>/ directory and its adjacent resources.
$env:UNDERSTAND_BOOK_BUILD_EXE = (Resolve-Path 'dist/build-engine/x86_64-pc-windows-msvc/understand-book-build-x86_64-pc-windows-msvc.exe').Path

# Optional: reading-goal planning controller. It depends on reader-service, not the cloud server.
cargo build -p understand-book-maintenance --release
$env:UNDERSTAND_BOOK_DESKTOP_EXE = (Resolve-Path 'target/release/understand-book-maintenance.exe').Path

# Optional: local read-only Book MCP.
node scripts/plugin/build-book-mcp.mjs
$env:UNDERSTAND_BOOK_MCP_BIN = (Resolve-Path 'dist/book-mcp/x86_64-pc-windows-msvc/book-mcp-x86_64-pc-windows-msvc.exe').Path
```

`crates/maintenance-host` shares the root Cargo workspace and lockfile. Its
`--codex-build-intent` command accepts `codex_build_intent_command.v3`, uses the code-issued
`context_id`/`context_revision` for a candidate, and confirms `plan_id`/`plan_revision` over stdin.
The standard Build Engine route retains its own plan-digest contract.

For reading-goal planning against the local app, set `UNDERSTAND_BOOK_LOCAL_DATA_DIR` to the same
data home used by that app. The controller follows its `data-location.json`, reads the selected
directory's existing settings, and uses its `private/` store. An explicit
`UNDERSTAND_BOOK_PRIVATE_DIR` still overrides the private store. Existing Windows operator
installations can omit the local-data setting to keep `%LOCALAPPDATA%/UnderstandBook/settings.json`
and their existing private-store resolution. The controller passes `UNDERSTAND_BOOK_BUILD_EXE`
to Reader's deterministic Core bridge; `UNDERSTAND_BOOK_BUILD_SIDECAR` is an explicit override.

The registration skill installs only the executor role. Build execution additionally needs this
plugin enabled so its shared executor MCP transport is available. Registration, plugin installation,
and local plan confirmation are separate actions; follow the corresponding skill for each.

Cloud account provisioning, metered operator queues, shared publication and `publish_book` belong
to the cloud service distribution. They are not prerequisites for external-harness registration
or a standard local prebuild. DSH's independent standard-prebuild adapter likewise uses the Engine;
its optional cloud operator integration is documented separately in `packages/dsh-plugin/README.md`.

The launchers use the explicitly configured executables and do not infer them from the cloud
Reader installation directory. Cloud upgrades and uninstalls leave local libraries, private records,
model settings and manually installed plugins alone.

For a provisioned maintenance host, run the isolated controller check from the source root:

```powershell
cargo test -p understand-book-maintenance --bin understand-book-maintenance
node scripts/plugin/smoke-maintenance-host.mjs $env:UNDERSTAND_BOOK_DESKTOP_EXE
```

The CLI check creates an authored temporary book, checks context/draft/confirmation revisions,
Reader data-directory selection and the legacy operator settings path, then removes its fixture.
It uses no model provider and verifies that existing model settings are preserved.
