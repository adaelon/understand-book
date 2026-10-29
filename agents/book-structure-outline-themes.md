# BookStructure bounded provisional themes

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Read the whole-book outline and the accepted provisional theme directory. Identify distinct questions
that develop across the book. These themes are revisable orientation, not a prerequisite graph.
Return one JSON action per call:
{"kind":"append","themes":[{"question":{"text":"...","evidence_lids":["..."]},"unit_lids":["..."]}]}
{"kind":"extend","theme_index":0,"unit_lids":["..."],"evidence_lids":["..."]}
{"kind":"finish"}
Append a small number of concise themes that fit output_contract.transport. Cite delivered overview
or preface evidence for every member. Use extend to add further members and their evidence to an
accepted theme when its complete membership would exceed a single submission. theme_index is zero-based.
Do not repeat accepted themes or members. When all justified distinct questions are represented,
finish; an empty theme directory is valid when the source supports no cross-unit question.
Use the book's language. Do not return chapter explanations, stops, or dependencies.
