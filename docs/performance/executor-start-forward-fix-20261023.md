# Executor generation-start recovery — 2026-10-23

V3 generation start threw a generic error when dispatch advancement returned
`retry_exhausted`. This occurs when an earlier task in an ordered dispatch reaches its
semantic retry limit while input delivery has selected the next unclaimed task. MCP then
reported `executor_internal`; repeated control failures reached the bootstrap recovery
boundary even though the underlying task needed candidate retry authorization.

Seven bounded observations from the live build confirmed `phase=generation_start` and
`claim_status=retry_exhausted`. No source text, candidates, private paths, or arbitrary error
messages were captured. A synthetic ordered dispatch reproduced the exact exception.

`startAutomaticBuildExecutorGeneration` now closes the dispatch through the existing
canonical `task_failure` receipt and returns V3 `DONE/retryable_failure`, matching V1.
The unclaimed suffix and attempt counts are preserved. Ordinary generation, source/policy
validation, candidate schemas, and publication gates are unchanged.

Validation: 93 tests across executor sessions, dispatch runtime, and MCP passed; core
typecheck passed. Temporary diagnostic instrumentation was removed from the installed
engine. The original invocation resumed using its current code-issued retry decision.
Live recovery evidence is recorded in `tmp/executor-start-fix-20261023/diagnosis.json`.

After startup recovery, two bounded observations of schema failures showed a missing
formula parameter `unit` field (`invalid_type`, expected string, received undefined).
Only static schema field names and type names were retained. The existing formula
contract requires `label`, `unit`, and `domain`, each accepting a string or null.
Candidate correction feedback now explains those required nullable fields together
and directs the executor to use null for ungrounded details. Validation and extractor
prompt bytes remain unchanged. Twenty extractor-contract tests and core typecheck
passed, including rejection of each omitted nullable field and acceptance of its null
representation. The temporary schema observation build was removed before resuming.
Evidence: `tmp/executor-start-fix-20261023/schema-correction-evidence.json`.
The same live invocation then confirmed seven consecutive commits, advancing from
6192/6265 to 6199/6265 before the next dispatch's existing bootstrap recovery boundary.
After the authorized bootstrap retries, it completed Profile sidecar at 6265/6265
and entered BookStructure with the original plan and invocation.
