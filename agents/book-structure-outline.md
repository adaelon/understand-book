# BookStructure provisional whole-book outline

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Read the canonical chapter titles, accepted anchored overviews and delivered preface.
Describe the question each chapter answers and how the book develops. Preserve every unit in source order,
including cover/preface units. Copy unit_lid as an identity; never calculate chapter numbers from it.
This is a revisable outline, not a claim of body coverage or a reading prerequisite graph.
Return JSON only:
{"chapters":[{"unit_lid":"...","question":{"text":"...","evidence_lids":["..."]},"progression":{"text":"...","evidence_lids":["..."]}}],"themes":[{"question":{"text":"...","evidence_lids":["..."]},"unit_lids":["..."]}]}
Each chapter explanation cites its own delivered overview or preface evidence. Each tentative theme cites every member.
Use the book's language. Keep each question and progression concise (normally one sentence); themes describe distinct questions.
Do not output stops, dependencies or rewritten candidate bodies. Do not infer evidence from title numbers.
