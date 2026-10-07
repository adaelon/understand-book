# BookStructure bounded chapter selection

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Continue the named chapter from its accepted action boundary. The complete browsing and source evidence ledgers are retained. Read selection_submission.continuation for the reason this delivery contract was requested. When revision is supplied, address revision.issue using its candidate_refs and previous_selection; the existing staged stops remain selected, and select_stops can append the required additional choices.
Preserve mechanisms, applicability conditions, tradeoffs and teaching value. Choose by content, with no per-section quota. Keep the complete ordered teaching selection; do not remove choices merely to fit one candidate request. Program assembly carries candidate bodies and sources; do not rewrite them or merge different candidates by shared LID.

Return one strict JSON action per call. Stage the ordered teaching selection in finite parts:
{"kind":"select_stops","refs":["complete-candidate-ref"]}
Each part contains 1 to 48 complete canonical references from this chapter's seen_refs. Parts append to selection_submission.accepted_stop_refs; never repeat an already staged ref. Staging is an unfinished draft and does not accept the chapter.
After all teaching stops have been staged, finalize:
{"kind":"select","selection":{"unit_lid":"...","role":"application","summary":{"text":"...","evidence_lids":["..."]},"macro_stop_refs":["..."]}}
Do not repeat accepted_stop_refs in the final action: Core injects the entire accumulated selection. macro_stop_refs is the ordered shorter overview route and must be a subset of the staged stops. Keep every established mainline mechanism. Roles: setup, foundation, method, application, case, synthesis.

The chapter summary may cite only previously_delivered_evidence_lids and evidence actually delivered by the current inspected candidates or excerpts. Previews, source indexes and working notes grant no evidence. If more source evidence is needed, these ordinary chapter actions remain available:
{"kind":"inspect","refs":["seen-candidate-ref"],"notes":"optional updated working notes"}
{"kind":"read","indices":[0],"notes":"optional updated working notes"}
{"kind":"browse","offset":64,"notes":"optional updated working notes"}
{"kind":"source_browse","offset":64,"notes":"optional updated working notes"}
Inspect at most 6 refs or read at most 3 stable global excerpt indices per call. Notes replace prior notes. Use index.next_offset and source_page.next_offset for browsing; every candidate must have been browsed before final select. inspected candidates supply source_indices for their actual evidence. Repeated previews may be omitted after delivery; seen_refs and index totals retain the complete browsing state.
Use canonical display titles; LIDs do not encode chapter numbers. No duplicate or foreign refs, no dependencies. Unselected candidates remain available for future work. Use the book's language. Return JSON only, without Markdown.
