# BookStructure chapter reference selection

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Organize the named chapter within the provisional whole-book outline. Examine the entire candidate index.
Keep mechanisms, applicability conditions, tradeoffs and teaching value; choose by content, with no per-section quota.
The index is a preview and grants no citation evidence. Inspect candidates for their accepted anchored content,
or read supplied source excerpts when the preview does not establish a condition or explanation.
Return one JSON action per call:
{"kind":"browse","offset":24,"notes":"working notes and useful refs from earlier pages"}
{"kind":"inspect","refs":["candidate-ref"],"notes":"optional updated working notes"}
{"kind":"read","indices":[0],"notes":"optional updated working notes"}
{"kind":"select","selection":{"unit_lid":"...","role":"application","summary":{"text":"...","evidence_lids":["..."]},"accepted_stop_refs":["..."],"macro_stop_refs":["..."]}}
Browse offsets come from index.next_offset. Inspect at most 6 refs, or read at most 3 excerpt indices per call.
Notes replace prior notes; preserve references and reasoning needed later. They are working memory, never source evidence.
Summary may cite only evidence delivered through inspected candidates or source reads (including previously delivered evidence).
Roles: setup, foundation, method, application, case, synthesis. Use canonical display titles; LIDs do not encode chapter numbers.
accepted_stop_refs is the ordered chapter teaching selection. macro_stop_refs is an ordered subset for the shorter overview route.
Program assembly carries candidate bodies and sources. Do not rewrite them, merge by shared LID or delete chapter choices to shorten the macro route.
No duplicate refs, no cross-chapter candidates, no dependencies. Unselected candidates remain available for future work.
Use the book's language. Return JSON only, without markdown.

## Full-chapter source navigation

When source_page is present, source_index is a page of this chapter's stable global excerpt indices. Use {"kind":"source_browse","offset":64,"notes":"optional updated notes"} with source_page.next_offset or a known chapter source-page offset to navigate. An absent entry on this page is not absent from the chapter. read uses the original excerpt indices. inspected candidates also supply source_indices for their actual evidence. Browsing grants no citation evidence.

The full-book candidate index uses 64 items per page. For browse, use index.next_offset from the delivered page; the earlier offset 24 is an example, not a fixed stride. Every candidate must still be browsed before select.

When index.preview_already_delivered is true, the current page was already delivered before this inspect/read action. Its repeated previews are omitted to leave room for the expanded content and notes. index.total, next_offset and seen_refs retain the complete browsing state; another browse delivers the requested page normally.
