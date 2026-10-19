use schemars::{gen::SchemaGenerator, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use ts_rs::TS;

pub const BOOK_TOOL_CONTRACT_VERSION: &str = "book_tool_contract.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum BookToolId {
    Manifest,
    Text,
    SearchText,
    Context,
    Concept,
    Structure,
    GuidePath,
    PaperMetadata,
    PaperLexicon,
    PaperReadingGuide,
    Query,
    Synthesize,
    Guide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceAliases {
    pub resident: Option<&'static str>,
    pub mcp: Option<&'static str>,
    pub rest: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookToolContract {
    pub id: BookToolId,
    pub version: &'static str,
    pub aliases: SurfaceAliases,
    pub description: &'static str,
    pub use_when: &'static str,
    pub do_not_use_when: &'static str,
    pub result_contract: &'static str,
}

impl BookToolContract {
    /// MCP callers have no Resident routing card; include its shared guidance here.
    pub fn mcp_description(&self) -> String {
        format!(
            "{} Use when: {} {}",
            self.description, self.use_when, self.do_not_use_when
        )
        .replace("book.text", "book_text")
        .replace("book.structure", "book_structure")
    }
}

const CONTRACTS: &[BookToolContract] = &[
    contract(
        BookToolId::Manifest,
        None,
        Some("book_manifest"),
        Some("manifest"),
        "Return the read-only book manifest.",
        "Inspect public book identity and structure metadata.",
        "Do not treat manifest metadata as source-text evidence.",
        "book_manifest.v1",
    ),
    contract(
        BookToolId::Text,
        Some("book.text"),
        Some("book_text"),
        Some("text"),
        "Read canonical source text by one LID or an inclusive LID range.",
        "Read source text after a LID has been located.",
        "Do not use for lexical discovery when no LID is known.",
        "book_text.v1",
    ),
    contract(
        BookToolId::SearchText,
        Some("book.search_text"),
        Some("book_search_text"),
        Some("search_text"),
        "Enumerate the complete set of literal-text occurrences in canonical source, returning stable source UTF-16 ranges, the total count, and a pagination cursor.",
        "Locate the first, previous, or every lexical occurrence before interpreting it.",
        "Do not use lexical exhaustiveness as proof of semantic exhaustiveness.",
        "search_text.v1",
    ),
    contract(
        BookToolId::Context,
        Some("book.context"),
        Some("book_context"),
        Some("context"),
        "Return context pointers for a LID: near adds tree adjacency and local edges, mid adds other concept or entity occurrences, and far adds long-range edges. Counts cover only known rule-recalled pointers, not all semantic relations; built structure availability is reported by book.structure. Pointers contain no source text; use book.text to read it.",
        "Expand around a verified LID through deterministic context links.",
        "Do not treat pointers as source text.",
        "book_context.v1",
    ),
    contract(
        BookToolId::Concept,
        Some("book.concept"),
        Some("book_concept"),
        Some("concept"),
        "Deterministically recall whole-book concept and entity candidates with match reasons, bounded previews, and complete occurrence LIDs. Video graph candidates also retain distinct object meanings, text/visual/joint support, relations and video.read_frames hints. Read chosen text with book.text and actual video frames before citing.",
        "Discover concept or entity candidates when the user may not know the graph display label.",
        "Do not treat previews or graph support as observed evidence, or silently select a homonym. Read chosen occurrence LIDs with book.text and video_graph.read_hints with video.read_frames; pure visual objects have no invented LIDs.",
        "book_concept.v2",
    ),
    contract(
        BookToolId::Structure,
        Some("book.structure"),
        Some("book_structure"),
        Some("structure"),
        "Project BookStructure to explain a LID's role in the whole-book spine, throughline, and key stops. Without at, return the whole-book overview; when the sidecar is absent, return explicit unavailable status.",
        "Inspect the structural role of a LID or the whole-book overview.",
        "Do not use as a source-text search surface.",
        "book_structure.v1",
    ),
    contract(
        BookToolId::GuidePath,
        Some("book.guide_path"),
        Some("book_guide_path"),
        Some("guide_path"),
        "Return a macro guided-reading route from BookStructure by expanding key stops along spine segments. It does not interpret natural language or read Reader or memory state; when the sidecar is absent, it returns explicit unavailable status.",
        "Choose macro reading stops from BookStructure.",
        "Do not use for semantic question answering.",
        "book_guide_path.v1",
    ),
    contract(
        BookToolId::PaperMetadata,
        Some("book.paper_metadata"),
        Some("book_paper_metadata"),
        Some("paper_metadata"),
        "Return the current paper's metadata projection while preserving value, source, evidence_lids, and confidence. It returns explicit unavailable status without a sidecar and never creates cross-paper relationships.",
        "Read the current paper metadata projection.",
        "Do not infer cross-paper relations.",
        "paper_metadata_projection.v1",
    ),
    contract(
        BookToolId::PaperLexicon,
        Some("book.paper_lexicon"),
        Some("book_paper_lexicon"),
        Some("paper_lexicon"),
        "Return the current paper's lexicon projection for aligning term, abbreviation, and dataset candidates. It returns explicit unavailable status when the sidecar is absent.",
        "Align paper terms, abbreviations, and dataset candidates.",
        "Do not treat candidates as verified definitions without source text.",
        "paper_lexicon_projection.v1",
    ),
    contract(
        BookToolId::PaperReadingGuide,
        Some("book.paper_reading_guide"),
        Some("book_paper_reading_guide"),
        Some("paper_reading_guide"),
        "Return the read-only PaperReadingGuide projection, combining paper metadata and lexicon, BookStructure, graph, discourse, and source text into ten paper questions, a codebook, and abstract-reading aids. It never creates or changes persistent truth.",
        "Read a deterministic paper-reading projection.",
        "Do not use it to mutate persistent truth.",
        "paper_reading_guide.v1",
    ),
    contract(
        BookToolId::Query,
        Some("book.query"),
        Some("book_query"),
        Some("query"),
        "Answer a self-contained semantic question about explicit referents by resolving targets first and then reading source evidence around the frozen bindings.",
        "Answer definition, explanation, relation, or comparison obligations for explicit referents.",
        "Do not use for literal occurrence location.",
        "book_query.v1",
    ),
    contract(
        BookToolId::Synthesize,
        Some("book.synthesize"),
        Some("book_synthesize"),
        Some("synthesize"),
        "Synthesize a caller-supplied discrete LID set without expanding retrieval, returning an answer whose citations are a subset of the input LIDs.",
        "Synthesize a caller-supplied, frozen set of LIDs.",
        "Do not expand evidence beyond the supplied LIDs.",
        "book_synthesize.v1",
    ),
    contract(
        BookToolId::Guide,
        None,
        Some("book_guide"),
        None,
        "Open, refine, or close an ephemeral visitor guide session.",
        "Maintain a visitor-only ephemeral guided-reading cursor.",
        "Do not read Resident reader or memory state.",
        "book_guide.v1",
    ),
];

const fn contract(
    id: BookToolId,
    resident: Option<&'static str>,
    mcp: Option<&'static str>,
    rest: Option<&'static str>,
    description: &'static str,
    use_when: &'static str,
    do_not_use_when: &'static str,
    result_contract: &'static str,
) -> BookToolContract {
    BookToolContract {
        id,
        version: BOOK_TOOL_CONTRACT_VERSION,
        aliases: SurfaceAliases {
            resident,
            mcp,
            rest,
        },
        description,
        use_when,
        do_not_use_when,
        result_contract,
    }
}

pub fn contracts() -> &'static [BookToolContract] {
    CONTRACTS
}

pub fn contract_for(id: BookToolId) -> &'static BookToolContract {
    CONTRACTS
        .iter()
        .find(|contract| contract.id == id)
        .expect("every BookToolId must have a contract")
}

