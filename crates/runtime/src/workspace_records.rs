//! Search original conversation and learning records without adopting mastery.
use serde_json::json;

pub const POLICY: &str = "workspace_records.v1 lists this learning workspace's own conversations across its materials and historical chats the user explicitly linked. Without a learning workspace it stays within the current book. Use workspace.records.read search to find prior expressions, explanations, source excerpts and corrections relevant to the current topic; search covers the full permitted history before paging. Follow next_request for remaining results and use read_request to inspect an original match. These are historical records with their original workspace, material, event positions and conditions, not current mastery or a resumed teaching stage. Keep learner facts separate from interpretations, retain supersedes/corrections, and cite original identities when arranging a progression on a board. A later question in an externally linked source chat is not included unless the user selects it. New learning facts and interpretations belong to the current workspace; never overwrite or supersede an original record from another scope. Linked history does not grant access to additional source materials.";

pub fn spec() -> crate::ToolSpec {
    crate::ToolSpec {
        name: "workspace.records.read".into(),
        description: "List, search or read original conversations and learning records in this learning workspace, across its materials, plus explicitly linked historical selections. Without a learning workspace only current-book chats are available. Search matches full original text before pagination, including older turns, facts and interpretation revisions. Read preserves original identities, event positions, times, sources and corrections. Follow next_request; read_request opens a match. These records do not establish current mastery or expand readable library materials.".into(),
        parameters: json!({"type":"object","additionalProperties":false,"properties":{
            "operation":{"type":"string","enum":["list","search","read"]},
            "chat_session_id":{"type":"string","description":"Identity from list/search; required for read, optional to limit search to one permitted chat."},
            "query":{"type":"string","minLength":1,"maxLength":512,"description":"Case-insensitive literal text to search across all permitted history; required for search, optional for read."},
            "turn_id":{"type":"string","description":"Optional original turn to read, including its learning records."},
            "record_id":{"type":"string","description":"Optional exact record identity returned by search/read."},
            "through_seq":{"type":"integer","minimum":1,"description":"Optional native chat snapshot position from an earlier result; cannot expand a linked selection."},
            "after":{"type":"integer","minimum":0},
            "limit":{"type":"integer","minimum":1,"maximum":20}
        },"required":["operation"]}),
    }
}
