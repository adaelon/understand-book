//! Model reads of explicitly linked history; original scope and facts stay intact.
use crate::{authorization, learning_record_api, session_recap, user_runtime::UserRuntime};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn read(user: &UserRuntime, workspace: &str, input: Value) -> Result<Value, ToolError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        operation: String,
        chat_session_id: Option<String>,
        #[serde(default)]
        after: usize,
        limit: Option<usize>,
    }
    let invalid = || memory::teaching::invalid("请选择已带入的原记录及有效分页范围");
    let request: Request = serde_json::from_value(input).map_err(|_| invalid())?;
    let limit = request.limit.unwrap_or(8);
    if !(1..=20).contains(&limit) { return Err(invalid()); }
    let (selection, items) = match request.operation.as_str() {
        "list" => (Value::Null, learning_record_api::linked_summaries(user, workspace)?),
        "read" => {
            let store = user.learning_store_for_workspace(Some(workspace))?;
            let id = request.chat_session_id.as_deref().ok_or_else(invalid)?;
            let selected = store.linked_record_selections()?.into_iter()
                .find(|linked| linked.selection.chat_session_id == id).ok_or_else(authorization::missing)?;
            let history = session_recap::snapshot(user, id, Some(selected.selection.through_seq))?;
            let session = history.session.as_ref().ok_or_else(authorization::missing)?;
            if session.learning_workspace_id != selected.selection.source_learning_workspace_id { return Err(authorization::missing()); }
            let bundle = store.linked_records(&selected.selection)?;
            let mut items = Vec::new();
            for turn in session.turns.iter().filter(|turn| selected.selection.turn_ids.contains(&turn.turn_id)) {
                let original = learning_record_api::historical_turn(turn, &history);
                items.push(json!({"kind":"chat_turn","turn_id":turn.turn_id,"published_book_ref":turn.published_book_ref,
                    "user":turn.user_authored_text(),"answer":turn.outcome.as_ref().map(|outcome|&outcome.answer),
                    "question_quote":original["question_quote"],"workspace_goal":original["workspace_goal"],
                    "accepted_workspace_goal":original["accepted_workspace_goal"],
                    "sources":turn.source_bindings,"status":turn.status}));
            }
            items.extend(bundle.facts.into_iter().map(|fact| json!({"kind":"fact","record":fact})));
            items.extend(bundle.evidence.into_iter().map(|evidence| json!({"kind":"interpretation","record":evidence})));
            items.extend(bundle.assessments.into_iter().map(|assessment| json!({"kind":"assessment","record":assessment})));
            (json!(selected.selection), items)
        },
        _ => return Err(invalid()),
    };
    let total = items.len();
    let rows: Vec<_> = items.into_iter().skip(request.after).take(limit).collect();
    let end = request.after.saturating_add(rows.len());
    Ok(json!({"learning_workspace_id":workspace,"selection":selection,"records":rows,"total":total,
        "next":(end < total).then_some(end),"historical":true,"current_mastery":false}))
}
