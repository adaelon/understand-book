//! Account-authorized reads of immutable publications, independent of Reader state and billing.
use crate::{
    mcp_access::McpIdentity,
    published_library::{PublicationManifest, PublishedBookRef, PublishedLibrary},
    user_storage_paths::error,
};
use book_tool_contracts::{
    contract_for, from_mcp_alias, input_schema, validate_input, BookToolId, BookToolInput,
};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Mutex};

pub const MAX_PAGE_ITEMS: usize = 50;
const READ_TOOLS: [BookToolId; 9] = [
    BookToolId::Text,
    BookToolId::SearchText,
    BookToolId::Context,
    BookToolId::Concept,
    BookToolId::Structure,
    BookToolId::GuidePath,
    BookToolId::PaperMetadata,
    BookToolId::PaperLexicon,
    BookToolId::PaperReadingGuide,
];

pub(crate) fn is_content_tool(name: &str) -> bool {
    from_mcp_alias(name).is_some_and(|id| READ_TOOLS.contains(&id))
}

fn invalid(message: &str) -> ToolError {
    error("MCP_READ_INPUT_INVALID", "validation", message)
}
fn material_error(err: ToolError) -> ToolError {
    if err.error_code == "PUBLICATION_UNAVAILABLE" {
        error(
            "MCP_MATERIAL_UNAVAILABLE",
            "not_found",
            "Select a currently authorized publication from book_list",
        )
    } else {
        err
    }
}
fn default_limit() -> usize {
    20
}

/// A JSON Pointer selects a collection in the unchanged local tool projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionPage {
    path: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    #[serde(default)]
    after: Option<Value>,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn valid_limit(limit: usize) -> Result<(), ToolError> {
    if (1..=MAX_PAGE_ITEMS).contains(&limit) {
        Ok(())
    } else {
        Err(invalid("limit must be between 1 and 50"))
    }
}
fn reference(value: Value) -> Result<PublishedBookRef, ToolError> {
    if !value.as_object().is_some_and(|o| {
        o.len() == 2 && o.contains_key("book_id") && o.contains_key("publication_id")
    }) {
        return Err(invalid(
            "published_book_ref requires only book_id and publication_id",
        ));
    }
    let reference: PublishedBookRef =
        serde_json::from_value(value).map_err(|_| invalid("Invalid published_book_ref"))?;
    if reference.book_id.trim().is_empty() || reference.publication_id.trim().is_empty() {
        return Err(invalid(
            "Use book_id and publication_id returned by book_list",
        ));
    }
    Ok(reference)
}

