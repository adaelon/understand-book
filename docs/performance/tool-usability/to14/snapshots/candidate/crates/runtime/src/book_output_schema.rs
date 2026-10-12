//! Runtime-owned complete results, before model-body budgeting or MCP wrapping.
use book_tool_contracts::BookToolId;
use read_tools::output_schema::schema_for;
use serde_json::Value;

pub fn canonical(id: BookToolId) -> Option<Value> {
    match id {
        BookToolId::Query => Some(schema_for::<crate::QueryOutcome>()),
        BookToolId::Synthesize => Some(schema_for::<crate::SynthesizeResponse>()),
        BookToolId::Guide => Some(schema_for::<crate::BookGuideResponse>()),
        _ => read_tools::output_schema::book_output_schema(id),
    }
}
