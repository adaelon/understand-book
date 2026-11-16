//! Teacher-owned correction memory. This path never interprets learner ability.
use super::*;
use memory::teacher_corrections::TeacherCorrection;

pub(super) fn step(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    request: &Value,
    ranges: &[EvidenceRange],
    now: &str,
) -> Result<Value, ToolError> {
    check_activity_source(state, binding)?;
    let mut store = state
        .user
        .learning_store_for_workspace(state.learning_workspace_id)?;
    let operation = || {
        request["operation_id"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid("教师纠错写入缺少 operation_id"))
    };
    match request["correction_action"].as_str() {
        Some("search") => {
            let queries: Vec<String> = serde_json::from_value(request["queries"].clone())
                .map_err(|e| invalid(e.to_string()))?;
            store.search_teacher_corrections(
                binding,
                &queries,
                request["after"].as_u64().unwrap_or(0) as usize,
            )
        }
        Some("record") => {
            let correction: TeacherCorrection =
                serde_json::from_value(request["correction"].clone())
                    .map_err(|e| invalid(e.to_string()))?;
            for source in &correction.source_quotes {
                require_target_read_this_turn(state, &source.lid, &source.lid, ranges)?;
                if source.quote.trim().is_empty()
                    || !state
                        .book
                        .text(&source.lid, Some(&source.lid))?
                        .contains(&source.quote)
                {
                    return Err(invalid("教师纠错依据必须逐字来自本轮已读的对应原文"));
                }
            }
            let original_request = state
                .user
                .agent_history
                .sessions
                .iter()
                .find(|s| s.id == binding.chat_session_id)
                .and_then(|s| s.turns.iter().find(|t| t.turn_id == binding.turn_id))
                .map(|t| t.user_authored_text())
                .unwrap_or_default();
            let event = store.record_teacher_correction(
                binding,
                operation()?,
                correction,
                &original_request,
                now,
            )?;
            Ok(
                json!({"correction_ref":event.event_id,"availability":"after_completed_delivery",
                "meaning":"Saved teacher correction; it becomes recallable only after this turn actually delivers a complete answer. This is not learner evidence."}),
            )
        }
        Some("read") => store.read_teacher_correction(
            binding,
            request["correction_ref"].as_str().unwrap_or_default(),
            request["after"].as_u64().unwrap_or(0) as usize,
        ),
        Some("use") => {
            let event = store.use_teacher_correction(
                binding,
                operation()?,
                request["correction_ref"].as_str().unwrap_or_default(),
                request["decision"].as_str().unwrap_or_default(),
                request["reason"].as_str().unwrap_or_default(),
                now,
            )?;
            Ok(
                json!({"use_ref":event.event_id,"correction_ref":event.payload["correction_ref"],"decision":event.payload["decision"]}),
            )
        }
        _ => Err(invalid(
            "correction_action 必须为 search、read、record 或 use",
        )),
    }
}
