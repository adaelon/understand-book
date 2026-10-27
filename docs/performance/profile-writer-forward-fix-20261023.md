# Profile sidecar writer recovery — 2026-10-23

Frozen profile tasks now use the existing core writers inside the executor engine, matching
Pass1. This removes an additional packaged-engine process and full-book parsing from each
profile submission while retaining task-private candidate validation, source and policy replay,
typed candidate failures, generation artifacts, and the mailbox receipt path.

The historical `writer_failed` receipts did not retain the underlying error. The failed
candidate passed deterministic validation with publication disabled. Host memory exhaustion
and unavailable executor startup were observed, but do not establish the original OS error.

The retry gate previously supported corrected writer diagnostics but lacked a successful
writer-repair record. `recordAutomaticBuildWriterForwardFix` now records a validation-backed
repair bound to the original scope, terminal receipt, and diagnostic. Driver accepts the
existing user decision only after that record exists; failure history remains immutable.

Validation: 16 writer/routing/CLI tests, 3 driver recovery tests, and 13 task-store tests passed;
core typecheck passed. The 100-event store test initially exceeded 5 seconds while compiling,
then passed alone in 2.9 seconds. The installed engine accepted `retry_current` on the same
invocation, committed the previously failed unit, and confirmed progress from 6143 to 6144.

Implementation: `skills/build/automatic-build.ts`, `packages/core/src/automatic-build-task-store.ts`,
and `packages/core/src/automatic-build-attempt-recovery.ts`. Local repair evidence and original
executable backup location are recorded in `tmp/writer-fix-20261023/repair-result.json`.
