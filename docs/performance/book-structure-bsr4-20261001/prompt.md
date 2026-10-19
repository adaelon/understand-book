# BookStructure theme work

Organize the book by questions that develop across chapters. Use the canonical titles, never derive display chapter numbers from LIDs. Shared vocabulary, similarity and topic membership do not establish identity or a reading prerequisite. Preserve mechanisms and their conditions. A theme explains what changes at each stage; avoid near-synonymous two-chapter lists.

The input phase selects the output contract. Return one strict JSON object, without Markdown.

## plan

Read all chapter overviews and the compact candidate index. Produce a small finite directory of substantive questions, with clear boundaries between them. Include the supplied fixed_question as one theme, then cover other cross-chapter questions supported by the book. Do not create every pair of members as a separate task. A candidate may participate in different questions.

Output {"themes":[{"ref":"theme:stable-name","question":"...","distinction":"What this theme explains and how it differs from the others","unit_lids":["...","..."],"member_refs":["candidate-ref"]}]}. At most 16 themes; each has at least two distinct chapters. References must come from the input. Seed with relevant candidate refs, not all candidates. These are locations for later inspection, not evidence of completion.

## theme

The compact directory is shared context. Work only on the specified theme. Search can find additional members across the entire book. An empty query browses every chapter/candidate. Nonempty query uses the configured retrieval mode; semantic Top-K is incomplete. Search previews, source indexes and titles grant no evidence. Inspect shows complete accepted content; read shows source excerpts. Cite only evidence actually delivered in this work, including previously_delivered_evidence. Any selected candidate must have been inspected. Source indexes can be read directly. If needed, inspect chapter records to obtain their accepted overview evidence.

Choose one action:
- {"kind":"search","query":"words or question","offset":0,"notes":"optional durable notes"}. To page the same search keep the exact query and use next_offset. If zero hits, try shorter terms or browse.
- {"kind":"inspect","refs":["seen candidate or chapter key"],"notes":"optional"}. Up to 6 keys, from the search history or initial member_refs.
- {"kind":"read","indices":[0],"notes":"optional"}. Up to 3 source indices. Prefer small requests when excerpts are long.
- {"kind":"resolve","result":ThemeResult}.

ThemeResult = {"ref":"same theme identity","name":"concise name","summary":AnchoredText,"stages":[{"unit_lid":"...","development":AnchoredText,"member_refs":["inspected candidate ref"]}],"dependencies":[Dependency]}.
AnchoredText = {"text":"at most 600 characters","evidence_lids":["actual delivered evidence"]}.
Use at least two distinct chapters; repeated chapters may describe distinct stages. Each development states that stage's mechanism, added conditions and cost/judgment change, with evidence from that chapter. The summary explains the whole progression and cites every included chapter. Candidate refs select additional stops by reference; do not rewrite their bodies. Empty member_refs is allowed for a stage supported directly by read evidence. Do not duplicate the same ref across stages.

Dependency = {"unit_lid":"dependent chapter","depends_on":"prerequisite chapter","prerequisite":AnchoredText,"application":AnchoredText,"rationale":"why the later explanation requires the earlier concept"}. prerequisite cites only the prerequisite chapter; application cites only the dependent chapter. Both must have been delivered. A common analogy or recovery/state vocabulary is insufficient: identify the specific concept needed to understand the later material. Empty dependencies is valid. Do not impose dependencies merely to make a connected graph.

## reconcile

Perform one bounded pass over the complete resolved directory and its stage explanations. Output {"edits":[ThemeResult],"merges":[{"source_refs":["theme:a","theme:b"],"result":ThemeResult}],"unresolved":[]}.
Return only actual changes. Empty edits/merges is valid. Rename or change membership with an edit. A merge retains one source identity and all selected candidates and dependencies; include all source stages' chapter evidence. Use only material delivered to those source themes; no new unsupported claims. Distinct mechanisms can remain within one theme. Each ref may occur in at most one edit/merge. If a real boundary problem cannot be resolved with available evidence, put its concrete description in unresolved; it stays incomplete for later work. Do not rewrite spine/key_stops or start another whole-directory scan.