pub fn from_resident_alias(name: &str) -> Option<BookToolId> {
    from_alias(name, |aliases| aliases.resident)
}

pub fn from_mcp_alias(name: &str) -> Option<BookToolId> {
    from_alias(name, |aliases| aliases.mcp)
}

pub fn from_rest_alias(name: &str) -> Option<BookToolId> {
    from_alias(name, |aliases| aliases.rest)
}

fn from_alias(
    name: &str,
    projection: impl Fn(SurfaceAliases) -> Option<&'static str>,
) -> Option<BookToolId> {
    CONTRACTS
        .iter()
        .find(|contract| projection(contract.aliases) == Some(name))
        .map(|contract| contract.id)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EmptyInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TextInput {
    #[schemars(description = "Known LID from this book's locating or structure results; for a search hit, use start_lid. Reads the node's canonical source span, including descendants.")]
    pub lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Inclusive final LID in source reading order; for a search hit, use end_lid. Reads from lid's span start through end_lid's span end. Omit or null to read only lid's span.")]
    pub end_lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Absolute canonical-source UTF-16 position copied from a text continuation. Keep the original lid/end_lid. Omit or null for the initial read; a continuation returns a bounded page and its source_range, requested_range and next_start_utf16.")]
    pub start_utf16: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextGranularity {
    Near,
    Mid,
    Far,
}

impl ContextGranularity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Near => "near",
            Self::Mid => "mid",
            Self::Far => "far",
        }
    }
}

