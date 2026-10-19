# BookStructure chapter reference selection

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