/// This schema projection is also the input contract for the later remote transport.
pub fn tool_definitions() -> Vec<Value> {
    let ref_schema = json!({"type":"object","additionalProperties":false,
        "description":"Copy the exact immutable publication reference from book_list; keep both identifiers on every content call and continuation.",
        "properties":{"book_id":{"type":"string","minLength":1,"description":"Material identifier returned by book_list."},"publication_id":{"type":"string","minLength":1,"description":"Exact publication version returned by book_list."}},
        "required":["book_id","publication_id"]});
    let mut tools = vec![
        json!({"name":"book_list","description":"Start here to select authorized immutable publications, or refresh after material_unavailable. Costs 0 calls, including every directory page, even when content credits are exhausted. items contains name, published_book_ref (book_id + publication_id), is_default and available_tools for each publication. Copy a selected reference into content arguments and choose an available tool. An empty directory means this account has no authorized materials; ask the user to contact the administrator. complete=false means more items exist: copy next_calls name and arguments for needed pages. Returns usage without a charge; directory entries are not evidence and have no source citations.",
        "inputSchema":{"type":"object","additionalProperties":false,"properties":{
            "after":ref_schema,"limit":{"type":"integer","minimum":1,"maximum":50,"default":20,"description":"Maximum publications on this directory page."}}}}),
    ];
    tools[0]["inputSchema"]["properties"]["after"]["description"] = json!("Omit for the first page; copy the after reference from book_list next_calls for later pages.");
    for id in READ_TOOLS {
        let contract = contract_for(id);
        let mut schema = input_schema(id);
        for (key, description) in [
            ("lid", "LID returned by a locating tool in this publication; do not invent it."),
            ("end_lid", "Optional inclusive final LID for a source range; omit to read only lid."),
            ("query", "Nonblank literal text for book_search_text, or a concept/entity name or alias for book_concept; at most 4,096 Unicode characters."),
            ("at", "Optional returned LID to focus the structural view or route; omit for the whole-publication overview."),
            ("anchor_lid", "Optional known LID to anchor concept candidate ranking."),
            ("limit", "Maximum concept candidates (default 12, maximum 50); inspect matched_count and truncated before claiming complete recall."),
            ("granularity", "Context reach: near for adjacency/local edges, mid for other concept/entity occurrences, far for long-range edges; returns pointers, not source text."),
            ("k", "Optional maximum context pointers; omit to use the tool's default."),
            ("match_mode", "Literal matching mode, not semantic retrieval; use the declared enum."),
            ("scope", "Optional within_lid subtree and/or relative_to LID with before/after direction, using known LIDs from this publication."),
            ("order", "Order of literal occurrences; use returned next_calls to preserve search pagination."),
            ("cursor", "Opaque search cursor; copy from next_calls with the original search arguments."),
            ("page_size", "Maximum literal occurrences per search batch (default 20, maximum 50)."),
            ("mode", "Reading depth: skim, close or deep; default skim."),
            ("stage", "Reading stage: passive, active, critical or creative; default passive."),
        ] {
            if let Some(property) = schema["properties"].get_mut(key) {
                property["description"] = json!(description);
            }
        }
        schema["properties"]["published_book_ref"] = ref_schema.clone();
        if schema.get("required").is_none() {
            schema["required"] = json!([]);
        }
        schema["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("published_book_ref"));
        if id == BookToolId::Text {
            schema["properties"]["start_utf16"] = json!({"type":"integer","minimum":0,
                "description":"Absolute canonical-source UTF-16 position from next_calls; omit for the first page."});
        } else {
            schema["properties"]["page"] = json!({"type":"object","additionalProperties":false,
                "description":"Copy a continuation from next_calls to read an omitted collection. data_path identifies its position in the full projection.",
                "properties":{"path":{"type":"string"},"offset":{"type":"integer","minimum":0,"default":0},
                    "limit":{"type":"integer","minimum":1,"maximum":50,"default":20}},"required":["path"]});
        }
        let description = format!("{} Use when: {} {} {} {} {}",
            contract.description, contract.use_when, contract.do_not_use_when,
            if id == BookToolId::SearchText {
                "Hits are data.occurrences: pass a hit's start_lid as book_text.lid and end_lid as book_text.end_lid; source_range_utf16 is the exact whole-hit range and ranges gives per-LID positions."
            } else { "" },
            if id == BookToolId::Text {
                "Returns data.text with at most 8,000 Unicode characters per page, data.source_range for this page and data.requested_range for the requested LIDs. Ranges are absolute canonical-source UTF-16 [start, end), not byte offsets. Preserve the publication and range when continuing."
            } else {
                "Returns a built projection in data, not a generated answer. Collections are paged: data_path identifies a continuation's collection in the original projection and total_items its size. next_calls may include multiple collection branches and search batches. Concept matched_count/truncated still describe bounded candidate recall. Read chosen evidence LIDs with book_text. Missing built capability returns material_unavailable."
            },
            "Copy published_book_ref from book_list and check its available_tools. Costs 1 call on success, including empty results, each continuation page and repeated successful requests; failures cost 0. complete=false means more pages exist; copy needed next_calls name and arguments unchanged. sources gives published_book_ref, start_lid/end_lid, source_range and source_url: cite only evidence actually read, retaining its version and exact range. An empty sources array provides no source citation. usage is the server's post-call balance with charged_units and charged_from; do not infer balance locally. On quota_exhausted stop content retries and report usage.free_resets_at plus top_up_url or top_up_instruction; book_list and mcp_usage remain free.")
            .replace("book.text", "book_text");
        tools.push(
            json!({"name":contract.aliases.mcp,"description":description,"inputSchema":schema}),
        );
    }
    tools
}

pub struct McpRead<'a> {
    library: &'a Mutex<PublishedLibrary>,
    origin: url::Url,
}
pub(crate) struct PreparedRead {
    pub(crate) reference: PublishedBookRef,
    id: BookToolId,
    input: BookToolInput,
    start: Option<usize>,
    page: Option<CollectionPage>,
    name: String,
    arguments: Value,
}
impl<'a> McpRead<'a> {
    /// Use the configured site Origin, never a request-supplied host.
    pub fn new(library: &'a Mutex<PublishedLibrary>, site_origin: &str) -> Result<Self, ToolError> {
        let origin = url::Url::parse(site_origin).map_err(|_| invalid("Invalid site Origin"))?;
        if !matches!(origin.scheme(), "https" | "http")
            || origin.origin().ascii_serialization() != site_origin.trim_end_matches('/')
        {
            return Err(invalid("Use the configured site Origin"));
        }
        Ok(Self { library, origin })
    }

