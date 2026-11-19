# BookStructure paged theme planning

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` and use its output. Write one strict JSON candidate to `candidate_path`, execute `submit_command`, and return only the receipt. Use `heartbeat_command` while active. On failure execute `fail_command`. Write native or executor-reported usage to `usage_path` when available. Without this envelope, follow the strict JSON contract below.

Read this page's chapter questions and complete chapter summaries, plus its macro-route candidate previews. Update the directory of cross-chapter questions. Preserve distinctions between mechanisms, conditions and tradeoffs. A theme can start with one chapter and gain others on later pages.

Return one strict JSON action:
{"kind":"advance","themes":[{"ref":"theme:stable-name","question":"Substantive question","distinction":"How it differs from the others","unit_lids":["chapter-id"],"member_refs":["candidate-ref"]}]}

Use the same ref to extend an existing question. Supply question and distinction when creating it. unit_lids adds chapters from this delivery; use [] to revise wording or seeds alone. If supplied, member_refs replaces the theme's seed list with up to six previously selected or currently delivered candidate refs from its chapters. Omit member_refs to preserve the seed list. Seed references are locations for later evidence inspection.
Include only actual updates. Add optional "remove":["theme:obsolete"] to withdraw earlier tentative questions when they overlap or no longer fit. The directory can contain up to sixteen distinct questions. An empty themes array advances a page with no updates. Core advances both lists exactly through the delivered page. After the last page, only questions spanning at least two chapters proceed to theme development.
Keep questions and distinctions concise and use the book's language. Return JSON only.