pub const DEFAULT_CONTEXT_GRANULARITY: ContextGranularity = ContextGranularity::Near;
pub const DEFAULT_CONTEXT_K: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextInput {
    #[schemars(description = "Known LID in this book to expand around. Returned items are context pointers; read their LIDs through the source-text tool for evidence.")]
    pub lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Context reach: near includes tree adjacency and local relations; mid adds other concept/entity occurrences; far also adds long-range relations. Omit or null for near.")]
    pub granularity: Option<ContextGranularity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum number of context pointers returned across all layers. Omit or null for 10; zero returns no items. total_candidates counts deduplicated pointers before k; returned_count and truncated describe selection by k before pagination or output budgeting. Counts pointers, not distinct LIDs, characters or source passages.")]
    pub k: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConceptInput {
    #[schemars(length(min = 1, max = 4096))]
    #[schemars(description = "Nonblank concept/entity name, alias or descriptive terms for deterministic candidate recall from names and occurrence text. At most 4,096 Unicode scalar values, including surrounding whitespace; not literal occurrence enumeration.")]
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional known LID in this book to rank nearby candidates and previews. Omit or null for unanchored ranking; this does not restrict recall to a subtree.")]
    pub anchor_lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum concept/entity candidates, 1..50; omit or null for 12. Inspect matched_count and truncated for candidate coverage; this does not limit occurrences within a candidate.")]
    pub limit: Option<usize>,
}

pub const DEFAULT_CONCEPT_LIMIT: usize = 12;
pub const MAX_CONCEPT_LIMIT: usize = 50;
pub const MAX_CONCEPT_QUERY_CHARS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AtInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional known LID in this book to locate its structural role or current route segment. Omit or null for the whole-book view; this is a location, not a natural-language query.")]
    pub at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaperMode {
    Skim,
    Close,
    Deep,
}

impl PaperMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skim => "skim",
            Self::Close => "close",
            Self::Deep => "deep",
        }
    }
}

impl Default for PaperMode {
    fn default() -> Self {
        Self::Skim
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaperStage {
    Passive,
    Active,
    Critical,
    Creative,
}

impl PaperStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Active => "active",
            Self::Critical => "critical",
            Self::Creative => "creative",
        }
    }
}