    /// `identity` must be freshly authenticated for this request by McpAccess.
    pub fn call(
        &self,
        identity: &McpIdentity,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        if name == "book_list" {
            return self.list(identity, arguments);
        }
        let prepared = self.prepare(identity, name, arguments)?;
        self.read_prepared(identity, prepared)
    }

    /// Validate and authorize without loading or computing content, before the quota check.
    pub(crate) fn prepare(
        &self,
        identity: &McpIdentity,
        name: &str,
        arguments: Value,
    ) -> Result<PreparedRead, ToolError> {
        let id = from_mcp_alias(name)
            .filter(|id| READ_TOOLS.contains(id))
            .ok_or_else(|| {
                error(
                    "MCP_TOOL_UNAVAILABLE",
                    "not_found",
                    "Use an available pure read tool from the remote tool list",
                )
            })?;
        let mut local = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| invalid("Tool arguments must be an object"))?;
        let reference = reference(
            local
                .remove("published_book_ref")
                .ok_or_else(|| invalid("published_book_ref is required"))?,
        )?;
        let start = if id == BookToolId::Text {
            local
                .remove("start_utf16")
                .map(|value| {
                    serde_json::from_value::<usize>(value)
                        .map_err(|_| invalid("start_utf16 must be a nonnegative integer"))
                })
                .transpose()?
        } else {
            None
        };
        let page = if id != BookToolId::Text {
            local
                .remove("page")
                .map(|value| {
                    serde_json::from_value::<CollectionPage>(value)
                        .map_err(|_| invalid("Invalid collection page"))
                })
                .transpose()?
        } else {
            None
        };
        if let Some(page) = &page {
            valid_limit(page.limit)?;
        }
        let input = validate_input(id, Value::Object(local))
            .map_err(|err| error(err.code, "validation", &err.message))?;
        self.library
            .lock()
            .unwrap()
            .authorize(identity.user_id(), &reference)
            .map_err(material_error)?;
        Ok(PreparedRead {
            reference,
            id,
            input,
            start,
            page,
            name: name.into(),
            arguments,
        })
    }

    pub(crate) fn read_prepared(
        &self,
        identity: &McpIdentity,
        prepared: PreparedRead,
    ) -> Result<Value, ToolError> {
        let PreparedRead {
            reference,
            id,
            input,
            start,
            page,
            name,
            arguments,
        } = prepared;
        // Release the shared cache lock before computing any tool projection.
        let publication = self
            .library
            .lock()
            .unwrap()
            .load(identity.user_id(), &reference)
            .map_err(material_error)?;
        let book = &publication.book;
        let mut next_calls = Vec::new();
        if let BookToolInput::Text(input) = input {
            let page = book.text_page(&input.lid, input.end_lid.as_deref(), start)?;
            if let Some(next) = page.next_start_utf16 {
                let mut args = arguments.clone();
                args["start_utf16"] = json!(next);
                next_calls.push(json!({"name":name,"arguments":args}));
            }
            let source = self.source(
                &reference,
                &input.lid,
                input.end_lid.as_deref().unwrap_or(&input.lid),
                &page.source_range,
            );
            return Ok(
                json!({"published_book_ref":reference,"data":page,"sources":[source],
                "complete":next_calls.is_empty(),"next_calls":next_calls}),
            );
        }
        let data = match (id, input) {
            (BookToolId::SearchText, BookToolInput::SearchText(input)) => {
                json!(book.search_text(&input)?)
            }
            (BookToolId::Context, BookToolInput::Context(input)) => {
                json!(book.context(&input.lid, input.granularity.map(|v| v.as_str()), input.k)?)
            }
            (BookToolId::Concept, BookToolInput::Concept(input)) => json!(
                book.concept_candidates(&input.query, input.anchor_lid.as_deref(), input.limit)?
            ),
            (BookToolId::Structure, BookToolInput::At(input)) => {
                json!(book.structure(input.at.as_deref())?)
            }
            (BookToolId::GuidePath, BookToolInput::At(input)) => {
                json!(book.guide_path(input.at.as_deref())?)
            }
            (BookToolId::PaperMetadata, BookToolInput::Empty(_)) => {
                json!(book.paper_metadata_projection())
            }
            (BookToolId::PaperLexicon, BookToolInput::Empty(_)) => {
                json!(book.paper_lexicon_projection())
            }
            (BookToolId::PaperReadingGuide, BookToolInput::PaperReadingGuide(input)) => {
                json!(book
                    .paper_reading_guide(Some(input.mode.as_str()), Some(input.stage.as_str()))?)
            }
            _ => unreachable!("validated pure read tool"),
        };
        if data.get("available") == Some(&Value::Bool(false)) {
            return Err(error(
                "MCP_CAPABILITY_UNAVAILABLE",
                "not_found",
                "This publication does not contain the requested built capability; check book_list",
            ));
        }
        let mut continuations = Vec::new();
        let (paged, data_path, total_items) = if let Some(page) = page {
            let items = data
                .pointer(&page.path)
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    invalid("page.path must select a collection returned by this tool")
                })?;
            if page.offset >= items.len() && !(page.offset == 0 && items.is_empty()) {
                return Err(invalid("page.offset is outside the collection"));
            }
            let result = page_array(
                items,
                &page.path,
                page.offset,
                page.limit,
                &mut continuations,
            );
            (result, page.path, Some(items.len()))
        } else {
            (
                bounded(&data, "", default_limit(), &mut continuations),
                String::new(),
                None,
            )
        };
        for continuation in continuations {
            let mut args = arguments.clone();
            args["page"] = json!(continuation);
            next_calls.push(json!({"name":name,"arguments":args}));
        }
        // Search has its own occurrence cursor; retain its native semantics alongside collection pages.
        if data_path.is_empty() {
            if let Some(cursor) = data.get("next_cursor").and_then(Value::as_str) {
                let mut args = arguments.clone();
                args.as_object_mut().unwrap().remove("page");
                args["cursor"] = json!(cursor);
                next_calls.push(json!({"name":name,"arguments":args}));
            }
        }
        let mut lids = BTreeSet::new();
        collect_lids(&paged, &data_path, &mut lids);
        // A nested page such as /references/value still has its enclosing field's provenance.
        let mut parent_path = data_path.as_str();
        while let Some((parent, _)) = parent_path.rsplit_once('/') {
            if let Some(fields) = data.pointer(parent).and_then(Value::as_object) {
                for key in ["lid", "evidence_lids", "source_lid", "anchor", "at"] {
                    if let Some(value) = fields.get(key) {
                        collect_lids(value, key, &mut lids);
                    }
                }
            }
            parent_path = parent;
        }
        let sources: Vec<_> = book
            .base
            .lid_nodes
            .iter()
            .filter(|node| lids.contains(&node.lid))
            .map(|node| self.source(&reference, &node.lid, &node.lid, &node.span))
            .collect();
        Ok(
            json!({"published_book_ref":reference,"data":paged,"data_path":data_path,
            "total_items":total_items,"sources":sources,"complete":next_calls.is_empty(),"next_calls":next_calls}),
        )
    }

    fn list(&self, identity: &McpIdentity, arguments: Value) -> Result<Value, ToolError> {
        let input: ListInput = serde_json::from_value(arguments)
            .map_err(|_| invalid("Use after and limit for book_list"))?;
        valid_limit(input.limit)?;
        let after = input.after.map(reference).transpose()?;
        let library = self.library.lock().unwrap();
        let refs = library
            .list_manifests_page(identity.user_id(), after.as_ref(), input.limit + 1)
            .map_err(material_error)?;
        let complete = refs.len() <= input.limit;
        let mut items = Vec::new();
        for (manifest, is_default, name) in refs.iter().take(input.limit) {
            items.push(
                json!({"published_book_ref":manifest.reference,"name":name,"is_default":is_default,
                "available_tools":available_tools(manifest)}),
            );
        }
        let next_calls = if complete {
            vec![]
        } else {
            vec![
                json!({"name":"book_list","arguments":{"after":refs[input.limit - 1].0.reference,"limit":input.limit}}),
            ]
        };
        Ok(json!({"items":items,"complete":complete,"next_calls":next_calls}))
    }

    fn source(
        &self,
        reference: &PublishedBookRef,
        lid: &str,
        end_lid: &str,
        range: &base_schema::Span,
    ) -> Value {
        let mut url = self.origin.clone();
        url.set_path("/");
        url.query_pairs_mut()
            .append_pair("book_id", &reference.book_id)
            .append_pair("publication_id", &reference.publication_id)
            .append_pair("start_lid", lid)
            .append_pair("end_lid", end_lid)
            .append_pair("start_utf16", &range.start.to_string())
            .append_pair("end_utf16", &range.end.to_string());
        json!({"published_book_ref":reference,"start_lid":lid,"end_lid":end_lid,
            "source_range":range,"source_url":url.as_str()})
    }
}

