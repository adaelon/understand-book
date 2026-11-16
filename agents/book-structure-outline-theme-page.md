# BookStructure paged provisional themes

## Automatic Build Executor Envelope

When the caller supplies an `automatic_build_executor.v1` envelope, execute `input_command` yourself and use its stdout as the input below. Produce the strict candidate JSON directly at `candidate_path`. If the harness exposes a native or executor-reported usage receipt, write `automatic_build_usage_receipt.v1` at `usage_path`; otherwise leave it absent, and never invent exact token counts. Execute `submit_command` and return only its receipt JSON. Never return candidate JSON to the caller. Use `heartbeat_command` while work is active; on failure execute `fail_command` and return only the failure receipt. Without this envelope, follow the ordinary strict-JSON output contract below.

Organize distinct questions that develop across the book from accepted anchored chapter explanations.
The book directory provides canonical order and titles. chapters.items and reading provide evidence.
themes.items is a page of accepted themes with stable zero-based theme_index values.
Return one strict JSON action per call.
{"kind":"browse","offset":16}
{"kind":"inspect","unit_lids":["..."]}
{"kind":"browse_themes","offset":0}
{"kind":"append","themes":[{"question":{"text":"...","evidence_lids":["..."]},"unit_lids":["..."]}]}
{"kind":"extend","theme_index":0,"unit_lids":["..."],"evidence_lids":["..."]}
{"kind":"finish"}

Use chapters.next_offset to browse until all chapters have been delivered. Its stride follows the
actual page capacity. inspect reads up to four units from anywhere in the book, including earlier
pages, so a theme can connect distant chapters. Use themes.next_offset or another valid theme offset
to inspect the accepted directory before appending a potentially overlapping theme.
Each new member must cite its question or progression evidence delivered in this call's chapters.items
or reading. Request an inspection first when those cards are absent. extend requires the theme's
directory entry on the current theme page and adds only new members with their delivered evidence.
Core retains previous members and citations. Keep distinct questions distinct and extend existing
themes when the same question develops in later chapters. A theme can begin with one grounded member.
Submit a small number of concise themes or members within output_contract.transport per action.
Finish only after every chapter has been delivered and the justified cross-chapter questions are
represented. An empty theme directory is valid. Use the book's language. Output the action only.
