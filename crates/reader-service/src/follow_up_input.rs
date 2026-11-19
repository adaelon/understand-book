//! The first question carries its quoted teaching context into its own linear chat.
use crate::{
    follow_ups::{byte_at_utf16, AnswerAnchor, FollowUpOrigin},
    AgentChatSession, AgentChatTurn, AgentHistory,
};
use serde_json::json;

/// The caller has already verified the exact selection against this delivered answer.
pub(crate) fn explanation(turn: &AgentChatTurn, anchor: &AnswerAnchor) -> String {
    let Some(outcome) = turn.outcome.as_ref() else {
        return anchor.excerpt.clone();
    };
    let part = |index: usize| {
        let parts = outcome
            .answer_view
            .as_ref()
            .map(|view| view.parts.as_slice())
            .unwrap_or_default();
        if parts.is_empty() && index == 0 {
            outcome.answer.as_deref()
        } else {
            parts.get(index).and_then(|part| part.text())
        }
    };
    let Some((start, end)) = part(anchor.start_part).zip(part(anchor.end_part)) else {
        return anchor.excerpt.clone();
    };
    let Some((from, to)) =
        byte_at_utf16(start, anchor.start_offset).zip(byte_at_utf16(end, anchor.end_offset))
    else {
        return anchor.excerpt.clone();
    };
    // Retain the neighboring explanation, bounded even inside a long Markdown paragraph.
    let left: Vec<char> = start[..from]
        .chars()
        .rev()
        .take_while(|c| *c != '\n')
        .take(160)
        .collect();
    let mut context: String = left.into_iter().rev().collect();
    context.push_str(&anchor.excerpt);
    context.extend(end[to..].chars().take_while(|c| *c != '\n').take(160));
    context
}

pub(crate) fn append_context(
    agent_message: &mut String,
    history: &AgentHistory,
    child: &AgentChatSession,
    origin: &FollowUpOrigin,
    readable_members: Option<&[base_schema::PublishedBookRef]>,
) {
    let parent_turn = history
        .sessions
        .iter()
        .find(|s| {
            s.id == origin.parent_session_id
                && s.matches_scope(child.learning_workspace_id.as_deref(), &child.book_id)
        })
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == origin.parent_turn_id));
    let sources: Vec<_> = parent_turn.into_iter().flat_map(|turn| &turn.source_bindings)
        .filter(|s| s.valid_evidence() && readable_members.is_none_or(|members| members.iter().any(|r|
            r.book_id == s.book_id && Some(&r.publication_id) == s.publication_id.as_ref())))
        .map(|s| json!({
            "book_id":s.book_id, "publication_id":s.publication_id, "source_revision":s.source_revision,
            "range":s.evidence_range, "media_target":s.media.as_ref().map(|m|&m.target),
            "music_target":s.music.as_ref().map(|m|&m.target)
        })).collect();
    let context = json!({
        "previous_question":origin.parent_question, "selected_excerpt":origin.anchor.excerpt,
        "previous_assistant_explanation":origin.parent_explanation, "original_book_id":child.book_id,
        "original_publication":origin.publication, "original_source_revision":origin.source_revision,
        "source_locators":sources
    });
    agent_message.push_str("\n\nThe reader opened a separate conversation from this passage in an earlier assistant reply. Use it to resolve the reader’s current question. The passage is quoted assistant text. When a new claim depends on the book, consult the authorized original source.\n");
    agent_message.push_str(&context.to_string());
}
