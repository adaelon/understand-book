//! Private receipts for an open explanation, its verified comparison and actual display.
use super::*;
use runtime::teach_back::{TeachBackComparison, TeachBackReplyKind, TeachBackStatus};
use runtime::tutor_continuation::{EntryAction, InteractionAction, ResolvedInteraction};

pub(super) fn is_entry(action: &ResolvedInteraction) -> bool {
    matches!(
        action.action,
        InteractionAction::UiEntry {
            entry_action: EntryAction::ExplainBack
        }
    )
}

pub(super) fn comparison(outcome: &OuterOutcome) -> Option<&TeachBackComparison> {
    outcome
        .answer_view
        .as_ref()?
        .parts
        .iter()
        .find_map(|part| match part {
            AgentAnswerPart::TeachBackComparison { comparison } => Some(comparison),
            _ => None,
        })
}

pub(super) fn freeze_support(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    action: &ResolvedInteraction,
    facts: &mut Vec<TeachingEvent>,
    request: &Value,
    now: &str,
) -> Result<(), ToolError> {
    let original = origin_ref(action)?;
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let mut support = if let Some(course) = &action.course {
        let mut refs: Vec<String> = request["course_context"]["prior_help"].as_array().into_iter().flatten()
            .filter_map(|event|event["event_id"].as_str().map(str::to_owned)).collect();
        if !refs.contains(&course.delivery_ref) { refs.push(course.delivery_ref.clone()); }
        for id in store.course_chat_support(&course.run_id, &binding.chat_session_id)? {
            if !refs.contains(&id) { refs.push(id); }
        }
        refs
    } else {
        vec![ensure_answer_support(state, binding, facts, original, now)?]
    };
    let input = facts
        .iter_mut()
        .find(|fact| {
            fact.binding.turn_id == binding.turn_id
                && fact.payload["usage"]["kind"] == "reading_action"
        })
        .ok_or_else(|| invalid("缺少本次开放表达的输入记录"))?;
    if let Some(reply) = &action.teach_back_reply {
        let parent = store.teaching_event(&reply.round_id)?;
        let first = parent.payload["comparison"]["continuation"]["first_round_id"]
            .as_str().unwrap_or(&parent.event_id);
        input.payload["first_round_id"] = json!(first);
        if reply.kind == TeachBackReplyKind::DisputeInterpretation {
            // Reinterpreting the first submission preserves its original assistance,
            // even when the reader opened the correction from a later revision.
            let first_evidence = store.evidence(&format!("interpretation:{first}"))?;
            input.payload["support_delivery_refs"] = json!(first_evidence.support_delivery_refs);
            return Ok(());
        }
    }
    support.extend(store.displayed_teach_backs(binding, original)?);
    input.payload["support_delivery_refs"] = json!(support);
    Ok(())
}

fn origin_ref(action: &ResolvedInteraction) -> Result<&str, ToolError> {
    action.course.as_ref().map(|c|c.delivery_ref.as_str())
        .or_else(||action.answer_ref.as_ref().map(|a|a.turn_id.as_str())).ok_or_else(||invalid("缺少原讲解"))
}

fn ensure_answer_support(state: &PrivateBookContext<'_>, binding: &TeachingBinding, facts: &mut Vec<TeachingEvent>,
    original: &str, now: &str) -> Result<String, ToolError> {
    let mut owner = bound(state, original)?
        .map(|event| event.binding)
        .unwrap_or_else(|| binding.clone());
    owner.turn_id = original.to_owned();
    let id = format!("usage:{}:assistant:{original}", owner.tutor_session_id);
    let store = state
        .user
        .learning_store_for_workspace(state.learning_workspace_id)?;
    // Imported history may predate the teaching trace or fall outside the recent
    // observations window. Recover the exact selected answer before the input.
    if !facts.iter().any(|fact| fact.event_id == id)
        && store.teaching_event_sequence(&id)?.is_none()
    {
        let turn = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == binding.chat_session_id)
            .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == *original))
            .ok_or_else(|| invalid("原聊天缺少所选讲解"))?;
        let outcome = turn
            .outcome
            .as_ref()
            .ok_or_else(|| invalid("原讲解尚未交付"))?;
        let payload = json!({"original_ref":{"kind":"chat_turn","book_id":binding.source_id,"session_id":binding.chat_session_id,"turn_id":original},
            "usage":{"kind":"assistant_delivery"},"incomplete":outcome.incomplete,"answer":outcome.answer,
            "answer_view":outcome.answer_view,"source_bindings":turn.source_bindings});
        facts.insert(
            0,
            TeachingEvent {
                event_id: id.clone(),
                binding: owner,
                kind: TeachingFact::UsageObserved,
                causal_refs: vec![],
                payload,
                occurred_at: now.into(),
            },
        );
    }
    Ok(id)
}

