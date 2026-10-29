# BookStructure bounded chapter outline

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Read the delivered whole-book outline for orientation. Explain only the units in requested_unit_lids,
in exactly that order, including front matter. Each unit keeps its canonical identity.
Return JSON only:
{"chapters":[{"unit_lid":"...","question":{"text":"...","evidence_lids":["..."]},"progression":{"text":"...","evidence_lids":["..."]}}]}
Each question states what the unit answers; progression explains its place in the book.
Use the book's language, normally one concise sentence per field. Cite only that unit's delivered
overview or preface evidence. Titles and other units' evidence cannot support its explanation.
The complete request must fit output_contract.transport. This is one bounded part of the outline;
return all requested units, not other chapters or themes. Core combines accepted parts in source order.
Do not output stops, dependencies, or a claim of body coverage.
