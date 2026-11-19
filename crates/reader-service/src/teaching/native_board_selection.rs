//! Native board selection reuses accepted metadata; semantic choices stay explicit.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateReference {
    candidate_id: String,
    candidate_revision: u32,
}

pub(super) fn bind(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    bind: &TeachingEvent,
    request: &Value,
    evidence: &[SourceBinding],
) -> Result<Value, ToolError> {
    let selected: CandidateReference = serde_json::from_value(request["native_board_selection"].clone())
        .map_err(|error| invalid(format!("画板候选引用无效：{error}")))?;
    let saved = state.user.agent_history.sessions.iter().find(|session|session.id==turn.session_id)
        .and_then(|session|session.turns.iter().find(|saved|saved.turn_id==turn.turn_id))
        .filter(|saved|saved.status==AgentAssistantStatus::PendingAssistant && saved.domain.board_context.is_some())
        .ok_or_else(||invalid("画板教学选择需要当前待完成回合的真实画板上下文"))?;
    let candidate = saved.domain.board_authoring.as_ref()
        .filter(|candidate|candidate.candidate_id==selected.candidate_id && candidate.candidate_revision==selected.candidate_revision)
        .ok_or_else(||invalid("画板候选已改变，请读取、预览并检查当前候选"))?;
    if !candidate.previewed || saved.domain.board_commits.iter().any(|part|part.candidate_revision==candidate.candidate_revision) {
        return Err(invalid("画板教学选择需要尚未提交且已经预览的当前候选"));
    }
    if request["move"]["delivery_text"].as_str()!=Some(candidate.delivery_text.as_str()) {
        return Err(invalid("画板教学正文必须由当前候选绑定，不能另写一份正文"));
    }
    let accepted = design::current_design(state,&bind.binding)?;
    let design_ref = accepted["design_ref"].as_str().ok_or_else(||invalid("画板教学选择需要已接纳的当前设计"))?;
    if request["move"]["design_ref"].as_str().is_some_and(|sampled|sampled!=design_ref) {
        return Err(invalid("教学动作必须采用当前设计修订；请先读取并消费新的设计"));
    }
    let design: memory::teaching_design::TeachingDesign = serde_json::from_value(accepted["design"].clone())
        .map_err(|error|invalid(error.to_string()))?;
    let target = inherited_selection_target(state,&bind.binding,Some(design_ref),&design.focus_block_refs,&design,evidence)?;
    let mut normalized = request.clone();
    let movement = normalized["move"].as_object_mut().ok_or_else(||invalid("画板教学选择需要 move 对象"))?;
    movement.insert("move_id".into(),json!(format!("native:{}:{design_ref}",candidate.candidate_id)));
    movement.insert("design_ref".into(),json!(design_ref));
    movement.insert("block_refs".into(),json!(design.focus_block_refs));
    movement.insert("capability".into(),json!(target.capability));
    movement.insert("object_ids".into(),json!(target.object_refs.iter().map(|object|&object.object_id).collect::<Vec<_>>()));
    movement.insert("prompt".into(),json!(target.learning_focus));
    movement.insert("target".into(),json!(target));
    let mut kind = bind.payload["current_action"]["action"].get("move_kind").and_then(Value::as_str).unwrap_or("explain").to_owned();
    bind_intent(movement,&bind.payload,&kind)?;
    if movement["interaction_intent"]=="direct_explanation" { kind="explain".into(); }
    movement.insert("kind".into(),json!(kind));
    if movement.get("participation").is_none_or(Value::is_null) {
        let support = design.blocks.iter().filter(|block|design.focus_block_refs.contains(&block.block_id))
            .map(|block|block.relation.as_str()).collect::<Vec<_>>().join("\n");
        movement.insert("participation".into(),json!({"choice":"support",
            "reason":"发布当前已检查画板的完整图文支持，不新增作答任务。","support":support,"learner_inference":null}));
    }
    if let Some(expectation)=movement.get_mut("response_expectation").filter(|value|value.is_object()) {
        expectation["block_refs"]=json!(design.focus_block_refs);
    }
    normalized.as_object_mut().unwrap().remove("native_board_selection");
    Ok(normalized)
}

fn bind_intent(movement:&mut serde_json::Map<String,Value>,context:&Value,kind:&str) -> Result<(),ToolError> {
    movement.remove("intent_quote");
    if let Some((intent,origin))=runtime::tutor::action_intent(context) {
        if movement.get("interaction_intent").is_some() {
            return Err(invalid("冻结的当前操作已经确定 interaction_intent，请省略重复的方式字段"));
        }
        movement.insert("interaction_intent".into(),json!(intent));
        movement.insert("intent_origin".into(),json!(origin));
        movement.remove("preference_ref");
        return Ok(());
    }
    let intent=movement.get("interaction_intent").and_then(Value::as_str)
        .filter(|intent|["direct_explanation","guided_inquiry","neutral"].contains(intent))
        .ok_or_else(||invalid("当前原话的教学方式尚未确定，请提供 interaction_intent：direct_explanation、guided_inquiry 或 neutral；不能默认 neutral"))?.to_owned();
    // Choosing which natural-language preference applies remains a semantic
    // decision. Its actual fact reference is checked by validate_move_intent.
    if movement.get("preference_ref").is_some_and(|reference|!reference.is_null()) {
        movement.insert("intent_origin".into(),json!("confirmed_preference"));
        return Ok(());
    }
    let current_request=context["current_request"].as_str().unwrap_or("");
    let default=context["default_teaching_intent"].as_str();
    let origin=if default==Some(intent.as_str()) {
        "session_default"
    } else if !current_request.trim().is_empty() && intent!="neutral" {
        movement.insert("intent_quote".into(),json!(current_request));
        "current_request"
    } else if default.is_some() {
        // Retain the normal validator's exact default-mode conflict error.
        "session_default"
    } else if serde_json::from_value::<runtime::tutor_continuation::ResolvedInteraction>(context["current_action"].clone())
        .is_ok_and(|action|action.validates_current_action(&intent,if intent=="direct_explanation" {"explain"} else {kind})) {
        "current_action"
    } else if intent=="neutral" {
        "neutral"
    } else {
        return Err(invalid("所选 interaction_intent 没有当前原话、已确认偏好、会话默认或冻结操作作为来源"));
    };
    movement.insert("intent_origin".into(),json!(origin));
    Ok(())
}