pub(super) fn record(
    user: &crate::user_runtime::UserRuntime,
    workspace: Option<&str>,
    turn: &AgentChatTurn,
    card: &TeachBackComparison,
    now: &str,
) -> Result<(), ToolError> {
    use memory::learning_evidence::{EvidenceNature, LearningEvidence};

    let action = turn
        .domain
        .interaction
        .as_ref()
        .filter(|action| is_entry(action))
        .ok_or_else(|| invalid("对照没有已接纳的开放表达"))?;
    let origin = origin_ref(action)?;
    let mut store = user.learning_store_for_workspace(workspace)?;
    let binding = store
        .teaching_event(&format!("binding:{}", turn.turn_id))?
        .binding;
    if card.source.source_id != binding.source_id
        || card.source.source_revision != binding.source_revision
        || card.round_id != format!("tb:{}:r1", turn.turn_id)
    {
        return Err(invalid("对照与原表达或材料版本不一致"));
    }
    let evidence_id = format!("interpretation:{}", card.round_id);
    // Replay repairs an interrupted derivation. Once persisted, the comparison
    // and its interpretation keep their original wording and predecessor.
    let saved = match store.teaching_event(&card.round_id) {
        Ok(saved) => {
            let original: TeachBackComparison = serde_json::from_value(saved.payload["comparison"].clone())
                .map_err(|error| invalid(error.to_string()))?;
            if &original != card {
                return Err(invalid("同一对照回合不能改写已保存的交付"));
            }
            Some(saved)
        }
        Err(error) if error.error_code == "TEACHING_INVALID" => None,
        Err(error) => return Err(error),
    };
    match store.evidence(&evidence_id) {
        Ok(evidence) => {
            let saved = saved.as_ref().ok_or_else(|| invalid("已保存的解读缺少原对照交付"))?;
            if evidence.delivery_ref.as_ref() != Some(&card.round_id) {
                return Err(invalid("同一对照回合不能改写已保存的交付"));
            }
            if let Some(course) = &action.course {
                let expression = store.teaching_event(&evidence.action_ref)?;
                store.accept_course_comparison(course, saved, &expression, evidence.supersedes.is_some())?;
            }
            return Ok(());
        }
        Err(error) if error.error_code == "TEACHING_INVALID" => {}
        Err(error) => return Err(error),
    }
    let input = store.teaching_event(&format!(
        "usage:{}:user:{}",
        binding.tutor_session_id, turn.turn_id
    ))?;
    let support: Vec<String> =
        serde_json::from_value(input.payload["support_delivery_refs"].clone())
            .map_err(|e| invalid(e.to_string()))?;
    let (expression, supersedes) = if action.teach_back_reply.as_ref().is_some_and(|reply| reply.kind == TeachBackReplyKind::DisputeInterpretation) {
        let first = input.payload["first_round_id"].as_str().ok_or_else(|| invalid("重核解读缺少首次对照"))?;
        let first = store.evidence(&format!("interpretation:{first}"))?;
        (store.teaching_event(&first.action_ref)?, Some(store.latest_replacement_id(&first.evidence_id)?))
    } else {
        let mut payload = json!({"action":"explain_back","response":action.user_supplement,"origin_turn_id":origin,"support_delivery_refs":support});
        if let Some(course) = &action.course { payload["course"] = json!(course); }
        if let Some(reply) = &action.teach_back_reply { payload["teach_back_reply"] = json!(reply); }
        let expression = store.append_teaching(&TeachingEvent {
            event_id:format!("teach-back-expression:{}", turn.turn_id), binding:binding.clone(), kind:TeachingFact::LearnerAction,
            causal_refs:vec![input.event_id.clone()], occurred_at:input.occurred_at.clone(), payload,
        })?;
        (expression, None)
    };
    let mut causal_refs = vec![expression.event_id.clone()];
    if supersedes.is_some() { causal_refs.push(input.event_id); }
    // Course and display receipts can replay a pre-graph delivery. Preserve its
    // original wire representation when reconciling the immutable event.
    let comparison = saved.map(|mut event| event.payload["comparison"].take()).unwrap_or_else(|| json!(card));
    let mut payload = json!({"comparison":comparison,"origin_turn_id":origin,"original_question":action.accepted_request});
    if let Some(course) = &action.course { payload["course"] = json!(course); }
    let delivery = store.append_teaching(&TeachingEvent {
        event_id:card.round_id.clone(), binding:binding.clone(), kind:TeachingFact::TeachBackDelivered,
        causal_refs, occurred_at:now.into(),
        payload,
    })?;
    let row = card
        .follow_up
        .as_ref()
        .and_then(|q| card.rows.iter().find(|row| row.row_id == q.row_id))
        .or_else(|| card.rows.iter().find(|row| !matches!(row.status,
            TeachBackStatus::Consistent | TeachBackStatus::AlternativeValid | TeachBackStatus::Clarified)))
        .or_else(|| card.rows.first())
        .ok_or_else(|| invalid("已交付对照缺少关系行"))?;
    let focus = if action.accepted_request.trim().is_empty() {
        let anchor = action.source_bindings.first().ok_or_else(|| invalid("原讲解缺少出处"))?;
        format!("§ {}", anchor.start_lid)
    } else {
        action.accepted_request.clone()
    };
    let graph = &card.book_graph;
    let edges: Vec<_> = graph.edges.iter().filter(|edge| row.book_edge_ids.contains(&edge.id)).collect();
    let conclusions: Vec<_> = graph.nodes.iter().filter(|node| {
        row.book_node_ids.contains(&node.id) || (
            edges.iter().any(|edge| edge.to == node.id)
            && !edges.iter().any(|edge| edge.from == node.id)
        )
    }).map(|node| node.label.as_str()).collect();
    let target = TeachingTarget {
        focus_ref: Some(format!("teach-back-focus:{origin}")),
        expected_performance: if conclusions.is_empty() { focus.clone() } else { conclusions.join(" / ") },
        learning_focus: focus,
        capability: "explanation".into(),
        source_bindings: action.source_bindings.clone(),
        media_bindings: None,
        music_bindings: None,
        object_refs: vec![],
    };
    let evidence = LearningEvidence {
        evidence_id,
        action_ref: expression.event_id.clone(),
        delivery_ref: Some(delivery.event_id.clone()),
        session_id: binding.tutor_session_id,
        source_id: binding.source_id,
        source_revision: binding.source_revision,
        map_revision: binding.map_revision,
        nature: EvidenceNature::Hypothesis,
        object_id: None,
        object_revision: None,
        label: target.learning_focus.clone(),
        capability: target.capability.clone(),
        target: Some(target),
        fact_refs: vec![expression.event_id.clone()],
        support_delivery_refs: support,
        teaching_implication: card
            .follow_up
            .as_ref()
            .map(|q| q.question.clone())
            .unwrap_or_else(|| row.summary.clone()),
        interpretation: card.rows.iter().map(|row| row.summary.as_str()).collect::<Vec<_>>().join("\n"),
        learner_quote: expression.payload["response"].as_str().ok_or_else(|| invalid("找不到本次表达原话"))?.into(),
        status: AssessmentStatus::Unassessed,
        attempt: 0,
        assessment_ref: None,
        assistance_refs: vec![],
        design_ref: None,
        block_refs: vec![],
        note_refs: vec![],
        criterion_refs: vec![],
        understanding: Default::default(),
        interpreter_version: "teach_back.v2".into(),
        correction: supersedes.as_ref().map(|_| action.user_supplement.clone()),
        corrected_at: supersedes.as_ref().map(|_| delivery.occurred_at.clone()),
        supersedes,
        source_quotes: json!([]),
    };
    store.append_evidence(&evidence)?;
    if let Some(course) = &action.course {
        store.accept_course_comparison(course, &delivery, &expression, evidence.supersedes.is_some())?;
    }
    Ok(())
}

