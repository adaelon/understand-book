# BookStructure writer diagnostic recovery — 2026-10-24

The optimization build's last chapter exhausted retries with `writer_failed`.
A publication-disabled replay of its bound failed candidate found a Zod `too_big`
violation at `/notes`: the existing chapter action contract limits notes to 4,000 characters.

`acceptStructureOrganizationAction` now maps Zod failures through the existing
`throwExtractorSchemaError` path. The executor receives `schema_invalid`, the field,
and its unchanged bound; unexpected writer exceptions retain their original behavior.
No prompt bytes, accepted candidates, length limits or historical failures were rewritten.

The replay recorded the existing scope/terminal-bound writer diagnostic correction,
enabling candidate retry through the original invocation's `retry_current` decision.
Twenty-six capacity, planning and theme tests passed; core typecheck passed.
Bounded replay evidence: `.codex-recovery/optimization-structure-writer-diagnosis.json`.
The same invocation accepted `retry_current`, committed the repaired unit, and
advanced from 1403/1404 to 1404/1405 before issuing its next task.
