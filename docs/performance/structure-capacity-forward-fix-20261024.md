# Structure organization capacity recovery — 2026-10-24

The optimization build reached `structure_execution_budget_exceeded`: rendered body
20,988 tokens, limit 20,000. Existing accepted artifacts remain authoritative.

`book-structure-organization.ts:renderStructureOrganizationInput` previously compacted
only oversized outlines. It now also removes JSON formatting overhead from oversized
chapter, theme and reconciliation bodies. Parsed content, prompts and capacity gates
are unchanged; fitting inputs and the established theme-plan rendering stay byte-stable.

Three synthetic regressions failed before the fix and passed afterward. Sixteen tests
across outline capacity and themes passed; core typecheck passed. The normal packaged
engine was rebuilt and installed with the previous executable retained. Windows file
mapping prevented overwrite, so the running executable was renamed before replacement.
The original `retry_current` request became stale when descriptor discovery advanced;
a fresh ordinary Engine read retained plan authorization and returned three executors.
Live continuation confirmed 1372/1375 tasks and 11/14 chapters on the same invocation.

A later compact chapter body reached 20,460 tokens: its 960-reference browsing ledger
used 11,221 tokens and its 64-candidate preview used 6,347. Bounded observation retained
only field token estimates and array counts (`tmp/structure-capacity-fix-20261024/capacity-counts.json`).
`fitStructureOrganizationExecution` now halves only an oversized chapter preview until
the real execution budget passes or the page reaches one item. Its fitted context is
stored with the generation input, so acceptance grants browsing state only for the
actual delivered page. All prior references, notes, excerpts and evidence remain intact;
unshown candidates remain reachable through `next_offset`. Already-fitting inputs retain
their identities. A genuinely oversized excerpt still fails the existing budget gate.
Twenty-five capacity, chapter planning and theme tests passed; core typecheck passed.
The normal engine resumed the same invocation at 1387/1389 tasks and 12/14 chapters,
issuing two fresh handoffs after its ordinary read revalidated the original plan.
