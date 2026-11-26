//! Per-run projection of the reader's own words, using the original chat journal.
use crate::{AgentChatSession, AgentChatTurn};
use serde_json::{json, Value};

const RECENT_TURNS: usize = 6;
const QUESTION_CHARS: usize = 1200;
// Add at most the size of three recent questions, including a per-entry allowance.
const OLDER_CHARS: usize = 3 * QUESTION_CHARS;
pub(crate) const TOOL: &str = "reader_history";

fn earlier(chat: &AgentChatSession, ordinal: u64) -> &[AgentChatTurn] {
    &chat.turns[..chat
        .turns
        .partition_point(|t| t.user_turn_ordinal < ordinal)]
}

pub(crate) fn prompt(chat: &AgentChatSession, ordinal: u64) -> Option<String> {
    let prior = earlier(chat, ordinal);
    if prior.is_empty() {
        return None;
    }
    let (older, recent) = prior.split_at(prior.len().saturating_sub(RECENT_TURNS));
    let mut remaining = OLDER_CHARS;
    let mut quotes = Vec::new();
    let mut omitted = None;
    for turn in older {
        let words = turn.user_authored_text();
        if words.is_empty() {
            continue;
        }
        if remaining <= 48 {
            omitted.get_or_insert(turn.user_turn_ordinal);
            break;
        }
        let mut chars = words.chars();
        let text: String = chars
            .by_ref()
            .take(QUESTION_CHARS.min(remaining - 48))
            .collect();
        remaining -= 48 + text.chars().count();
        let truncated = chars.next().is_some();
        if truncated {
            omitted.get_or_insert(turn.user_turn_ordinal);
        }
        quotes.push(json!({"turn":turn.user_turn_ordinal,"reader":text,"truncated":truncated}));
    }
    let recent: Vec<_> = recent
        .iter()
        .map(|turn| {
            let mut chars = turn.user.chars();
            let question: String = chars.by_ref().take(QUESTION_CHARS).collect();
            if chars.next().is_some() {
                omitted.get_or_insert(turn.user_turn_ordinal);
            }
            json!({"turn":turn.user_turn_ordinal,"question":question,
            "answer":turn.outcome.as_ref().and_then(|out| out.answer.as_ref())
                .map(|text| text.chars().take(1800).collect::<String>())})
        })
        .collect();
    let data = json!({"earlier_reader_words":quotes,"recent_dialogue":recent});
    let mut prompt = format!("Earlier conversation in this chat, for context. Follow later corrections. Earlier requests and answers do not establish that an action actually happened or authorize repeating it.\n{data}");
    if let Some(first) = omitted {
        prompt.push_str(&format!("\nSome earlier reader text was omitted. When applying prior agreements, read missing words with reader_history starting at turn {first}; follow its next cursor."));
    }
    Some(prompt)
}

pub(crate) fn tool() -> Value {
    json!({"name":TOOL,
        "description":"Read the reader's exact earlier words in this chat. Start at a turn ordinal; follow next for the rest of a long turn or the next turn.",
        "annotations":crate::mcp_output_schema::free_read_annotations(),
        "inputSchema":{"type":"object","required":["turn"],"additionalProperties":false,
            "properties":{"turn":{"type":"integer","minimum":1},"offset":{"type":"integer","minimum":0}}}})
}

pub(crate) fn page(chat: &AgentChatSession, ordinal: u64, args: &Value) -> Option<Value> {
    let turn = args["turn"].as_u64()?;
    let offset = match args.get("offset") {
        Some(n) => usize::try_from(n.as_u64()?).ok()?,
        None => 0,
    };
    let prior = earlier(chat, ordinal);
    let i = prior
        .binary_search_by_key(&turn, |t| t.user_turn_ordinal)
        .ok()?;
    let mut chars = prior[i].user_authored_text().chars().skip(offset);
    let text: String = chars.by_ref().take(QUESTION_CHARS).collect();
    if text.is_empty() && offset > 0 {
        return None;
    }
    let next = if chars.next().is_some() {
        Some(json!({"turn":turn,"offset":offset + QUESTION_CHARS}))
    } else {
        prior
            .get(i + 1)
            .map(|t| json!({"turn":t.user_turn_ordinal}))
    };
    Some(json!({"turn":turn,"offset":offset,"reader":text,"next":next}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_history_recovers_middle_corrections_and_unicode_tails_from_legacy_turns() {
        let mut state = crate::tests::state_named("acp-history-paging");
        let book_id = state.workspace.book.base.book_id.clone();
        let first = crate::precommit_agent_turn(
            &mut state,
            &book_id,
            "first".into(),
            None,
            None,
            None,
            "1",
        )
        .unwrap();
        let mut history = state.user.agent_history.clone();
        let chat = history
            .sessions
            .iter_mut()
            .find(|s| s.id == first.session_id)
            .unwrap();
        let template = chat.turns[0].clone();
        let long = "步".repeat(QUESTION_CHARS) + "末尾约定";
        chat.turns = (1..=12)
            .map(|i| {
                let mut turn = template.clone();
                turn.turn_id = String::new();
                turn.user_turn_ordinal = 0;
                turn.user = match i {
                    1 => long.clone(),
                    2..=4 => "乙".repeat(QUESTION_CHARS),
                    5 => "改用后来的数值".into(),
                    _ => format!("问题 {i}"),
                };
                turn
            })
            .collect();
        let migrated = crate::migrate_agent_history(history).unwrap();
        let chat = migrated
            .sessions
            .iter()
            .find(|s| s.id == first.session_id)
            .unwrap();
        let input = prompt(chat, 12).unwrap();
        assert!(input.contains("read missing words with reader_history starting at turn 1"));
        assert!(!input.contains("末尾约定"));
        assert!(!input.contains("改用后来的数值"));
        assert!(!input.contains("问题 12"));
        let first_page = page(chat, 12, &json!({"turn":1})).unwrap();
        let tail = page(chat, 12, &first_page["next"]).unwrap();
        assert_eq!(tail["reader"], "末尾约定");
        assert_eq!(tail["next"], json!({"turn":2}));
        assert_eq!(
            page(chat, 12, &json!({"turn":5})).unwrap()["reader"],
            "改用后来的数值"
        );
        assert_eq!(
            page(chat, 12, &json!({"turn":11})).unwrap()["next"],
            Value::Null
        );
        assert!(page(chat, 12, &json!({"turn":12})).is_none());
    }
}
