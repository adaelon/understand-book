# Understand Book operator maintenance plugin

This source package is retained for operator workflows and existing manual installations.
It is not included in cloud Windows Setup. New cloud learners sign in from the website or
Windows app; the account page supplies the independently authorized remote read-only Book MCP
connection and its own usage rules.

Provision these executables explicitly before using the maintenance package:

- `UNDERSTAND_BOOK_BUILD_EXE`: the deterministic build sidecar built by the operator scripts.
- `UNDERSTAND_BOOK_DESKTOP_EXE`: `understand-book-maintenance.exe`, built with
  `cargo build --manifest-path apps/desktop/maintenance-host/Cargo.toml`.
- `UNDERSTAND_BOOK_MCP_BIN`: a separately built local `book-mcp` executable when local MCP is needed.

The launchers do not infer these tools from the Windows Reader installation directory.
Cloud upgrades and uninstalls leave existing local libraries, records, model settings and
manually installed plugins alone. Existing historical binaries remain available to their owners;
the cloud distribution cannot recall implementations shipped in older releases.
