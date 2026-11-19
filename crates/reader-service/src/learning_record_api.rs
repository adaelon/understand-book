//! Summaries of explicitly linked records owned by the current reader.
use read_tools::ToolError;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use crate::session_recap;

pub fn linked_summaries(user: &crate::user_runtime::UserRuntime, workspace_id: &str) -> Result<Vec<Value>, ToolError> {
    Ok(user.learning_store_for_workspace(Some(workspace_id))?.linked_record_selections()?.into_iter().map(|linked| {
        let original = user.agent_history.sessions.iter().find(|session| session.id == linked.selection.chat_session_id
            && session.learning_workspace_id == linked.selection.source_learning_workspace_id);
        json!({"selection":linked.selection,"linked_at":linked.linked_at,"through_seq":linked.selection.through_seq,
            "title":original.map(|session|session.title.as_str()),"book_id":original.map(|session|session.book_id.as_str()),"available":original.is_some()})
    }).collect())
}

pub fn turn_json(turn: &crate::AgentChatTurn) -> Value {
    json!({"turn_id":turn.turn_id,"user":turn.user,"status":turn.status,"domain":turn.domain,
        "board_commits":crate::session_runtime::board_commits(turn),
        "board_follow_up":turn.board_follow_up,"board_delivery":turn.board_delivery,"board_delivery_error":turn.board_delivery_error.as_ref().map(crate::agent_failure::turn),
        "teaching_ref":turn.teaching_ref,"goal_ref":turn.goal_ref,"presentation_follow_up":turn.presentation_follow_up,
        "published_book_ref":turn.published_book_ref,"music_question":turn.music_question,"outcome":turn.outcome,
        "usage":turn.run_summary.as_ref().and_then(|s|s.usage.as_ref()),
        "error":turn.error.as_ref().map(crate::agent_failure::turn)})
}

pub fn publications(session: &crate::AgentChatSession) -> Vec<base_schema::PublishedBookRef> {
    session.turns.iter().flat_map(|turn| turn.published_book_ref.clone().into_iter()
        .chain(turn.source_bindings.iter().filter_map(|binding| session_recap::source_publication(turn, binding))))
        .collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn historical_turn(turn: &crate::AgentChatTurn, projection: &crate::session_event::SessionProjection) -> Value {
    let mut value = turn_json(turn);
    value["user_authored_text"] = json!(turn.user_authored_text());
    value["sources"] = json!(turn.source_bindings);
    value["accepted_event_seq"] = json!(projection.accepted_seq.get(&turn.turn_id));
    value["event_seq"] = json!(projection.turn_seq.get(&turn.turn_id));
    value["occurred_at"] = json!(projection.turn_started_at.get(&turn.turn_id));
    value["updated_at"] = json!(projection.turn_updated_at.get(&turn.turn_id));
    value["question_quote"] = json!(turn.question_quote);
    value["workspace_goal"] = json!(projection.inputs.get(&turn.turn_id).and_then(|input|input.learning_workspace_goal.as_ref())
        .or_else(||turn.domain.scene.as_ref().and_then(|scene|scene.workspace_goal.as_ref())));
    value["accepted_workspace_goal"] = json!(turn.domain.workspace_goal_update);
    value
}