fn available_tools(manifest: &PublicationManifest) -> Vec<&'static str> {
    READ_TOOLS
        .iter()
        .copied()
        .filter(|id| match id {
            BookToolId::Structure | BookToolId::GuidePath => {
                manifest.files.contains_key("book_structure.json")
            }
            BookToolId::PaperMetadata => manifest.files.contains_key("paper_metadata.json"),
            BookToolId::PaperLexicon => manifest.files.contains_key("paper_lexicon.json"),
            BookToolId::PaperReadingGuide => {
                manifest.files.contains_key("paper_metadata.json")
                    || manifest.files.contains_key("paper_lexicon.json")
                    || manifest
                        .artifact_versions
                        .get("book_structure.json")
                        .is_some_and(|s| s["header"]["profile_id"] == "paper")
            }
            _ => true,
        })
        .map(|id| contract_for(id).aliases.mcp.unwrap())
        .collect()
}

fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
fn bounded(value: &Value, path: &str, limit: usize, next: &mut Vec<CollectionPage>) -> Value {
    match value {
        Value::Array(items) => page_array(items, path, 0, limit, next),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        bounded(value, &format!("{path}/{}", pointer_key(key)), limit, next),
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn page_array(
    items: &[Value],
    path: &str,
    offset: usize,
    limit: usize,
    next: &mut Vec<CollectionPage>,
) -> Value {
    let end = offset.saturating_add(limit).min(items.len());
    if end < items.len() {
        next.push(CollectionPage {
            path: path.into(),
            offset: end,
            limit,
        });
    }
    Value::Array(
        items[offset..end]
            .iter()
            .enumerate()
            .map(|(i, value)| bounded(value, &format!("{path}/{}", offset + i), limit, next))
            .collect(),
    )
}
fn collect_lids(value: &Value, path: &str, out: &mut BTreeSet<String>) {
    let key = path.rsplit('/').next().unwrap_or("");
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                collect_lids(value, key, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_lids(value, key, out);
            }
        }
        Value::String(lid)
            if key == "lid"
                || key.ends_with("_lid")
                || key == "anchor"
                || key == "at"
                || key == "lids"
                || key.ends_with("_lids")
                || key == "occurrences" =>
        {
            out.insert(lid.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "mcp_read_tests.rs"]
mod tests;