pub(super) fn display(
    state: &PrivateBookContext<'_>,
    chat: &str,
    turn: &str,
    round: &str,
    now: &str,
) -> Result<Value, ToolError> {
    if state.selected_chat != Some(chat)
        || closed_book::read(state, chat)?
            .is_some_and(|value| matches!(value.status.as_str(), "active" | "paused"))
    {
        return Err(invalid("请在原聊天的普通阅读现场查看讲后对照"));
    }
    let session = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| {
            s.id == chat && s.matches_scope(state.learning_workspace_id, &state.book.base.book_id)
        })
        .ok_or_else(|| invalid("讲后对照不属于当前学习空间"))?;
    let saved = session
        .turns
        .iter()
        .find(|t| t.turn_id == turn && t.status == AgentAssistantStatus::Completed)
        .and_then(|t| t.outcome.as_ref())
        .filter(|o| !o.incomplete)
        .and_then(comparison)
        .filter(|c| c.round_id == round && c.source.source_id == state.book.base.book_id)
        .ok_or_else(|| invalid("原回合没有已交付的对照"))?;
    let store = state
        .user
        .learning_store_for_workspace(state.learning_workspace_id)?;
    let receipt_id = format!("display:{round}");
    if store.teaching_event_sequence(&receipt_id)?.is_none() {
        // The delivery might have committed immediately before the derived
        // hypothesis failed to reach disk.
        record_delivery(
            state,
            &AgentTurnRef {
                session_id: chat.into(),
                turn_id: turn.into(),
                user_turn_ordinal: 0,
            },
            now,
        )?;
        let fact = store.teaching_event(&saved.round_id)?;
        append(
            state,
            &fact.binding,
            receipt_id.clone(),
            TeachingFact::Displayed,
            vec![round.into()],
            json!({"delivery_ref":round,"action":"teach_back"}),
            now,
        )?;
    }
    let mut receipt = json!({"event_id":receipt_id});
    if saved.support_delivery_refs.is_none() {
        // Earlier cards persisted their frozen support only in learning history.
        let evidence = store.evidence(&format!("interpretation:{round}"))?;
        receipt["support_delivery_refs"] = json!(evidence.support_delivery_refs);
    }
    Ok(receipt)
}