impl Default for PaperStage {
    fn default() -> Self {
        Self::Passive
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PaperReadingGuideInput {
    #[serde(default)]
    #[schemars(description = "Reading depth: skim, close or deep. Defaults to skim.")]
    pub mode: PaperMode,
    #[serde(default)]
    #[schemars(description = "Reading stage: passive, active, critical or creative. Defaults to passive.")]
    pub stage: PaperStage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BookQueryIntent {
    Definition,
    Explanation,
    Relation,
    Comparison,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct QueryObligation {
    #[schemars(description = "One nonblank, independently checkable requirement the semantic answer must address.")]
    pub requirement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BookQueryRequest {
    #[schemars(description = "Self-contained, nonblank semantic question about the named targets, with enough context to resolve referents. Use literal search for exact wording or first/previous occurrences.")]
    pub query: String,
    #[schemars(description = "Question kind: definition or explanation requires 1..3 targets; relation or comparison requires 2..3 separately named targets.")]
    pub intent: BookQueryIntent,
    #[schemars(length(min = 1, max = 3))]
    #[schemars(description = "Explicit referents to resolve before reading evidence. Each item is nonblank and unique after trimming and lowercasing; name comparison/relation targets separately, not as one combined phrase.")]
    pub targets: Vec<String>,
    #[schemars(length(min = 1, max = 3))]
    #[schemars(description = "One to three answer requirements, each in its own requirement object; these guide evidence sufficiency for the requested answer.")]
    pub obligations: Vec<QueryObligation>,
    #[schemars(description = "Required known LID in this book anchoring referent resolution. It provides local context without restricting evidence retrieval to this node.")]
    pub anchor_lid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SynthesizeInput {
    #[schemars(length(min = 1))]
    #[schemars(description = "Nonempty list of known LIDs selected as the complete evidence scope. Synthesis reads only these nodes and returns citations drawn from this list, without expanding retrieval.")]
    pub lids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Synthesis instruction for the supplied LIDs; omit or null to synthesize their content without an additional task.")]
    pub task: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuideAction {
    Guide,
    Close,
}

impl Default for GuideAction {
    fn default() -> Self {
        Self::Guide
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuideInput {
    #[serde(default)]
    #[schemars(description = "guide opens or refines an ephemeral visitor session and requires intent; close ends it and requires session_id. Defaults to guide.")]
    pub action: GuideAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Session identifier returned by an earlier guide call. For guide, omit or null to open a session, or supply it to refine that session; required for close.")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Nonblank natural-language reading goal, required on every guide action, including session refinement; unused for close.")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional known LID in this book to anchor guided reading. Omit or null when no location is chosen; unused for close.")]
    pub anchor_lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional reader feedback to combine with intent when generating the next guide; unused for close.")]
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMatchMode {
    Exact,
    Normalized,
}

impl Default for SearchMatchMode {
    fn default() -> Self {
        Self::Exact
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchRelativeDirection {
    Before,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SearchRelativeScope {
    #[schemars(description = "Known reference LID in this book. The relative filter excludes this node's entire source span, including its descendants.")]
    pub lid: String,
    #[schemars(description = "before keeps hits ending at or before the reference span's start; after keeps hits starting at or after its end. Direction filters scope independently of result order.")]
    pub direction: SearchRelativeDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SearchTextScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Known LID whose source span, including descendants, must contain each whole hit. Omit or null for no subtree restriction.")]
    pub within_lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional before/after filter around a known LID, excluding that node's span. Intersects with within_lid when both are supplied.")]
    pub relative_to: Option<SearchRelativeScope>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchOrder {
    Document,
    ReverseDocument,
}

impl Default for SearchOrder {
    fn default() -> Self {
        Self::Document
    }
}

fn default_search_page_size() -> usize {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SearchTextInput {
    #[schemars(length(min = 1, max = 4096))]
    #[schemars(description = "Literal text to locate in canonical source, containing non-whitespace text and at most 4,096 Unicode scalar values. Exact mode preserves query whitespace; this is not a semantic question or regex. Read chosen hits using their start_lid/end_lid.")]
    pub query: String,
    #[serde(default)]
    #[schemars(description = "exact (default) matches unchanged text; normalized applies NFKC, full non-Turkic case folding and whitespace collapsing. Both locate literal occurrences, not semantic equivalents; result ranges use original-source UTF-16 positions.")]
    pub match_mode: SearchMatchMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional source scope using known LIDs from this book. Omit, null or {} searches the whole book; within_lid and relative_to intersect, and each whole hit must fit the resulting range.")]
    pub scope: Option<SearchTextScope>,
    #[serde(default)]
    #[schemars(description = "document (default) returns earliest occurrences first; reverse_document returns latest first. Uses canonical source order, not lexical LID sorting. Combine before with reverse_document for previous occurrences.")]
    pub order: SearchOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Opaque next_cursor from the preceding search result. Omit or null for the first page; preserve query, match_mode, scope, order and page_size on continuation. A changed source revision invalidates it.")]
    pub cursor: Option<String>,
    #[serde(default = "default_search_page_size")]
    #[schemars(range(min = 1, max = 50))]
    #[schemars(description = "Maximum literal occurrences per search page, 1..50, default 20. Does not change total_occurrences or exhaustive coverage of the requested literal scope; preserve it when using cursor.")]
    pub page_size: usize,
}

pub fn search_text_input_schema() -> Value {
    schema_value::<SearchTextInput>()
}

pub fn validate_search_text_input(
    value: Value,
) -> Result<SearchTextInput, ContractValidationError> {
    let input: SearchTextInput = parse(value)?;
    validate_search_text_semantics(&input)?;
    Ok(input)
}

fn validate_search_text_semantics(input: &SearchTextInput) -> Result<(), ContractValidationError> {
    let query_length = input.query.chars().count();
    if input.query.trim().is_empty() {
        return Err(ContractValidationError {
            code: "SEARCH_QUERY_EMPTY",
            message: "search query must contain non-whitespace text".into(),
        });
    }
    if query_length > 4096 {
        return Err(ContractValidationError {
            code: "SEARCH_QUERY_TOO_LONG",
            message: "search query must contain at most 4096 Unicode scalar values".into(),
        });
    }
    if !(1..=50).contains(&input.page_size) {
        return Err(ContractValidationError {
            code: "BOOK_TOOL_INPUT_INVALID",
            message: "page_size must be between 1 and 50".into(),
        });
    }
    if let Some(scope) = &input.scope {
        if scope
            .within_lid
            .as_deref()
            .is_some_and(|lid| lid.trim().is_empty())
            || scope
                .relative_to
                .as_ref()
                .is_some_and(|relative| relative.lid.trim().is_empty())
        {
            return Err(ContractValidationError {
                code: "SEARCH_SCOPE_INVALID",
                message: "scope LIDs must not be empty".into(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookToolInput {
    Empty(EmptyInput),
    Text(TextInput),
    SearchText(SearchTextInput),
    Context(ContextInput),
    Concept(ConceptInput),
    At(AtInput),
    PaperReadingGuide(PaperReadingGuideInput),
    Query(BookQueryRequest),
    Synthesize(SynthesizeInput),
    Guide(GuideInput),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractValidationError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryValidationIssues {
    pub missing_requirements: Vec<String>,
    pub target_issues: Vec<String>,
}

pub fn input_schema(id: BookToolId) -> Value {
    let mut schema = match id {
        BookToolId::Manifest | BookToolId::PaperMetadata | BookToolId::PaperLexicon => {
            schema_value::<EmptyInput>()
        }
        BookToolId::Text => schema_value::<TextInput>(),
        BookToolId::SearchText => search_text_input_schema(),
        BookToolId::Context => {
            let mut schema = schema_value::<ContextInput>();
            schema["properties"]["granularity"]["default"] = json!(DEFAULT_CONTEXT_GRANULARITY);
            schema["properties"]["k"]["default"] = json!(DEFAULT_CONTEXT_K);
            schema
        },
        BookToolId::Concept => concept_input_schema(),
        BookToolId::Structure | BookToolId::GuidePath => schema_value::<AtInput>(),
        BookToolId::PaperReadingGuide => schema_value::<PaperReadingGuideInput>(),
        BookToolId::Query => {
            let mut schema = schema_value::<BookQueryRequest>();
            schema["anyOf"] = json!([
                {"properties":{"intent":{"enum":["definition","explanation"]}}},
                {"properties":{"intent":{"enum":["relation","comparison"]},"targets":{"minItems":2}}}
            ]);
            schema
        },
        BookToolId::Synthesize => schema_value::<SynthesizeInput>(),
        BookToolId::Guide => {
            let mut schema = schema_value::<GuideInput>();
            schema["anyOf"] = json!([
                {"properties":{"action":{"enum":["guide"]},"intent":{"type":"string","minLength":1}},"required":["intent"]},
                {"properties":{"action":{"enum":["close"]},"session_id":{"type":"string","minLength":1}},"required":["action","session_id"]}
            ]);
            schema
        },
    };
    let examples = input_examples(id);
    if !examples.is_empty() {
        schema["examples"] = json!(examples);
    }
    schema
}

/// Complete inputs for the small `fixtures/example-book` material. LIDs in a
/// real call still come from that book's locating results, as field docs specify.
pub fn input_examples(id: BookToolId) -> Vec<Value> {
    match id {
        BookToolId::SearchText => vec![
            json!({"query":"threshold","match_mode":"exact","order":"document","page_size":1}),
            json!({"query":"THRESHOLD","match_mode":"normalized","scope":{"within_lid":"1","relative_to":{"lid":"1.3","direction":"before"}},"order":"reverse_document","page_size":1}),
        ],
        BookToolId::Concept => vec![json!({"query":"threshold","anchor_lid":"1.2","limit":5})],
        BookToolId::Query => vec![json!({
            "query":"How do the activation conditions of the alpha threshold and beta threshold differ?",
            "intent":"comparison","targets":["alpha threshold","beta threshold"],
            "obligations":[{"requirement":"State the activation condition at each location."},{"requirement":"Compare whether equality activates each threshold, citing both passages."}],
            "anchor_lid":"1.1"
        })],
        BookToolId::Text => vec![json!({"lid":"1.1","end_lid":"1.2"})],
        BookToolId::Synthesize => vec![json!({"lids":["1.1","1.2"],"task":"Summarize these two threshold conditions using only these passages; state any evidence gaps."})],
        BookToolId::Context => vec![json!({"lid":"1.2","granularity":"near","k":3})],
        BookToolId::Structure => vec![json!({"at":"1.2"})],
        _ => vec![],
    }
}

pub fn concept_input_schema() -> Value {
    let mut schema = schema_value::<ConceptInput>();
    if let Some(limit) = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("limit"))
        .and_then(Value::as_object_mut)
    {
        limit.insert("default".into(), json!(DEFAULT_CONCEPT_LIMIT));
        limit.insert("minimum".into(), json!(1));
        limit.insert("maximum".into(), json!(MAX_CONCEPT_LIMIT));
    }
    schema
}

pub fn schema_value<T: JsonSchema>() -> Value {
    let root = SchemaGenerator::default().into_root_schema_for::<T>();
    let mut value = serde_json::to_value(root).expect("JsonSchema must serialize");
    let definitions = value
        .get("definitions")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    inline_tool_schema(&mut value, &definitions);
    if let Some(object) = value.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
        object.remove("definitions");
    }
    value
}

fn inline_tool_schema(value: &mut Value, definitions: &serde_json::Map<String, Value>) {
    let referenced = value
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/definitions/"))
        .and_then(|name| definitions.get(name))
        .cloned();
    if let Some(mut referenced) = referenced {
        if let (Some(target), Some(source)) = (referenced.as_object_mut(), value.as_object()) {
            for (key, value) in source {
                if key != "$ref" {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        *value = referenced;
        inline_tool_schema(value, definitions);
        return;
    }

    let single_all_of = value
        .get("allOf")
        .and_then(Value::as_array)
        .filter(|variants| variants.len() == 1)
        .and_then(|variants| variants.first())
        .cloned();
    if let Some(mut single_all_of) = single_all_of {
        if let (Some(target), Some(source)) = (single_all_of.as_object_mut(), value.as_object()) {
            for (key, value) in source {
                if key != "allOf" {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        *value = single_all_of;
        inline_tool_schema(value, definitions);
        return;
    }

    match value {
        Value::Array(values) => {
            for value in values {
                inline_tool_schema(value, definitions);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                inline_tool_schema(value, definitions);
            }
        }
        _ => {}
    }
}

pub fn validate_input(
    id: BookToolId,
    value: Value,
) -> Result<BookToolInput, ContractValidationError> {
    let input = match id {
        BookToolId::Manifest | BookToolId::PaperMetadata | BookToolId::PaperLexicon => {
            BookToolInput::Empty(parse(value)?)
        }
        BookToolId::Text => BookToolInput::Text(parse(value)?),
        BookToolId::SearchText => BookToolInput::SearchText(parse(value)?),
        BookToolId::Context => BookToolInput::Context(parse(value)?),
        BookToolId::Concept => BookToolInput::Concept(parse(value)?),
        BookToolId::Structure | BookToolId::GuidePath => BookToolInput::At(parse(value)?),
        BookToolId::PaperReadingGuide => BookToolInput::PaperReadingGuide(parse(value)?),
        BookToolId::Query => BookToolInput::Query(parse(value)?),
        BookToolId::Synthesize => BookToolInput::Synthesize(parse(value)?),
        BookToolId::Guide => BookToolInput::Guide(parse(value)?),
    };
    validate_semantics(id, &input)?;
    Ok(input)
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, ContractValidationError> {
    serde_json::from_value(value).map_err(|error| ContractValidationError {
        code: "BOOK_TOOL_INPUT_INVALID",
        message: error.to_string(),
    })
}

fn validate_semantics(
    id: BookToolId,
    input: &BookToolInput,
) -> Result<(), ContractValidationError> {
    let invalid = |message: String| ContractValidationError {
        code: "BOOK_TOOL_INPUT_INVALID",
        message,
    };
    match (id, input) {
        (BookToolId::Text, BookToolInput::Text(input)) => {
            require_non_empty("lid", &input.lid)?;
            if let Some(end_lid) = &input.end_lid {
                require_non_empty("end_lid", end_lid)?;
            }
        }
        (BookToolId::SearchText, BookToolInput::SearchText(input)) => {
            validate_search_text_semantics(input)?;
        }
        (BookToolId::Context, BookToolInput::Context(input)) => {
            require_non_empty("lid", &input.lid)?;
        }
        (BookToolId::Concept, BookToolInput::Concept(input)) => {
            require_non_empty("query", &input.query)?;
            if input.query.chars().count() > MAX_CONCEPT_QUERY_CHARS {
                return Err(invalid(format!(
                    "query must contain at most {MAX_CONCEPT_QUERY_CHARS} Unicode scalar values"
                )));
            }
            if let Some(anchor_lid) = &input.anchor_lid {
                require_non_empty("anchor_lid", anchor_lid)?;
            }
            if input
                .limit
                .is_some_and(|limit| !(1..=MAX_CONCEPT_LIMIT).contains(&limit))
            {
                return Err(invalid(format!(
                    "limit must be between 1 and {MAX_CONCEPT_LIMIT}"
                )));
            }
        }
        (BookToolId::Structure | BookToolId::GuidePath, BookToolInput::At(input)) => {
            if let Some(at) = &input.at {
                require_non_empty("at", at)?;
            }
        }
        (BookToolId::Query, BookToolInput::Query(input)) => {
            if let Err(issues) = validate_query_request(input) {
                return Err(invalid(format!(
                    "query validation failed: missing={:?}; targets={:?}",
                    issues.missing_requirements, issues.target_issues
                )));
            }
        }
        (BookToolId::Synthesize, BookToolInput::Synthesize(input)) => {
            if input.lids.is_empty() {
                return Err(invalid("lids must contain at least one item".into()));
            }
            for lid in &input.lids {
                require_non_empty("lids[]", lid)?;
            }
        }
        (BookToolId::Guide, BookToolInput::Guide(input)) => match input.action {
            GuideAction::Guide => {
                let intent = input.intent.as_deref().unwrap_or_default();
                require_non_empty("intent", intent)?;
            }
            GuideAction::Close => {
                let session_id = input.session_id.as_deref().unwrap_or_default();
                require_non_empty("session_id", session_id)?;
            }
        },
        _ => {}
    }
    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<(), ContractValidationError> {
    if value.trim().is_empty() {
        Err(ContractValidationError {
            code: "BOOK_TOOL_INPUT_INVALID",
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(())
    }
}

pub fn validate_query_request(request: &BookQueryRequest) -> Result<(), QueryValidationIssues> {
    let mut missing_requirements = Vec::new();
    let mut target_issues = Vec::new();
    if request.query.trim().is_empty() {
        missing_requirements.push("query".into());
    }
    if request.anchor_lid.trim().is_empty() {
        missing_requirements.push("anchor_lid".into());
    }
    if request.targets.is_empty() {
        missing_requirements.push("targets".into());
    }
    if request.obligations.is_empty() {
        missing_requirements.push("obligations".into());
    }

    let target_range = match request.intent {
        BookQueryIntent::Definition | BookQueryIntent::Explanation => 1..=3,
        BookQueryIntent::Relation | BookQueryIntent::Comparison => 2..=3,
    };
    if !request.targets.is_empty() && !target_range.contains(&request.targets.len()) {
        target_issues.push(format!(
            "intent {:?} requires {} targets",
            request.intent,
            if matches!(
                request.intent,
                BookQueryIntent::Definition | BookQueryIntent::Explanation
            ) {
                "1..3"
            } else {
                "2..3"
            }
        ));
    }
    if request
        .targets
        .iter()
        .any(|target| target.trim().is_empty())
    {
        target_issues.push("targets must not contain empty items".into());
    }
    let unique_targets: HashSet<String> = request
        .targets
        .iter()
        .map(|target| target.trim().to_lowercase())
        .collect();
    if unique_targets.len() != request.targets.len() {
        target_issues.push("targets must be unique".into());
    }
    if request.obligations.len() > 3 {
        missing_requirements.push("obligations must contain 1..3 items".into());
    }
    if request
        .obligations
        .iter()
        .any(|obligation| obligation.requirement.trim().is_empty())
    {
        missing_requirements.push("obligations must not contain empty requirements".into());
    }

    if missing_requirements.is_empty() && target_issues.is_empty() {
        Ok(())
    } else {
        Err(QueryValidationIssues {
            missing_requirements,
            target_issues,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn advertised_examples_are_complete_distinct_typed_inputs() {
        let mut count = 0;
        for contract in contracts() {
            let schema = input_schema(contract.id);
            let Some(examples) = schema["examples"].as_array() else { continue };
            assert_eq!(examples, &input_examples(contract.id));
            for (index, example) in examples.iter().enumerate() {
                assert!(!examples[..index].contains(example));
                validate_input(contract.id, example.clone()).unwrap();
                count += 1;
            }
        }
        assert_eq!(count, 8);
    }

    #[test]
    fn common_parameters_explain_nested_fields_and_tool_specific_meanings() {
        fn check_fields(schema: &Value) {
            match schema {
                Value::Object(object) => {
                    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                        for (name, property) in properties {
                            assert!(property["description"].as_str().is_some_and(|s| !s.is_empty()), "{name}: {property}");
                        }
                    }
                    for value in object.values() { check_fields(value); }
                }
                Value::Array(items) => items.iter().for_each(check_fields),
                _ => {}
            }
        }
        for contract in contracts() {
            // Inspect property definitions, not conditional constraint fragments.
            check_fields(&input_schema(contract.id)["properties"]);
            if let Some(properties) = input_schema(contract.id)["properties"].as_object() {
                for (name, property) in properties {
                    assert!(property["description"].as_str().is_some_and(|s| !s.is_empty()), "{:?}.{name}", contract.id);
                }
            }
        }
        let literal = input_schema(BookToolId::SearchText);
        let concept = input_schema(BookToolId::Concept);
        let semantic = input_schema(BookToolId::Query);
        assert!(literal["properties"]["query"]["description"].as_str().unwrap().contains("Literal"));
        assert!(concept["properties"]["query"]["description"].as_str().unwrap().contains("concept"));
        assert!(semantic["properties"]["query"]["description"].as_str().unwrap().contains("semantic"));
        assert_ne!(literal["properties"]["query"], concept["properties"]["query"]);
        assert!(literal["properties"]["cursor"]["description"].as_str().unwrap().contains("page_size"));
        let context = input_schema(BookToolId::Context);
        assert_eq!(context["properties"]["granularity"]["default"], "near");
        assert_eq!(context["properties"]["k"]["default"], 10);
        assert_eq!(context["properties"]["k"]["minimum"].as_f64(), Some(0.0));
        assert!(validate_input(BookToolId::Context, json!({"lid":"1.1","k":0})).is_ok());
        assert!(validate_input(BookToolId::Context, json!({"lid":"1.1","k":null,"granularity":null})).is_ok());
    }

    #[test]
    fn aliases_are_unique_per_surface() {
        fn assert_unique(values: Vec<&'static str>) {
            let unique: HashSet<_> = values.iter().copied().collect();
            assert_eq!(unique.len(), values.len());
        }
        assert_unique(
            contracts()
                .iter()
                .filter_map(|contract| contract.aliases.resident)
                .collect(),
        );
        assert_unique(
            contracts()
                .iter()
                .filter_map(|contract| contract.aliases.mcp)
                .collect(),
        );
        assert_unique(
            contracts()
                .iter()
                .filter_map(|contract| contract.aliases.rest)
                .collect(),
        );
    }

    #[test]
    fn model_visible_contract_metadata_is_authored_in_english() {
        fn contains_han(value: &str) -> bool {
            value.chars().any(|character| {
                matches!(
                    character as u32,
                    0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
                )
            })
        }

        for contract in contracts() {
            assert!(!contains_han(contract.description), "{:?}", contract.id);
            assert!(!contains_han(contract.use_when), "{:?}", contract.id);
            assert!(!contains_han(contract.do_not_use_when), "{:?}", contract.id);
        }
    }

    #[test]
    fn resident_contract_schema_is_closed_and_typed() {
        for contract in contracts()
            .iter()
            .filter(|contract| contract.aliases.resident.is_some())
        {
            assert_eq!(
                input_schema(contract.id)["additionalProperties"],
                Value::Bool(false),
                "{}",
                contract.aliases.resident.unwrap()
            );
        }
        assert_eq!(input_schema(BookToolId::Text)["required"], json!(["lid"]));
        assert_eq!(
            input_schema(BookToolId::Context)["properties"]["granularity"]["anyOf"][0]["enum"],
            json!(["near", "mid", "far"])
        );
        assert_eq!(input_schema(BookToolId::Context)["properties"]["granularity"]["anyOf"][1], json!({"type":"null"}));
        assert!(validate_input(BookToolId::Context, json!({"lid":"1.1","granularity":null})).is_ok());
        assert_eq!(
            input_schema(BookToolId::PaperReadingGuide)["properties"]["mode"]["default"],
            "skim"
        );
        assert_eq!(
            input_schema(BookToolId::PaperReadingGuide)["properties"]["mode"]["enum"],
            json!(["skim", "close", "deep"])
        );
        assert_eq!(
            input_schema(BookToolId::PaperReadingGuide)["properties"]["stage"]["default"],
            "passive"
        );
    }

    #[test]
    fn query_validator_enforces_intent_dependent_target_count() {
        let value = json!({
            "query": "How are alpha and beta related?",
            "intent": "relation",
            "targets": ["alpha"],
            "obligations": [{"requirement": "Explain the relation"}],
            "anchor_lid": "1.1"
        });
        let error = validate_input(BookToolId::Query, value).unwrap_err();
        assert!(error.message.contains("requires 2..3 targets"));
    }

    #[test]
    fn validator_rejects_unknown_fields_and_applies_defaults() {
        assert!(
            validate_input(BookToolId::Text, json!({"lid": "1.1", "unexpected": true})).is_err()
        );
        assert_eq!(
            validate_input(BookToolId::PaperReadingGuide, json!({})).unwrap(),
            BookToolInput::PaperReadingGuide(PaperReadingGuideInput {
                mode: PaperMode::Skim,
                stage: PaperStage::Passive,
            })
        );
    }

    #[test]
    fn concept_v2_contract_and_validation() {
        let contract = contract_for(BookToolId::Concept);
        assert_eq!(contract.aliases.resident, Some("book.concept"));
        assert_eq!(contract.aliases.mcp, Some("book_concept"));
        assert_eq!(contract.aliases.rest, Some("concept"));
        assert_eq!(contract.result_contract, "book_concept.v2");
        assert!(contract.description.contains("candidates"));
        assert!(contract.description.contains("book.text"));
        assert!(contract.use_when.contains("candidates"));
        assert!(contract.do_not_use_when.contains("previews"));
        assert!(contract.do_not_use_when.contains("silently select"));

        let schema = input_schema(BookToolId::Concept);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["query"]));
        assert_eq!(
            schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["anchor_lid", "limit", "query"]
        );
        assert_eq!(schema["properties"]["limit"]["default"], 12);
        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 50);
        assert_eq!(
            validate_input(BookToolId::Concept, json!({"query": "command"})).unwrap(),
            BookToolInput::Concept(ConceptInput {
                query: "command".into(),
                anchor_lid: None,
                limit: None,
            })
        );
        for invalid in [
            json!({"name": "command"}),
            json!({"query": " "}),
            json!({"query": "command", "anchor_lid": " "}),
            json!({"query": "command", "limit": 0}),
            json!({"query": "command", "limit": 51}),
        ] {
            assert!(validate_input(BookToolId::Concept, invalid).is_err());
        }
        assert!(validate_input(
            BookToolId::Concept,
            json!({"query": "x".repeat(MAX_CONCEPT_QUERY_CHARS + 1)})
        )
        .is_err());
    }

    #[test]
    fn search_text_contract_applies_defaults_and_rejects_invalid_bounds() {
        let contract = contract_for(BookToolId::SearchText);
        assert_eq!(contract.aliases.resident, Some("book.search_text"));
        assert_eq!(contract.aliases.mcp, Some("book_search_text"));
        assert_eq!(contract.aliases.rest, Some("search_text"));
        assert_eq!(contract.result_contract, "search_text.v1");
        let input = validate_search_text_input(json!({"query": "Alpha"})).unwrap();
        assert_eq!(input.match_mode, SearchMatchMode::Exact);
        assert_eq!(input.order, SearchOrder::Document);
        assert_eq!(input.page_size, 20);
        assert_eq!(search_text_input_schema()["additionalProperties"], false);
        assert_eq!(search_text_input_schema()["required"], json!(["query"]));
        assert_eq!(
            search_text_input_schema()["properties"]["match_mode"]["default"],
            "exact"
        );
        assert_eq!(
            validate_search_text_input(json!({"query": "x", "page_size": 0}))
                .unwrap_err()
                .message,
            "page_size must be between 1 and 50"
        );
        assert_eq!(
            validate_search_text_input(json!({"query": "   "}))
                .unwrap_err()
                .code,
            "SEARCH_QUERY_EMPTY"
        );
        assert!(matches!(
            validate_input(
                BookToolId::SearchText,
                json!({
                    "query": "alpha",
                    "scope": {"relative_to": {"lid": "1.2", "direction": "before"}}
                })
            )
            .unwrap(),
            BookToolInput::SearchText(SearchTextInput {
                match_mode: SearchMatchMode::Exact,
                order: SearchOrder::Document,
                page_size: 20,
                ..
            })
        ));
    }
}
