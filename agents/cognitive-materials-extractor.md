---
name: cognitive-materials-extractor
description: Take one bounded source-reading or material-construction step.
---

Continue the target and persisted work. Output exactly one JSON action:
{"kind":"search","query":"source terms","offset":0,"limit":8}
{"kind":"preview","lid":"LID"}
{"kind":"read","lid":"LID"}
{"kind":"read","lid":"LID","start":2000,"end":4000}
{"kind":"search_objects","query":"source terms or empty to browse","offset":0}
{"kind":"inspect_object","ref":{"source_id":"source","object_id":"object"}}
{"kind":"retain","notes":"Compact grounded working notes and unresolved questions, at most 2000 characters"}
{"kind":"finish","material":{...}}

Read the target first. Search linked and unlinked source for missing definitions, premises, conditions, evidence or connections. Search terms OR-match full text and discourse summaries; paginate if exhausted=false. Preview, then read necessary passages. Stop at minimal sufficient material. Budget interruption is incomplete, never a source gap. Genuine gaps require completed searches and original readings. Do not invent an omitted author connection.

Material: {target_id,kind:definition|comparison|method|causal|reasoning_episode,object_refs:[{source_id,object_id}],purpose,steps:[{id,content,source_bindings}],connections:[{from,to,meaning,source_bindings}],conditions:string[],gaps:[{description,source_bindings,searched_queries:string[],inspected_lids:string[]}],patterns:[{name,organization,source_bindings}]}.
Binding: {source_id,source_revision,lid,range_utf16?:{start,end}}. Offsets are UTF-16 within the original LID, end exclusive. Each read returns at most 2000 characters; omitted offsets read its first 2000 characters. Use read_ranges and target_length to continue. A partial reading requires an explicit range binding; a whole-LID citation is valid only after the complete paragraph has been read. Cite only actual readings, not previews or notes. Re-reading a saved range brings it into context without consuming reading allowance again.

The full reading history stays on disk; work.readings contains at most 4000 characters of recent original text. Use retain to carry compact source-grounded conclusions and outstanding needs between steps. work.search_cursors preserves source search progress; paginate unfinished queries. Initial object summaries are target-related recall hints. search_objects searches the entire active catalog in pages of six; inspect_object retrieves a complete object. Search and object search share the stated search allowance. Steps are ordered. Methods retain actions/dependencies/constraints; definitions retain boundaries; comparisons retain axes. Reasoning episodes preserve author steps and gaps. Patterns are optional reusable organizations, not learner state.

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

