# Tool input example material

Three short paragraphs distinguish the alpha threshold (`> 10`) from the beta
threshold (`>= 10`). `base.json` contains their actual source spans and two
concept candidates. `book_structure.json` supplies the structural view.
Line endings are pinned to preserve source spans and recorded publication sizes.

`book-tool-contracts::input_examples` uses these LIDs. The Resident
`source.present` example quotes the first sentence verbatim. Every root example
is a complete input object; optional fields can be omitted. For another book,
use that book's returned LIDs and observed source text.

`publication.json` and `publication-ref.json` were captured from the existing
`PublishedLibrary::publish` flow. Remote MCP advertises that reference as example
data. Its HTTP test restores the captured manifest and registry row into a fresh
local account store, with these exact material files, and submits the advertised
inputs unchanged. Other books require their reference from `book_list`.

To recapture after an intentional fixture change, set `TOOL_EXAMPLE_REFERENCE`
to an **output directory** and explicitly run the ignored
`mcp_contract_capture_example_publication` test. It publishes the fixture through
the existing readiness gate and writes both publication files. Replace both
recorded files together, then rebuild the tests. Ordinary tests do not overwrite
fixtures, publish to a production service, or call a model.
