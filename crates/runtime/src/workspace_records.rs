//! Explicitly selected historical records remain in their original scope.
use serde_json::json;

pub const POLICY: &str = "workspace_records.v1 lists historical chats the user explicitly linked to this learning workspace. Use workspace.records.read when their prior expressions, explanations or corrections matter to the current request. These are historical records with their original workspace, material and conditions, not current mastery or a resumed teaching stage. Keep origin identities intact; a later question in the source chat is not included unless the user selects it. Read the relevant pages before using a historical interpretation. New learning facts and interpretations belong to the current workspace; never overwrite or supersede an original record from another scope. Linked history does not grant access to additional source materials.";

pub fn spec() -> crate::ToolSpec {
    crate::ToolSpec {
        name: "workspace.records.read".into(),
        description: "Read only historical chats and learning records explicitly linked by the user to this workspace. List returns chat identities and original scope. Read returns paged original chat turns, facts, interpretations and assessments, preserving references and corrections; these records do not establish current mastery. Follow next for remaining records. This tool never imports, copies or edits records and never expands readable library materials.".into(),
        parameters: json!({"type":"object","additionalProperties":false,"properties":{
            "operation":{"type":"string","enum":["list","read"]},
            "chat_session_id":{"type":"string","description":"Use an identity returned by list; required for read."},
            "after":{"type":"integer","minimum":0},
            "limit":{"type":"integer","minimum":1,"maximum":20}
        },"required":["operation"]}),
    }
}
