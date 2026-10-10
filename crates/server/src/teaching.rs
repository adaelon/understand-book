//! Server-owned teaching delivery, cross-chat references and explicit learner actions.
use crate::*;
use memory::assessment::{
    AssessmentContract, AssessmentStatus, FeedbackPolicy, ResponseAssessment, ScoringRule,
};
use memory::learning::{
    TutorAction, TutorMaterial, TutorMaterialRole, TutorMutation, TutorSessionStatus,
};
use memory::teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact};
use runtime::tutor::TeachingMove;

fn preview(value: &Value, limit: usize) -> Value {
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    json!({"text":text.chars().take(limit).collect::<String>(),"truncated":text.chars().count()>limit})
}
fn trace_summary(event: &TeachingEvent) -> Value {
    json!({"event_id":event.event_id,"binding":event.binding,"kind":event.kind,"causal_refs":event.causal_refs,
        "action":event.payload["action"],"attempt":event.payload["attempt"],"assistance_count":event.payload["assistance_refs"].as_array().map_or(0,Vec::len),"response_preview":preview(&event.payload["response"],600),"move_id":event.payload["move"]["move_id"],
        "prompt_preview":preview(&event.payload["move"]["prompt"],600),"target":event.payload["move"]["target"],
        "answer_preview":preview(&event.payload["answer"],1200),"help_preview":preview(&event.payload["text"],600),
        "original_ref":event.payload["original_ref"],"usage":event.payload["usage"],"occurred_at":event.occurred_at})
}
fn delivered_text(event: &TeachingEvent) -> Value {
    let mut value = event.payload.clone();
    // Browser files/media are preserved in storage; they are not teaching model context.
    if let Some(content) = value
        .get_mut("presentation")
        .and_then(|p| p.get_mut("content"))
        .and_then(Value::as_object_mut)
    {
        content.remove("content_files");
        content.remove("animation_assets");
        content.remove("initial_state");
        content.remove("state_contract");
    }
    json!({"event_id":event.event_id,"binding":event.binding,"kind":event.kind,"content":value})
}

fn readable_fact(state: &PrivateBookContext<'_>, fact: &TeachingEvent) -> Result<Value, ToolError> {
    let mut value = delivered_text(fact);
    if fact.kind == TeachingFact::UsageObserved {
        let mut presentations = Vec::new();
        for part in fact.payload["answer_view"]["parts"].as_array().into_iter().flatten() {
            if let (Some(id),Some(revision))=(part["presentation_id"].as_str(),part["revision"].as_u64()) {
                let version = state.read_presentation(&fact.binding.chat_session_id,
                    &runtime::presentation::PresentationRef {presentation_id:id.into(),revision:revision.try_into().map_err(|_|invalid("演示修订无效"))?})?;
                presentations.push(json!({"reference":version.reference,"owner":version.owner,
                    "readable_content":version.content.readable_content,"source_bindings":version.content.source_bindings}));
            }
        }
        value["presentations"] = json!(presentations);
        if !fact.payload["presentation_follow_up"].is_null() {
            let receipt = serde_json::from_value(fact.payload["presentation_follow_up"].clone()).map_err(|e|invalid(e.to_string()))?;
            value["scene"] = serde_json::to_value(state.read_presentation_state(&receipt)?).map_err(|e|invalid(e.to_string()))?;
        }
    }
    Ok(value)
}

fn event(state: &PrivateBookContext<'_>, id: &str) -> Result<TeachingEvent, ToolError> {
    state.user.learning_store()?.teaching_event(id)
}
fn append(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    id: String,
    kind: TeachingFact,
    causes: Vec<String>,
    payload: Value,
    now: &str,
) -> Result<TeachingEvent, ToolError> {
    append_user(state.user, binding, id, kind, causes, payload, now)
}
fn append_user(
    user: &crate::user_runtime::UserRuntime,
    binding: &TeachingBinding,
    id: String,
    kind: TeachingFact,
    causes: Vec<String>,
    payload: Value,
    now: &str,
) -> Result<TeachingEvent, ToolError> {
    user.learning_store()?.append_teaching(&TeachingEvent {
            event_id: id,
            binding: binding.clone(),
            kind,
            causal_refs: causes,
            payload,
            occurred_at: now.into(),
        })
}
fn bound(state: &PrivateBookContext<'_>, turn: &str) -> Result<Option<TeachingEvent>, ToolError> {
    bound_user(state.user, turn)
}
fn bound_user(user: &crate::user_runtime::UserRuntime, turn: &str) -> Result<Option<TeachingEvent>, ToolError> {
    match user.learning_store()?.teaching_event(&format!("binding:{turn}")) {
        Ok(event) => Ok(Some(event)),
        Err(e) if e.error_code == "TEACHING_INVALID" => Ok(None),
        Err(e) => Err(e),
    }
}
fn current(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<bool, ToolError> {
    let view = state.user.learning_store()?.state()?;
    Ok(view.control.enabled
        && view.control.revision == binding.control_revision
        && view.control.current_tutor_session_id.as_deref() == Some(&binding.tutor_session_id)
        && view
            .sessions
            .get(&binding.tutor_session_id)
            .is_some_and(|s| {
                s.status == TutorSessionStatus::Active && s.revision == binding.session_revision
            }))
}
pub(crate) fn turn_active(state: &PrivateBookContext<'_>, turn: &str) -> Result<bool, ToolError> {
    match bound(state, turn)? {
        Some(event) => current(state, &event.binding),
        None => Ok(false),
    }
}
fn check_source(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<(), ToolError> {
    if binding.source_id != state.book.base.book_id
        || binding.source_revision != state.book.source_fingerprint()
    {
        return Err(invalid("原教学来源已变化，不能替换为最新来源"));
    }
    Ok(())
}
fn read_map(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<Value, ToolError> {
    check_source(state, binding)?;
    let path = state.book_dir
        .join("teaching/versions")
        .join(binding.map_revision.as_deref().ok_or_else(|| invalid("本次教学未绑定地图"))?)
        .join("map.json");
    let map: Value = serde_json::from_slice(
        &std::fs::read(path).map_err(|e| invalid(format!("原教学素材无法读取：{e}")))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    if map["source_revision"] != binding.source_revision || map["source_id"] != binding.source_id {
        return Err(invalid("原教学版本不可用"));
    }
    Ok(map)
}

/// A published map is immutable. Standalone accepted assets are frozen privately
/// because the builder may replace their public files during this turn.
fn turn_assets(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<Value, ToolError> {
    check_source(state, binding)?;
    if binding.map_revision.is_some() {
        read_map(state, binding)
    } else {
        Ok(event(state, &format!("assets:{}", binding.turn_id))?.payload)
    }
}

fn accepted_assets(state: &PrivateBookContext<'_>, ready: &Value) -> Value {
    let mut assets = json!({});
    for (stage, key) in [("formal_objects", "objects"), ("cognitive_materials", "cognitive_materials")] {
        let reference = &ready["teaching_assets"][stage];
        if reference["status"] != "ready" { continue; }
        let path = state.book_dir.join(format!("{stage}.json"));
        let value = std::fs::read(path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        if let Some(value) = value.filter(|v| v["source_id"] == state.book.base.book_id
            && v["source_revision"] == state.book.source_fingerprint() && v["revision"] == reference["revision"])
        {
            assets[key] = value;
        }
    }
    assets
}

fn check_activity_source(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<(), ToolError> {
    // Delivery and MoveSelected retain the original target, material revisions,
    // source quotes and scoring contract. A later publication need not include
    // the old map file; responding never substitutes its current map or rubric.
    check_source(state, binding)
}

fn target_read_this_turn(state: &PrivateBookContext<'_>, start: &str, end: &str, ranges: &[EvidenceRange]) -> bool {
    let nodes = &state.book.base.lid_nodes;
    let node = |lid: &str| nodes.iter().find(|n| n.lid == lid);
    let (Some(first), Some(last)) = (node(start), node(end)) else { return false; };
    if first.span.start > last.span.start || first.span.start >= last.span.end { return false; }
    // Compare the source spans actually read, including parent-LID reads and
    // selected-text intervals. Endpoint overlap alone cannot prove a whole target.
    let mut reads = Vec::new();
    for range in ranges {
        if range.ranges.is_empty() {
            if let (Some(a), Some(b)) = (node(&range.start_lid), node(&range.end_lid)) {
                reads.push((a.span.start, b.span.end));
            }
        } else {
            for selected in &range.ranges {
                if let Some(n) = node(&selected.lid) {
                    reads.push((n.span.start + selected.range.start as usize, n.span.start + selected.range.end as usize));
                }
            }
        }
    }
    reads.sort_unstable();
    let mut covered = first.span.start;
    for (a, b) in reads {
        if a > covered { break; }
        covered = covered.max(b);
        if covered >= last.span.end { return true; }
    }
    false
}
fn active_objects(map: &Value) -> Vec<Value> {
    map["objects"]["active_refs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            map["objects"]["objects"]
                .as_array()?
                .iter()
                .rev()
                .find(|o| o["ref"] == *r)
                .cloned()
        })
        .collect()
}

/// Control/start buttons are explicit start requests. Navigation and readiness reads never call this.
pub(crate) fn start_request(state: &PrivateBookContext<'_>, now: &str) -> Result<Value, ToolError> {
    let ready = crate::tutor_api::tutor_source_readiness(&state.book, &state.book_dir);
    let mut store = state.user.learning_store()?;
    let mut view = store.state()?;
    if !view.control.enabled || ready["status"] != "ready" {
        return Ok(json!({"started":false,"reason":ready["reason"]}));
    }
    let existing = view
        .control
        .current_tutor_session_id
        .as_ref()
        .and_then(|id| view.sessions.get(id));
    if let Some(session) = existing {
        if session.status == TutorSessionStatus::Ended {
            return Ok(json!({"started":false,"reason":"本次学习已结束，请明确开始或继续"}));
        }
        if !session
            .material_scope
            .iter()
            .any(|m| m.source_id == state.book.base.book_id)
        {
            return Ok(
                json!({"started":false,"reason":"当前材料不属于本次学习，请回到原材料或明确开始新目标"}),
            );
        }
        if session.status == TutorSessionStatus::Paused {
            view = store.mutate(
                &TutorMutation {
                    operation_id: format!("resume-{}-{}", session.id, view.control.revision),
                    expected_revision: view.control.revision,
                    action: TutorAction::Resume {
                        session_id: session.id.clone(),
                    },
                },
                now,
            )?;
        }
    } else {
        view = store.mutate(
            &TutorMutation {
                operation_id: format!("begin-{}", view.control.revision),
                expected_revision: view.control.revision,
                action: TutorAction::Start {
                    user_intent: "理解当前阅读材料；从当前问题或阅读位置开始".into(),
                    explicit_constraints: vec![],
                    material_scope: vec![TutorMaterial {
                        source_id: state.book.base.book_id.clone(),
                        scope_refs: vec![],
                        role: TutorMaterialRole::Primary,
                    }],
                    default_teaching_intent: None,
                },
            },
            now,
        )?;
    }
    Ok(
        json!({"started":true,"state":view,"message":"请继续本次学习，结合当前阅读位置和已有回应，选择一个有依据的教学动作。"}),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FrozenTeachingPreparation {
    pub context: Option<Value>,
    pub events: Vec<TeachingEvent>,
}
impl FrozenTeachingPreparation {
    /// append_teaching compares the complete payload and reuses the original receipt.
    pub(crate) fn resume(&self, user: &crate::user_runtime::UserRuntime) -> Result<(), ToolError> {
        let mut learning = user.learning_store()?;
        for event in &self.events { learning.append_teaching(event)?; }
        Ok(())
    }
}
pub(crate) fn prepare(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, request: &Value, now: &str) -> Result<Option<Value>, ToolError> {
    let frozen = freeze_prepare(state, turn, request, now)?;
    frozen.resume(state.user)?;
    Ok(frozen.context)
}

pub(crate) fn freeze_prepare(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    request: &Value,
    now: &str,
) -> Result<FrozenTeachingPreparation, ToolError> {
    let mut events = Vec::new();
    if let Some(id) = request["teaching_ref"].as_str() {
        let original = referenced_delivery(state, id)?;
        let prior = state
            .user.learning_store()?
            .teaching_activity_events(&original.event_id)?;
        let mut binding = original.binding.clone();
        binding.chat_session_id = turn.session_id.clone();
        binding.turn_id = turn.turn_id.clone();
        events.push(TeachingEvent {
            event_id: format!("response:{}", turn.turn_id),
            binding,
            kind: TeachingFact::LearnerAction,
            causal_refs: vec![id.into(), original.event_id.clone()],
            payload: json!({"delivery_ref":original.event_id,"action":"chat_response","response":request["message"],"reference":id,"assessment":"unassessed",
                "assistance_refs":prior.iter().filter(|e|e.kind==TeachingFact::HelpDisplayed).map(|e|&e.event_id).collect::<Vec<_>>(),
                "attempt":prior.iter().filter(|e|e.kind==TeachingFact::LearnerAction && matches!(e.payload["action"].as_str(),Some("submit"|"revise"|"chat_response"))).count()+1}),
            occurred_at: now.into(),
        });
    }
    let mut view = state.user.learning_store()?.state()?;
    // The first question supplies the starting intent. Merely opening a book or
    // enabling the switch never invents a goal or sends a synthetic question.
    if view.control.enabled && view.control.current_tutor_session_id.is_none() {
        if let Some(message) = request["message"].as_str().filter(|s| !s.trim().is_empty()) {
            view = state.user.learning_store()?.mutate(&TutorMutation {
                operation_id: format!("first-question:{}", turn.session_id),
                expected_revision: view.control.revision,
                action: TutorAction::Start { user_intent:message.into(), explicit_constraints:vec![],
                    material_scope:vec![TutorMaterial { source_id:state.book.base.book_id.clone(), scope_refs:vec![], role:TutorMaterialRole::Primary }],
                    default_teaching_intent:None },
            }, now)?;
        }
    }
    let Some(session) = view
        .control
        .current_tutor_session_id
        .as_ref()
        .and_then(|id| view.sessions.get(id))
    else {
        return prepare_observation(state, turn, request, now, events);
    };
    if !view.control.enabled
        || session.status != TutorSessionStatus::Active
        || !session
            .material_scope
            .iter()
            .any(|m| m.source_id == state.book.base.book_id)
    {
        return prepare_observation(state, turn, request, now, events);
    }
    let ready = crate::tutor_api::tutor_source_readiness(&state.book, &state.book_dir);
    if ready["status"] != "ready" {
        return prepare_observation(state, turn, request, now, events);
    }
    let binding = TeachingBinding {
        tutor_session_id: session.id.clone(),
        session_revision: session.revision,
        control_revision: view.control.revision,
        source_id: state.book.base.book_id.clone(),
        source_revision: state.book.source_fingerprint().into(),
        map_revision: ready["teaching_assets"]["teaching_map_revision"].as_str().map(str::to_owned),
        chat_session_id: turn.session_id.clone(),
        turn_id: turn.turn_id.clone(),
    };
    let map = if binding.map_revision.is_some() { read_map(state, &binding)? } else {
        let assets = accepted_assets(state, &ready);
        events.push(TeachingEvent {
            event_id: format!("assets:{}", turn.turn_id), binding: binding.clone(),
            kind: TeachingFact::TurnBound, causal_refs: vec![], payload: assets.clone(), occurred_at: now.into(),
        });
        assets
    };
    let objects = active_objects(&map);
    events.extend(usage_facts(state, &binding, request, now)?);
    refresh_learning(state, &binding.tutor_session_id, &binding.source_id)?;
    let mut recent = state
        .user.learning_store()?
        .teaching_recent(&session.id, 8)?;
    for event in events.iter().filter(|e| e.binding.tutor_session_id == session.id && matches!(e.kind, TeachingFact::LearnerAction | TeachingFact::UsageObserved)) {
        recent.insert(0, event.clone());
    }
    recent.truncate(8);
    let confirmed: Vec<_> = state
        .user.store
        .resolve_profile_facts(&ProfileResolutionContext {
            book_id: Some(binding.source_id.clone()),
            content_profile: Some(current_content_profile(&state.book).into()),
            now: Some(now.into()),
            ..Default::default()
        })
        .into_iter()
        .filter(|f| f.status == memory::FactStatus::Confirmed)
        .take(16)
        .collect();
    let preferences: Vec<_> = confirmed.iter().filter(|f| matches!(f.payload, memory::ProfilePayload::ExplanationPreference(_)))
        .map(|f| json!({"fact_id":f.fact_id,"preference":f.payload})).collect();
    let background: Vec<_> = confirmed.iter().filter(|f| matches!(f.payload, memory::ProfilePayload::Background(_) | memory::ProfilePayload::Capability(_)))
        .map(|f| json!({"fact_id":f.fact_id,"claim":preview(&json!(f.payload),1000)})).collect();
    let mut learner = learner_context(state,&binding,&objects,&session.current_focus.target_object_refs,request["teaching_ref"].as_str())?;
    learner["current_request"] = request["message"].clone();
    learner["user_intent"] = json!(session.user_intent);
    learner["constraints"] = json!(session.explicit_constraints);
    learner["confirmed_background"] = json!(background);
    learner["confirmed_preferences"] = json!(preferences);
    learner["session_focus"] = json!(session.current_focus);
    learner["recent_facts"] = json!(recent.iter().filter(|e| e.binding.source_id == binding.source_id).map(trace_summary).collect::<Vec<_>>());
    learner["uninterpreted_fact_refs"] = json!(recent.iter().filter(|e|e.binding.source_id==binding.source_id && e.binding.source_revision==binding.source_revision)
        .map(|e|state.user.learning_store()?.fact_has_interpretation(&e.event_id).map(|seen|(!seen).then_some(e.event_id.clone())))
        .collect::<Result<Vec<_>,ToolError>>()?.into_iter().flatten().collect::<Vec<_>>());
    let context = json!({"status":"active","binding":binding,"user_intent":session.user_intent,
        "constraints":session.explicit_constraints,"default_teaching_intent":session.default_teaching_intent,
        "current_request":request["message"],"confirmed_preferences":preferences,"learner_context":learner,"limitations":ready["teaching_assets"]["limitations"],
        "teaching_assets":ready["teaching_assets"],
        "objects":objects.iter().take(16).map(|o| json!({"ref":o["ref"],"meaning":o["meaning"]})).collect::<Vec<_>>(),
        "object_count":objects.len(),"recent_facts":recent.iter().map(trace_summary).collect::<Vec<_>>()});
    events.push(TeachingEvent {
        event_id: format!("binding:{}", turn.turn_id), binding,
        kind: TeachingFact::TurnBound, causal_refs: vec![], payload: context.clone(), occurred_at: now.into(),
    });
    Ok(FrozenTeachingPreparation { context: Some(context), events })
}

/// Ordinary use has an observation scope, not a persisted/active TutorSession.
fn prepare_observation(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, request: &Value, now: &str,
    mut events: Vec<TeachingEvent>) -> Result<FrozenTeachingPreparation, ToolError> {
    let binding = TeachingBinding {
        tutor_session_id:format!("observation:{}",state.book.base.book_id), session_revision:0, control_revision:0,
        source_id:state.book.base.book_id.clone(), source_revision:state.book.source_fingerprint().into(), map_revision:None,
        chat_session_id:turn.session_id.clone(), turn_id:turn.turn_id.clone(),
    };
    let ready = crate::tutor_api::tutor_source_readiness(&state.book,&state.book_dir);
    let assets = accepted_assets(state,&ready);
    let objects = active_objects(&assets);
    events.push(TeachingEvent { event_id:format!("assets:{}",turn.turn_id), binding:binding.clone(),kind:TeachingFact::TurnBound,
        causal_refs:vec![],payload:assets,occurred_at:now.into() });
    events.extend(usage_facts(state,&binding,request,now)?);
    let mut recent = state.user.learning_store()?.teaching_recent(&binding.tutor_session_id,8)?;
    for fact in events.iter().filter(|e| e.kind == TeachingFact::UsageObserved) { recent.insert(0,fact.clone()); }
    recent.truncate(8);
    let mut learner = learner_context(state,&binding,&objects,&[],None)?;
    learner["current_request"] = request["message"].clone();
    learner["recent_facts"] = json!(recent.iter().map(trace_summary).collect::<Vec<_>>());
    let context = json!({"status":"observing","binding":binding,"learner_context":learner,"readiness":ready["status"],
        "reason":ready["reason"],"reference":request["teaching_ref"]});
    events.push(TeachingEvent { event_id:format!("binding:{}",turn.turn_id),binding,kind:TeachingFact::TurnBound,
        causal_refs:vec![],payload:context.clone(),occurred_at:now.into() });
    Ok(FrozenTeachingPreparation { context:Some(context),events })
}

/// Reference the existing private owners. Reading snapshots retain their original
/// count/time; they do not claim a source revision the reading ledger never stored.
fn usage_facts(state: &PrivateBookContext<'_>, binding: &TeachingBinding, request: &Value, now: &str) -> Result<Vec<TeachingEvent>, ToolError> {
    let mut facts = Vec::new();
    let mut add = |id: String, chat: &str, turn: &str, payload: Value, at: &str| -> Result<(), ToolError> {
        if facts.iter().any(|e: &TeachingEvent| e.event_id == id) { return Ok(()); }
        match state.user.learning_store()?.teaching_event(&id) {
            Ok(_) => return Ok(()),
            Err(e) if e.error_code == "TEACHING_INVALID" => {},
            Err(e) => return Err(e),
        }
        let mut original_binding = binding.clone();
        original_binding.chat_session_id = chat.into();
        original_binding.turn_id = turn.into();
        let causes = facts.last().map(|e: &TeachingEvent| vec![e.event_id.clone()]).unwrap_or_default();
        facts.push(TeachingEvent { event_id:id, binding:original_binding, kind:TeachingFact::UsageObserved,
            causal_refs:causes, payload, occurred_at:at.into() });
        Ok(())
    };
    let mut reads = state.user.store.recall(&memory::RecallQuery { book_id:Some(binding.source_id.clone()), mem_type:Some("read".into()), ..Default::default() });
    reads.sort_by(|a,b| a.generated_at.cmp(&b.generated_at).then(a.mem_id.cmp(&b.mem_id)));
    for record in reads.iter().rev().take(8).rev() {
        add(format!("usage:{}:read:{}:{}", binding.tutor_session_id, record.mem_id, record.usage.count),
            &binding.chat_session_id, &binding.turn_id,
            json!({"original_ref":{"kind":"reading_record","mem_id":record.mem_id,"book_id":record.book_id},
                "usage":{"kind":"reading_contact","lid":record.anchor.lid,"count":record.usage.count,"source_revision":null},"record":record}), &record.generated_at)?;
    }
    let mut sessions: Vec<_> = state.user.agent_history.sessions.iter().filter(|s| s.book_id == binding.source_id).collect();
    sessions.sort_by(|a,b|a.updated_at.cmp(&b.updated_at));
    let recent: Vec<_> = sessions.into_iter().flat_map(|s| s.turns.iter().map(move |t| (s,t))).rev().take(12).collect();
    for (session, turn) in recent.into_iter().rev() {
        let original = json!({"kind":"chat_turn","book_id":session.book_id,"session_id":session.id,"turn_id":turn.turn_id});
        add(format!("usage:{}:user:{}", binding.tutor_session_id, turn.turn_id), &session.id, &turn.turn_id,
            json!({"original_ref":original,"usage":{"kind":"user_message"},"response":turn.user,"presentation_follow_up":turn.presentation_follow_up,
                "question_anchor_lid":turn.question_anchor_lid,"teaching_ref":turn.teaching_ref}), &session.updated_at)?;
        if let Some(outcome) = turn.outcome.as_ref().filter(|o| turn.status == AgentAssistantStatus::Completed
            && !crate::observability::lifecycle::delivery_failed(o)
            && !turn.delivery_diagnostics.as_ref().and_then(|d|d.repair.as_ref()).is_some_and(|r|!r.issues.is_empty())) {
            add(format!("usage:{}:assistant:{}", binding.tutor_session_id, turn.turn_id), &session.id, &turn.turn_id,
                json!({"original_ref":original,"usage":{"kind":"assistant_delivery"},"answer":outcome.answer,"answer_view":outcome.answer_view,
                    "source_bindings":turn.source_bindings}), &session.updated_at)?;
        }
    }
    add(format!("usage:{}:user:{}", binding.tutor_session_id, binding.turn_id), &binding.chat_session_id, &binding.turn_id,
        json!({"original_ref":{"kind":"chat_turn","book_id":binding.source_id,"session_id":binding.chat_session_id,"turn_id":binding.turn_id},
            "usage":{"kind":"user_message"},"response":request["message"],"presentation_follow_up":request["presentation_follow_up"]}), now)?;
    Ok(facts)
}

pub(crate) fn step(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    request: Value,
    evidence: &[SourceBinding],
    ranges: &[EvidenceRange],
) -> Result<Value, ToolError> {
    let occurred_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| invalid(e.to_string()))?;
    let bind = match bound(state, &turn.turn_id)? {
        Some(bind) => bind,
        None if request["operation"] == "trace" => {
            event(state, &format!("response:{}", turn.turn_id))?
        }
        None => return Err(invalid("本轮教学素材尚未就绪或教学未开启")),
    };
    let binding = &bind.binding;
    let observation_operation = matches!(request["operation"].as_str(),Some("trace"|"understanding"))
        || (request["operation"] == "evidence" && request["nature"] == "hypothesis");
    if !observation_operation && !current(state, binding)? {
        return Err(invalid(
            "教学已暂停或切换，请完成普通请求并停止自动教学推进",
        ));
    }
    match request["operation"].as_str() {
        Some("evidence") if request["nature"] == "hypothesis" => accept_hypothesis(state, binding, &request, ranges),
        Some("evidence") => accept_ungraded_evidence(state, binding, &request),
        Some("understanding") => {
            if let Some(id) = request["evidence_ref"].as_str() {
                let value = evidence_view(state, id)?;
                let text = value.to_string();
                let after = request["after"].as_u64().unwrap_or(0) as usize;
                Ok(
                    json!({"chunk":text.chars().skip(after).take(8000).collect::<String>(),"next":(after+8000<text.chars().count()).then_some(after+8000)}),
                )
            } else {
                understanding(
                    state,
                    &binding.source_id,
                    request["object_id"].as_str(),
                    request["after"].as_u64().unwrap_or(0),
                )
            }
        }
        Some("trace") => {
            check_activity_source(state, binding)?;
            if let Some(id) = request["event_id"].as_str() {
                let fact = event(state, id)?;
                if (fact.binding.tutor_session_id != binding.tutor_session_id && bind.payload["status"] != "observing") || fact.binding.source_id != binding.source_id
                    || matches!(
                        fact.kind,
                        TeachingFact::MoveSelected
                            | TeachingFact::TurnBound
                            | TeachingFact::HelpDelivered
                    )
                {
                    return Err(invalid("此引用不是可回读的已交付教学事实"));
                }
                let text = readable_fact(state,&fact)?.to_string();
                let offset = request["after"].as_u64().unwrap_or(0) as usize;
                let total = text.chars().count();
                return Ok(
                    json!({"event_id":id,"chunk":text.chars().skip(offset).take(8000).collect::<String>(),"total_chars":total,"next":(offset+8000<total).then_some(offset+8000)}),
                );
            }
            let rows = state.user.learning_store()?.teaching_events(
                &binding.tutor_session_id,
                request["after"].as_u64().unwrap_or(0),
                12,
            )?;
            // Do not expose unrequested answer keys/help held in move candidates.
            Ok(
                json!({"events": rows.iter().filter(|(_, e)| !matches!(e.kind, TeachingFact::MoveSelected | TeachingFact::TurnBound | TeachingFact::HelpDelivered)).map(|(seq,e)| json!({"cursor":seq,"event":trace_summary(e)})).collect::<Vec<_>>(), "next":rows.last().map(|(seq,_)| seq)}),
            )
        }
        Some("material") => {
            let map = turn_assets(state, binding)?;
            let objects = active_objects(&map);
            if let Some(id) = request["object_id"].as_str() {
                let object = objects
                    .iter()
                    .find(|o| o["ref"]["object_id"] == id)
                    .ok_or_else(|| invalid("正式对象不存在"))?;
                let materials: Vec<_> = map["cognitive_materials"]["materials"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| {
                        m["object_refs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|r| r["object_id"] == id)
                    })
                    .cloned()
                    .collect();
                append(
                    state,
                    binding,
                    format!("material:{}:{id}", turn.turn_id),
                    TeachingFact::TurnBound,
                    vec![bind.event_id],
                    json!({"object":object,"materials":materials}),
                    &occurred_at,
                )?;
                Ok(
                    json!({"object":object,"materials":materials,"source_revision":binding.source_revision}),
                )
            } else {
                let offset = request["after"].as_u64().unwrap_or(0) as usize;
                Ok(
                    json!({"objects":objects.iter().skip(offset).take(16).map(|o| json!({"ref":o["ref"],"meaning":o["meaning"]})).collect::<Vec<_>>(),"next":(offset+16<objects.len()).then_some(offset+16)}),
                )
            }
        }
        Some("outside") => Ok(serde_json::to_value(append(
            state,
            binding,
            format!("outside:{}", turn.turn_id),
            TeachingFact::TurnBound,
            vec![bind.event_id],
            json!({"outside":true}),
            &occurred_at,
        )?)
        .unwrap()),
        Some("select") => {
            let movement: TeachingMove = serde_json::from_value(request["move"].clone())
                .map_err(|e| invalid(e.to_string()))?;
            for id in &movement.interpretation_refs {
                let store = state.user.learning_store()?;
                let interpretation = store.evidence(id)?;
                if interpretation.source_id != binding.source_id || interpretation.source_revision != binding.source_revision
                    || store.evidence_is_superseded(id)?
                { return Err(invalid("所采用解释已被修订或不属于当前来源")); }
                if let (Some(scope), Some(target)) = (&interpretation.target, &movement.target) {
                    if !scope.source_bindings.iter().any(|a| target.source_bindings.iter().any(|b|
                        a.source_id == b.source_id && a.source_revision == b.source_revision
                        && source_ranges_overlap(state, &a.start_lid, &a.end_lid, &b.start_lid, &b.end_lid)))
                    { return Err(invalid("所采用解释与本次教学范围不相交")); }
                }
            }
            if !["direct_explanation", "guided_inquiry", "neutral"]
                .contains(&movement.interaction_intent.as_str())
                || ![
                    "current_request",
                    "session_default",
                    "confirmed_preference",
                    "neutral",
                ]
                .contains(&movement.intent_origin.as_str())
                || (movement.intent_origin == "current_request"
                    && !movement.intent_quote.as_deref().is_some_and(|q| {
                        !q.trim().is_empty()
                            && bind.payload["current_request"]
                                .as_str()
                                .is_some_and(|s| s.contains(q))
                    }))
                || (movement.intent_origin == "session_default"
                    && bind.payload["default_teaching_intent"] != movement.interaction_intent)
                || (movement.intent_origin == "confirmed_preference"
                    && (!bind.payload["default_teaching_intent"].is_null()
                        || !bind.payload["confirmed_preferences"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|p| p["fact_id"].as_str() == movement.preference_ref.as_deref())))
                || (movement.interaction_intent == "direct_explanation"
                    && movement.kind != "explain")
            {
                return Err(invalid(
                    "教学意图需匹配当前原话或会话默认；直接讲解应采用解释动作",
                ));
            }
            if movement.move_id.trim().is_empty()
                || ((movement.object_ids.is_empty() || binding.map_revision.is_none()) && movement.target.is_none())
                || movement.prompt.trim().is_empty()
                || movement.capability.trim().is_empty()
                || !["explain", "observe", "compare", "question", "source_focus"]
                    .contains(&movement.kind.as_str())
                || movement.actions.iter().any(|a| {
                    ![
                        "submit",
                        "revise",
                        "request_hint",
                        "reveal",
                        "skip",
                        "self_report",
                    ]
                    .contains(&a.as_str())
                })
            {
                return Err(invalid("教学动作缺少来源目标、能力、题面或使用了未知行为"));
            }
            if movement.actions.iter().any(|a| a == "request_hint")
                && movement.hint.as_deref().is_none_or(|s| s.trim().is_empty())
                || movement.actions.iter().any(|a| a == "reveal")
                    && movement
                        .explanation
                        .as_deref()
                        .is_none_or(|s| s.trim().is_empty())
            {
                return Err(invalid("可用帮助必须有实际内容"));
            }
            let map = turn_assets(state, binding)?;
            let objects = active_objects(&map);
            if let Some(target) = &movement.target {
                target.validate(binding)?;
                if target.capability != movement.capability
                    || target.object_refs.len() != movement.object_ids.len()
                    || movement.object_ids.iter().any(|id| !target.object_refs.iter().any(|r| &r.object_id == id))
                    || target.object_refs.iter().any(|r| !objects.iter().any(|o|
                        o["ref"]["source_id"] == r.source_id && o["ref"]["object_id"] == r.object_id
                            && o["object_revision"].as_u64() == Some(r.object_revision)))
                {
                    return Err(invalid("教学目标的能力、对象与所用修订不一致"));
                }
                if !target.source_bindings.iter().all(|s| target_read_this_turn(state, &s.start_lid, &s.end_lid, ranges)) {
                    return Err(invalid("教学目标的确切范围必须由本轮原文读取支持，请读取完整目标原文"));
                }
            }
            for id in &movement.object_ids {
                let object = objects
                    .iter()
                    .find(|o| o["ref"]["object_id"] == *id)
                    .ok_or_else(|| invalid("正式对象不属于此教学版本"))?;
                event(state, &format!("material:{}:{id}", turn.turn_id))?;
                let has_evidence = object["source_bindings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|source| {
                        ranges.iter().any(|e| {
                            source["lid"].as_str().is_some_and(|lid| {
                                let nodes = &state.book.base.lid_nodes;
                                let position =
                                    |value: &str| nodes.iter().position(|n| n.lid == value);
                                match (position(&e.start_lid), position(lid), position(&e.end_lid))
                                {
                                    (Some(a), Some(b), Some(c)) => a <= b && b <= c,
                                    _ => false,
                                }
                            })
                        })
                    });
                if !has_evidence {
                    return Err(invalid("选择教学动作前请读取目标对象的真实原文"));
                }
            }
            if let Some(reference) = &movement.presentation {
                state.read_presentation(&turn.session_id, reference)?;
            }
            // Public prompt/help use the same provenance compiler as an answer.
            runtime::orchestrator::compile_presentation_text(
                &movement.prompt,
                evidence,
                &state.messages,
            )
            .map_err(|_| invalid("教学文字来源无效"))?;
            for text in movement.hint.iter().chain(movement.explanation.iter()) {
                runtime::orchestrator::compile_presentation_text(text, evidence, &state.messages)
                    .map_err(|_| invalid("帮助文字来源无效"))?;
            }
            let mut scoring_sources = BTreeMap::new();
            if let Some(contract) = &movement.assessment {
                contract.validate()?;
                if !movement.actions.iter().any(|a| a == "submit") {
                    return Err(invalid("评分活动必须允许提交"));
                }
                for lid in &contract.source_lids {
                    if !target_read_this_turn(state,lid,lid,ranges) {
                        return Err(invalid("冻结评分合同前必须读取评分来源"));
                    }
                    scoring_sources.insert(lid.clone(), state.book.text(lid, Some(lid))?);
                }
                if serde_json::to_string(&scoring_sources).unwrap().len() > 24000 {
                    return Err(invalid("评分来源超出单次评估预算，请缩小活动"));
                }
            }
            let object_versions: Vec<_> = objects.iter().filter(|o| movement.object_ids.iter().any(|id| o["ref"]["object_id"] == *id))
                .map(|o| json!({"ref":o["ref"],"object_revision":o["object_revision"],"meaning":o["meaning"]})).collect();
            let saved = append(
                state,
                binding,
                format!("move:{}:{}", turn.turn_id, movement.move_id),
                TeachingFact::MoveSelected,
                vec![bind.event_id],
                json!({"move":movement,"object_versions":object_versions,"scoring_sources":scoring_sources,"scorer_version":"assessment.v1","retry_policy":"append_attempt","help_policy":"preserve_exposure"}),
                &occurred_at,
            )?;
            Ok(
                json!({"selected":saved.event_id,"instruction":"Deliver this move in the final answer. Saved candidate is not displayed."}),
            )
        }
        _ => Err(invalid("未知教学操作")),
    }
}

/// Called only after the completed answer is durably committed. Safe to retry after a DB failure.
pub(crate) fn record_delivery(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    now: &str,
) -> Result<(), ToolError> {
    record_user_delivery(state.user, turn, now)
}

/// Replays only durable private delivery facts; no live Book or current teaching control is needed.
pub(crate) fn record_user_delivery(
    user: &crate::user_runtime::UserRuntime,
    turn: &AgentTurnRef,
    now: &str,
) -> Result<(), ToolError> {
    let session = user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == turn.session_id)
        .ok_or_else(|| invalid("原聊天不存在"))?;
    let saved = session
        .turns
        .iter()
        .find(|t| t.turn_id == turn.turn_id)
        .ok_or_else(|| invalid("原回合不存在"))?;
    let Some(outcome) = saved.outcome.as_ref().filter(|o| {
        saved.status == AgentAssistantStatus::Completed
            && !crate::observability::lifecycle::delivery_failed(o)
            && !saved
                .delivery_diagnostics
                .as_ref()
                .and_then(|d| d.repair.as_ref())
                .is_some_and(|r| !r.issues.is_empty())
    }) else {
        return Ok(());
    };
    if let Some(id) = &saved.teaching_ref {
        let original = referenced_user_delivery(user, &session.book_id, id)?;
        let mut binding = original.binding.clone();
        binding.chat_session_id = turn.session_id.clone();
        binding.turn_id = turn.turn_id.clone();
        // A reply about an existing activity remains observable help even in ordinary mode.
        append_user(
            user,
            &binding,
            format!("followup:{}", turn.turn_id),
            TeachingFact::MessageDelivered,
            vec![original.event_id.clone()],
            json!({"move":{"move_id":"followup","prompt":outcome.answer,"actions":[],"presentation":null,
                "target":original.payload["move"]["target"],"capability":original.payload["move"]["capability"],"object_ids":original.payload["move"]["object_ids"]},"answer":outcome.answer,"answer_view":outcome.answer_view,
                "source_bindings":saved.source_bindings,"assistance_for":original.event_id}),
            now,
        )?;
    }
    let Some(bind) = bound_user(user, &turn.turn_id)? else {
        return Ok(());
    };
    let request_ref = format!("usage:{}:user:{}", bind.binding.tutor_session_id, turn.turn_id);
    let causes = user.learning_store()?.teaching_event(&request_ref).ok().map(|e|vec![e.event_id]).unwrap_or_default();
    append_user(user, &bind.binding, format!("usage:{}:assistant:{}", bind.binding.tutor_session_id, turn.turn_id),
        TeachingFact::UsageObserved, causes,
        json!({"original_ref":{"kind":"chat_turn","book_id":session.book_id,"session_id":session.id,"turn_id":saved.turn_id},
            "usage":{"kind":"assistant_delivery"},"answer":outcome.answer,"answer_view":outcome.answer_view,"source_bindings":saved.source_bindings}), now)?;
    let events = user
        .learning_store()?
        .teaching_turn_events(&bind.binding.tutor_session_id, &turn.turn_id)?;
    if events.iter().any(|e| e.payload["outside"] == true) {
        return Ok(());
    }
    // A committed answer remains a delivery fact after pause, including recovery of a failed trace write.
    for selected in events
        .iter()
        .filter(|e| e.kind == TeachingFact::MoveSelected)
    {
        let movement: TeachingMove = serde_json::from_value(selected.payload["move"].clone())
            .map_err(|e| invalid(e.to_string()))?;
        let presentation = if let Some(reference) = &movement.presentation {
            if !outcome.answer_view.as_ref().is_some_and(|view| view.parts.iter().any(|p| matches!(p, AgentAnswerPart::Presentation { presentation_id, revision } if presentation_id == &reference.presentation_id && revision == &reference.revision))) { continue; }
            Some(crate::presentation_store::PresentationStore::for_user(user)?.read_version(
                &runtime::presentation::PresentationOwner { book_id: session.book_id.clone(), session_id: turn.session_id.clone() }, reference)?)
        } else {
            None
        };
        let mut public_move = serde_json::to_value(&movement).unwrap();
        public_move.as_object_mut().unwrap().remove("hint");
        public_move.as_object_mut().unwrap().remove("explanation");
        public_move.as_object_mut().unwrap().remove("assessment");
        append_user(
            user,
            &bind.binding,
            format!("delivery:{}:{}", turn.turn_id, movement.move_id),
            TeachingFact::MessageDelivered,
            vec![selected.event_id.clone()],
            json!({"move":public_move,"object_versions":selected.payload["object_versions"],"assessment_contract_ref":movement.assessment.as_ref().map(|_| &selected.event_id),"answer":outcome.answer,"answer_view":outcome.answer_view,"source_bindings":saved.source_bindings,"presentation":presentation}),
            now,
        )?;
    }
    Ok(())
}

fn referenced_delivery(state: &PrivateBookContext<'_>, id: &str) -> Result<TeachingEvent, ToolError> {
    referenced_user_delivery(state.user, &state.book.base.book_id, id)
}
fn referenced_user_delivery(user: &crate::user_runtime::UserRuntime, book_id: &str, id: &str) -> Result<TeachingEvent, ToolError> {
    let store = user.learning_store()?;
    let fact = store.teaching_event(id)?;
    let saved = if fact.kind == TeachingFact::MessageDelivered { fact } else {
        store.teaching_event(fact.payload["delivery_ref"].as_str().ok_or_else(|| invalid("原教学交付引用缺失"))?)?
    };
    if saved.kind != TeachingFact::MessageDelivered || saved.binding.source_id != book_id {
        return Err(invalid("教学交付不属于当前材料"));
    }
    Ok(saved)
}

/// Explicit references bypass only the active-chat restriction, never original ownership/version.
pub(crate) fn reference_context(state: &PrivateBookContext<'_>, id: &str) -> Result<String, ToolError> {
    let fact = event(state, id)?;
    if !matches!(
        fact.kind,
        TeachingFact::MessageDelivered
            | TeachingFact::Displayed
            | TeachingFact::LearnerAction
            | TeachingFact::HelpDisplayed
    ) {
        return Err(invalid("此引用不是已交付教学或显式行为"));
    }
    if fact.binding.source_id != state.book.base.book_id {
        return Err(invalid("请打开原教学材料后读取此引用"));
    }
    check_activity_source(state, &fact.binding)?;
    let delivery = if fact.kind == TeachingFact::MessageDelivered {
        fact.clone()
    } else {
        let id = fact.payload["delivery_ref"]
            .as_str()
            .ok_or_else(|| invalid("教学交付引用缺失"))?;
        event(state, id)?
    };
    Ok(format!("\nExplicit teaching reference, original ownership and immutable content/scene; page results are observations, not assessments. Reacquire source evidence for new claims. Previews may be truncated; tutor.step trace with event_id reads exact stored text in bounded chunks.\n{}", json!({"fact":trace_summary(&fact),"scene":preview(&fact.payload["scene"],4000),"delivery":preview(&delivered_text(&delivery),6000)})))
}

fn delivery(state: &PrivateBookContext<'_>, id: &str) -> Result<TeachingEvent, ToolError> {
    let saved = event(state, id)?;
    if saved.kind != TeachingFact::MessageDelivered
        || saved.binding.source_id != state.book.base.book_id
    {
        return Err(invalid("教学交付不属于当前材料"));
    }
    Ok(saved)
}
fn activity_status(state: &PrivateBookContext<'_>, saved: &TeachingEvent) -> Result<&'static str, ToolError> {
    let view = state.user.learning_store()?.state()?;
    if !view.control.enabled
        || view.control.current_tutor_session_id.as_deref() != Some(&saved.binding.tutor_session_id)
        || view
            .sessions
            .get(&saved.binding.tutor_session_id)
            .is_none_or(|s| s.status != TutorSessionStatus::Active)
    {
        return Ok("paused");
    }
    if check_activity_source(state, &saved.binding).is_err() {
        return Ok("unavailable");
    }
    Ok("active")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BehaviorRequest {
    operation_id: String,
    delivery_ref: String,
    action: String,
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    scene: Option<runtime::presentation::PresentationFollowUp>,
}

fn scene_snapshot(
    state: &PrivateBookContext<'_>,
    saved: &TeachingEvent,
    receipt: Option<&runtime::presentation::PresentationFollowUp>,
) -> Result<Value, ToolError> {
    match receipt {
        Some(receipt) => {
            if receipt.session_id != saved.binding.chat_session_id
                || receipt.turn_id != saved.binding.turn_id
                || serde_json::to_value(&receipt.reference).unwrap()
                    != saved.payload["move"]["presentation"]
            {
                return Err(invalid("现场不属于原教学活动版本"));
            }
            Ok(serde_json::to_value(state.read_presentation_state(receipt)?).unwrap())
        }
        None if !saved.payload["move"]["presentation"].is_null() => {
            Err(invalid("演示活动需要提交时的现场"))
        }
        None => Ok(Value::Null),
    }
}

fn behavior(state: &PrivateBookContext<'_>, request: BehaviorRequest, now: &str) -> Result<Value, ToolError> {
    let saved = delivery(state, &request.delivery_ref)?;
    let id = format!("action:{}", request.operation_id);
    let scene = scene_snapshot(state, &saved, request.scene.as_ref())?;
    let mut payload = json!({"delivery_ref":saved.event_id,"action":request.action,"response":request.response,"scene":scene,"assessment":"unassessed"});
    let old = state
        .user.learning_store()?
        .teaching_activity_events(&saved.event_id)?;
    if !old.iter().any(|e| e.event_id == id) {
        let actions = saved.payload["move"]["actions"]
            .as_array()
            .ok_or_else(|| invalid("此活动没有正式行为"))?;
        if !actions.iter().any(|a| a == &request.action) {
            return Err(invalid("此活动不支持该行为"));
        }
        if !old.iter().any(|e| e.kind == TeachingFact::Displayed) {
            return Err(invalid("教学活动尚未实际展示"));
        }
        if ["submit", "revise", "skip"].contains(&request.action.as_str())
            && activity_status(state, &saved)? != "active"
        {
            return Err(invalid("活动已暂停或来源不可用，请继续学习后提交"));
        }
        if ["submit", "revise", "self_report"].contains(&request.action.as_str())
            && request
                .response
                .as_deref()
                .is_none_or(|r| r.trim().is_empty())
        {
            return Err(invalid("请填写回答"));
        }
        let attempted = old.iter().any(|e| {
            e.kind == TeachingFact::LearnerAction
                && matches!(e.payload["action"].as_str(), Some("submit" | "revise"))
        });
        if (request.action == "revise" && !attempted) || (request.action == "submit" && attempted) {
            return Err(invalid("已有回答请使用改答；首次回答请使用提交"));
        }
    }
    // Help and attempt conditions are frozen at submission, not reconstructed after a reveal.
    if let Some(previous) = old.iter().find(|e| e.event_id == id) {
        payload["assistance_refs"] = previous.payload["assistance_refs"].clone();
        payload["attempt"] = previous.payload["attempt"].clone();
    } else {
        payload["assistance_refs"] = json!(old
            .iter()
            .filter(|e| e.kind == TeachingFact::HelpDisplayed)
            .map(|e| &e.event_id)
            .collect::<Vec<_>>());
        payload["attempt"] = json!(
            old.iter()
                .filter(|e| e.kind == TeachingFact::LearnerAction
                    && matches!(e.payload["action"].as_str(), Some("submit" | "revise")))
                .count()
                + usize::from(matches!(request.action.as_str(), "submit" | "revise"))
        );
    }
    let action = append(
        state,
        &saved.binding,
        id,
        TeachingFact::LearnerAction,
        vec![saved.event_id.clone()],
        payload,
        now,
    )?;
    let mut help = None;
    if matches!(request.action.as_str(), "request_hint" | "reveal") {
        let selected = event(state, &saved.causal_refs[0])?;
        let key = if request.action == "request_hint" {
            "hint"
        } else {
            "explanation"
        };
        let text = selected.payload["move"][key]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid("帮助内容当前不可用"))?;
        let offered = append(
            state,
            &saved.binding,
            format!("help:{}", request.operation_id),
            TeachingFact::HelpDelivered,
            vec![action.event_id.clone()],
            json!({"delivery_ref":saved.event_id,"action":request.action,"text":text}),
            now,
        )?;
        help = Some(json!({"event_id":offered.event_id,"text":text}));
    }
    let assessment = assess_action(state, &saved, &action).ok().flatten();
    if assessment.is_some() || request.action == "self_report" {
        refresh_learning(
            state,
            &saved.binding.tutor_session_id,
            &saved.binding.source_id,
        )?;
    }
    Ok(
        json!({"event_id":action.event_id,"assessment":assessment.as_ref().map(|a| &a.status).unwrap_or(&AssessmentStatus::Unassessed),"help":help}),
    )
}

fn frozen_contract(
    state: &PrivateBookContext<'_>,
    delivery: &TeachingEvent,
) -> Result<Option<(TeachingEvent, AssessmentContract)>, ToolError> {
    let Some(id) = delivery.payload["assessment_contract_ref"].as_str() else {
        return Ok(None);
    };
    let selected = event(state, id)?;
    let contract = serde_json::from_value(selected.payload["move"]["assessment"].clone())
        .map_err(|e| invalid(format!("评分合同不可读：{e}")))?;
    Ok(Some((selected, contract)))
}

fn assess_action(
    state: &PrivateBookContext<'_>,
    delivery: &TeachingEvent,
    action: &TeachingEvent,
) -> Result<Option<ResponseAssessment>, ToolError> {
    let mut store = state.user.learning_store()?;
    if let Some(old) = store.assessment(&action.event_id)? {
        return Ok(Some(old));
    }
    if !matches!(action.payload["action"].as_str(), Some("submit" | "revise")) {
        return Ok(None);
    }
    let Some((selected, contract)) = frozen_contract(state, delivery)? else {
        return Ok(None);
    };
    if matches!(contract.rule, ScoringRule::Open { .. }) {
        return Ok(None);
    }
    let status = memory::assessment::assess_closed(
        &contract,
        action.payload["response"].as_str().unwrap_or_default(),
    )?;
    store
        .save_assessment(&ResponseAssessment {
            action_ref: action.event_id.clone(),
            contract_ref: selected.event_id,
            evaluator_version: "closed.v1".into(),
            status,
            items: vec![],
        })
        .map(Some)
}

fn assessment_view(
    state: &PrivateBookContext<'_>,
    delivery: &TeachingEvent,
    facts: &[TeachingEvent],
) -> Result<Value, ToolError> {
    let Some(action) = facts.iter().rev().find(|e| {
        e.kind == TeachingFact::LearnerAction
            && matches!(e.payload["action"].as_str(), Some("submit" | "revise"))
    }) else {
        return Ok(Value::Null);
    };
    let assessment = assess_action(state, delivery, action)?;
    let reveal = frozen_contract(state, delivery)?
        .is_some_and(|(_, c)| c.feedback == FeedbackPolicy::AfterSubmit)
        || facts
            .iter()
            .any(|e| e.kind == TeachingFact::HelpDisplayed && e.payload["action"] == "reveal");
    Ok(
        json!({"action_ref":action.event_id,"status":assessment.as_ref().map(|a| &a.status).unwrap_or(&AssessmentStatus::Unassessed),
        "attempt":action.payload["attempt"],"assistance_count":action.payload["assistance_refs"].as_array().map_or(0,Vec::len),
        "items":if reveal { assessment.map(|a| a.items).unwrap_or_default() } else { vec![] }}),
    )
}

/// Private packet used only by the isolated evaluator; never returned by tutor.step or HTTP.
pub(crate) fn assessment_input(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    id: &str,
) -> Result<Value, ToolError> {
    let bind = bound(state, &turn.turn_id)?.ok_or_else(|| invalid("教学未开启"))?;
    if !current(state, &bind.binding)? {
        return Err(invalid("教学已暂停"));
    }
    let action = event(state, id)?;
    if action.binding.tutor_session_id != bind.binding.tutor_session_id
        || action.kind != TeachingFact::LearnerAction
        || !matches!(action.payload["action"].as_str(), Some("submit" | "revise"))
    {
        return Err(invalid("只能评估本会话正式提交"));
    }
    let saved = referenced_delivery(state, id)?;
    check_activity_source(state, &saved.binding)?;
    if let Some(old) = state.user.learning_store()?.assessment(id)? {
        return Ok(json!({"existing":old.status}));
    }
    let (selected, contract) =
        frozen_contract(state, &saved)?.ok_or_else(|| invalid("没有冻结评分合同"))?;
    if !matches!(contract.rule, ScoringRule::Open { .. }) {
        return Err(invalid("封闭题无需模型评估"));
    }
    Ok(
        json!({"contract":contract,"prompt":saved.payload["move"]["prompt"],"response":action.payload["response"],"sources":selected.payload["scoring_sources"]}),
    )
}

pub(crate) fn assessment_accept(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    id: &str,
    candidate: Value,
) -> Result<Value, ToolError> {
    let packet = assessment_input(state, turn, id)?;
    if !packet["existing"].is_null() {
        return Ok(json!({"assessment":packet["existing"]}));
    }
    let contract: AssessmentContract =
        serde_json::from_value(packet["contract"].clone()).map_err(|e| invalid(e.to_string()))?;
    let sources =
        serde_json::from_value(packet["sources"].clone()).map_err(|e| invalid(e.to_string()))?;
    let items: Vec<memory::assessment::ItemAssessment> =
        serde_json::from_value(candidate).map_err(|e| invalid(format!("评估格式不完整：{e}")))?;
    let status = memory::assessment::accept_open(
        &contract,
        packet["response"].as_str().unwrap_or_default(),
        &sources,
        &items,
    )?;
    let delivery = referenced_delivery(state, id)?;
    state
        .user.learning_store()?
        .save_assessment(&ResponseAssessment {
            action_ref: id.into(),
            contract_ref: delivery.payload["assessment_contract_ref"]
                .as_str()
                .unwrap()
                .into(),
            evaluator_version: "open.rubric.v1".into(),
            status: status.clone(),
            items,
        })?;
    refresh_learning(
        state,
        &delivery.binding.tutor_session_id,
        &delivery.binding.source_id,
    )?;
    // The ordinary assistant gets a status, not the hidden rubric or unrequested answer.
    let bind = bound(state, &turn.turn_id)?.ok_or_else(|| invalid("本轮教学绑定不存在"))?;
    let objects = active_objects(&turn_assets(state, &bind.binding)?);
    Ok(
        json!({"assessment":status,"action_ref":id,"learner_context":learner_context(state,&bind.binding,&objects,&[],Some(id))?}),
    )
}

fn refresh_learning(state: &PrivateBookContext<'_>, session: &str, source: &str) -> Result<(), ToolError> {
    let mut store = state.user.learning_store()?;
    store.derive_assessed_evidence(session)?;
    let (projection, watermark) = store.learner_projection(source)?;
    if projection.as_ref().is_none_or(|p| {
        p.evidence_watermark != watermark
            || p.estimator_version != memory::learning_evidence::ESTIMATOR_VERSION
    }) {
        store.rebuild_learner_projection(source)?;
    }
    Ok(())
}

fn source_ranges_overlap(state: &PrivateBookContext<'_>, a: &str, b: &str, c: &str, d: &str) -> bool {
    let position = |lid: &str| state.book.base.lid_nodes.iter().find(|n|n.lid==lid).map(|n|n.span.clone());
    match (position(a),position(b),position(c),position(d)) {
        (Some(a),Some(b),Some(c),Some(d)) => a.start < d.end && c.start < b.end,
        _ => false,
    }
}

fn accept_hypothesis(state: &PrivateBookContext<'_>, binding: &TeachingBinding, request: &Value, ranges: &[EvidenceRange]) -> Result<Value, ToolError> {
    use memory::learning_evidence::{EvidenceNature, LearningEvidence};
    let required = |key: &str| request[key].as_str().filter(|v|!v.trim().is_empty()).ok_or_else(||invalid(format!("缺少 {key}")));
    let target: memory::teaching::TeachingTarget = serde_json::from_value(request["target"].clone()).map_err(|e|invalid(e.to_string()))?;
    target.validate(binding)?;
    let objects=active_objects(&turn_assets(state,binding)?);
    if target.object_refs.iter().any(|r|!objects.iter().any(|o|o["ref"]["object_id"]==r.object_id && o["object_revision"].as_u64()==Some(r.object_revision))) {
        return Err(invalid("理解目标的对象修订不属于本轮资料"));
    }
    if !target.source_bindings.iter().all(|s|target_read_this_turn(state,&s.start_lid,&s.end_lid,ranges)) {
        return Err(invalid("理解解释的适用范围须有本轮原文读取依据"));
    }
    let fact_refs: Vec<String> = serde_json::from_value(request["fact_refs"].clone()).map_err(|e|invalid(e.to_string()))?;
    let mut quote = String::new();
    let mut assistance = Vec::new();
    for id in &fact_refs {
        let fact = event(state,id)?;
        if quote.is_empty() { quote = fact.payload["response"].as_str().unwrap_or_default().into(); }
        if fact.kind == TeachingFact::HelpDisplayed { assistance.push(id.clone()); }
        assistance.extend(fact.payload["assistance_refs"].as_array().into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)));
    }
    assistance.sort(); assistance.dedup();
    let evidence = LearningEvidence {
        evidence_id:format!("interpretation:{}",required("operation_id")?),
        action_ref:fact_refs.first().cloned().ok_or_else(||invalid("缺少使用事实依据"))?, delivery_ref:None,
        session_id:binding.tutor_session_id.clone(),source_id:binding.source_id.clone(),source_revision:binding.source_revision.clone(),map_revision:binding.map_revision.clone(),
        object_id:None,object_revision:None,nature:EvidenceNature::Hypothesis,
        label:target.learning_focus.clone(),capability:target.capability.clone(),target:Some(target),fact_refs,
        teaching_implication:required("teaching_implication")?.into(),
        assistance_refs:assistance,attempt:0,assessment_ref:None,status:AssessmentStatus::Unassessed,
        interpretation:required("interpretation")?.into(),learner_quote:quote,interpreter_version:"resident.hypothesis.v1".into(),
        supersedes:request["supersedes"].as_str().map(str::to_owned),correction:None,source_quotes:json!([]),
    };
    let accepted = state.user.learning_store()?.append_evidence(&evidence)?;
    refresh_learning(state,&binding.tutor_session_id,&binding.source_id)?;
    Ok(json!({"evidence_ref":accepted.evidence_id,"nature":accepted.nature,"teaching_implication":accepted.teaching_implication,
        "instruction":"Use this current interpretation when relevant to the move; record its reference in move.interpretation_refs. The frozen input remains unchanged."}))
}

fn accept_ungraded_evidence(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    request: &Value,
) -> Result<Value, ToolError> {
    let action = event(
        state,
        request["action_ref"]
            .as_str()
            .ok_or_else(|| invalid("缺少行为引用"))?,
    )?;
    if action.binding.tutor_session_id != binding.tutor_session_id
        || action.kind != TeachingFact::LearnerAction
        || !matches!(
            action.payload["action"].as_str(),
            Some("submit" | "revise" | "chat_response")
        )
    {
        return Err(invalid("没有可解释的正式表现"));
    }
    let delivery = referenced_delivery(state, &action.event_id)?;
    if !delivery.payload["assessment_contract_ref"].is_null() {
        return Err(invalid("有评分合同的表现应先按合同评估"));
    }
    // Only evidence discrimination has an independent, ungraded objective basis in v1.
    if delivery.payload["move"]["capability"] != "evaluation" {
        return Err(invalid("无评分的客观证据目前仅支持来源辨析"));
    }
    let quote = request["response_quote"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("缺少回答片段"))?;
    if !action.payload["response"]
        .as_str()
        .is_some_and(|r| r.contains(quote))
    {
        return Err(invalid("回答片段不属于原回应"));
    }
    let interpretation = request["interpretation"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("缺少解释"))?;
    let sources: Vec<memory::assessment::SourceQuote> =
        serde_json::from_value(request["sources"].clone()).map_err(|e| invalid(e.to_string()))?;
    if sources.is_empty() {
        return Err(invalid("来源辨析必须有原文依据"));
    }
    let map = turn_assets(state, &delivery.binding)?;
    let objects = active_objects(&map);
    let object_id = request["object_id"]
        .as_str()
        .ok_or_else(|| invalid("缺少正式对象"))?;
    let object = delivery.payload["object_versions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|o| o["ref"]["object_id"] == object_id)
        .ok_or_else(|| invalid("对象不属于此活动"))?;
    let formal = objects
        .iter()
        .find(|o| o["ref"]["object_id"] == object_id)
        .ok_or_else(|| invalid("原对象不可用"))?;
    for source in &sources {
        if source.quote.trim().is_empty()
            || !formal["source_bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|s| s["lid"] == source.lid)
            || !state.book
                .text(&source.lid, Some(&source.lid))?
                .contains(&source.quote)
        {
            return Err(invalid("辨析引用不属于原对象来源"));
        }
    }
    let supersedes = request["supersedes"].as_str().map(str::to_string);
    let evidence = memory::learning_evidence::LearningEvidence {
        evidence_id: format!(
            "interpretation:{}",
            request["operation_id"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| invalid("缺少操作身份"))?
        ),
        action_ref: action.event_id.clone(),
        delivery_ref: Some(delivery.event_id.clone()),
        session_id: delivery.binding.tutor_session_id.clone(),
        source_id: delivery.binding.source_id.clone(),
        source_revision: delivery.binding.source_revision.clone(),
        map_revision: delivery.binding.map_revision.clone(),
        object_id: Some(object_id.into()),
        object_revision: object["object_revision"].as_u64(),
        nature: memory::learning_evidence::EvidenceNature::Performance,
        target: serde_json::from_value(delivery.payload["move"].get("target").cloned().unwrap_or(Value::Null)).map_err(|e|invalid(e.to_string()))?,
        fact_refs: vec![action.event_id.clone(), delivery.event_id.clone()],
        teaching_implication: "按来源辨析表现选择后续支持".into(),
        label: object["meaning"].as_str().unwrap_or(object_id).into(),
        capability: "evaluation".into(),
        assistance_refs: serde_json::from_value(action.payload["assistance_refs"].clone())
            .unwrap_or_default(),
        attempt: action.payload["attempt"].as_u64().unwrap_or(1),
        assessment_ref: None,
        status: AssessmentStatus::Uncertain,
        interpretation: interpretation.into(),
        learner_quote: quote.into(),
        interpreter_version: "source.discrimination.v1".into(),
        supersedes,
        correction: None,
        source_quotes: json!(sources),
    };
    let accepted = state.user.learning_store()?.append_evidence(&evidence)?;
    refresh_learning(state, &binding.tutor_session_id, &binding.source_id)?;
    Ok(
        json!({"evidence_ref":accepted.evidence_id,"status":"observed_source_discrimination","mastery":"unknown"}),
    )
}

fn learner_context(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    objects: &[Value],
    focus: &[String],
    reference: Option<&str>,
) -> Result<Value, ToolError> {
    let mut targets = focus.to_vec();
    let mut current_target = Value::Null;
    if let Some(id) = reference {
        let delivery = referenced_delivery(state, id)?;
        if delivery.binding.tutor_session_id == binding.tutor_session_id {
            if delivery.binding.source_revision == binding.source_revision {
                current_target = delivery.payload["move"]["target"].clone();
            }
            targets.extend(
                delivery.payload["move"]["object_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str().map(str::to_string)),
            );
        }
    }
    if current_target.is_null() {
        current_target = state.user.learning_store()?.teaching_recent(&binding.tutor_session_id, 8)?
            .iter().find(|e| e.binding.source_id == binding.source_id && e.binding.source_revision == binding.source_revision
                && !e.payload["move"]["target"].is_null())
            .map(|e| e.payload["move"]["target"].clone()).unwrap_or(Value::Null);
    }
    if targets.is_empty() {
        let recent = state
            .user.learning_store()?
            .teaching_recent(&binding.tutor_session_id, 8)?;
        if let Some(last) = recent.iter().find(|e| {
            e.binding.source_id == binding.source_id && e.payload["move"]["object_ids"].is_array()
        }) {
            targets.extend(
                last.payload["move"]["object_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string)),
            );
        }
    }
    // No invented object target: first turn starts unknown until the Agent chooses one.
    let store = state.user.learning_store()?;
    let (projection, watermark) = store.learner_projection(&binding.source_id)?;
    let interpretations: Vec<_> = store.current_interpretations(&binding.source_id,&binding.source_revision,None,12)?.into_iter()
        .map(|(_,e)|json!({"evidence_ref":e.evidence_id,"nature":e.nature,"target":e.target,"interpretation":preview(&json!(e.interpretation),600),
            "teaching_implication":preview(&json!(e.teaching_implication),600),"learner_quote":preview(&json!(e.learner_quote),600),"correction":e.correction,
            "fact_refs":e.fact_refs,"status":e.status,"assistance_refs":e.assistance_refs,"attempt":e.attempt})).collect();
    let mut entries = vec![];
    let mut chars = 0;
    if let Some(p) = &projection {
        for row in p.rows.iter().filter(|r| {
            targets.contains(&r.object_id)
                && r.source_revision == binding.source_revision
                && objects.iter().any(|o| {
                    o["ref"]["object_id"] == r.object_id
                        && o["object_revision"].as_u64() == Some(r.object_revision)
                })
        }) {
            let latest = row
                .evidence_refs
                .last()
                .map(|id| store.evidence(id))
                .transpose()?;
            let entry = json!({"object_id":row.object_id,"object_revision":row.object_revision,"capability":row.capability,"state":row.state,
                "independent_support":row.independent_support,"assisted_support":row.assisted_support,"revised_support":row.revised_support,"partial":row.partial,"difficulty":row.difficulty,
                "latest_evidence_ref":row.evidence_refs.last(),"learner_quote":latest.as_ref().map(|e|preview(&json!(e.learner_quote),600)),"system_interpretation":latest.as_ref().map(|e|preview(&json!(e.interpretation),600)),"learner_correction":latest.as_ref().and_then(|e|e.correction.as_ref()).map(|s|preview(&json!(s),600))});
            let size = entry.to_string().chars().count();
            if entries.len() >= 6 || chars + size > 6000 {
                break;
            }
            chars += size;
            entries.push(entry);
        }
    }
    let paths:Vec<_>=projection.as_ref().into_iter().flat_map(|p|p.paths.values()).filter(|p|p["session_id"]==binding.tutor_session_id && p["source_revision"]==binding.source_revision && p["object_id"].as_str().is_some_and(|id|targets.iter().any(|t|t==id)) && objects.iter().any(|o|o["ref"]["object_id"]==p["object_id"] && o["object_revision"]==p["object_revision"]))
        .take(6).map(|p|json!({"object_id":p["object_id"],"capability":p["capability"],"last_status":p["last_status"],"assistance_count":p["assistance_refs"].as_array().map_or(0,Vec::len),"attempt":p["attempt"],"evidence_ref":p["evidence_ref"]})).collect();
    Ok(
        json!({"entries":entries,"interpretations":interpretations,"path_progress":paths,"current_target":current_target,"unknown_when_absent":true,"evidence_watermark":watermark,"projection_watermark":projection.as_ref().map(|p|p.evidence_watermark),
        "stale":projection.as_ref().is_none_or(|p|p.evidence_watermark!=watermark),"details":"tutor.step understanding with object_id and after"}),
    )
}

fn understanding(
    state: &PrivateBookContext<'_>,
    source: &str,
    object: Option<&str>,
    after: u64,
) -> Result<Value, ToolError> {
    let store = state.user.learning_store()?;
    let (projection, watermark) = store.learner_projection(source)?;
    let ready = crate::tutor_api::tutor_source_readiness(&state.book, &state.book_dir);
    let objects = if let Some(revision) = ready["teaching_assets"]["teaching_map_revision"]
        .as_str()
        .filter(|_| ready["status"] == "ready")
    {
        let map: Value = serde_json::from_slice(
            &std::fs::read(
                state.book_dir
                    .join("teaching/versions")
                    .join(revision)
                    .join("map.json"),
            )
            .map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
        active_objects(&map)
    } else {
        active_objects(&accepted_assets(state,&ready))
    };
    let mut all: Vec<_> = projection
        .as_ref()
        .into_iter()
        .flat_map(|p| &p.rows)
        .filter(|r| object.is_none_or(|id| r.object_id == id))
        .map(|r| {
            let mut v = serde_json::to_value(r).unwrap();
            v["evidence_count"] = json!(r.evidence_refs.len());
            v["evidence_refs"] = json!(r.evidence_refs.iter().rev().take(5).collect::<Vec<_>>());
            v["historical"] = json!(
                r.source_revision != state.book.source_fingerprint()
                    || !objects.iter().any(|o| o["ref"]["object_id"] == r.object_id
                        && o["object_revision"].as_u64() == Some(r.object_revision))
            );
            v
        })
        .collect();
    for o in objects
        .iter()
        .filter(|o| object.is_none_or(|id| o["ref"]["object_id"] == id))
    {
        if !all
            .iter()
            .any(|r| r["object_id"] == o["ref"]["object_id"] && r["historical"] == false)
        {
            all.push(json!({"object_id":o["ref"]["object_id"],"object_revision":o["object_revision"],"label":o["meaning"],"capability":"","state":"unknown","independent_support":0,"assisted_support":0,"revised_support":0,"partial":0,"difficulty":0,"uncertain":0,"evidence_count":0,"evidence_refs":[],"historical":false}));
        }
    }
    let total = all.len();
    let rows: Vec<_> = all.into_iter().skip(after as usize).take(12).collect();
    Ok(
        json!({"rows":rows,"interpretations":store.current_interpretations(source,state.book.source_fingerprint(),None,20)?.into_iter().map(|(_,e)|json!({"evidence_id":e.evidence_id,"nature":e.nature,"label":e.label,"interpretation":e.interpretation,"teaching_implication":e.teaching_implication,"correction":e.correction})).collect::<Vec<_>>(),"next":(after as usize+12<total).then_some(after+12),"evidence_watermark":watermark,"projection_watermark":projection.as_ref().map(|p|p.evidence_watermark),"stale":projection.as_ref().is_none_or(|p|p.evidence_watermark!=watermark),"estimator_version":projection.as_ref().map(|p|&p.estimator_version)}),
    )
}

fn evidence_view(state: &PrivateBookContext<'_>, id: &str) -> Result<Value, ToolError> {
    let evidence = state.user.learning_store()?.evidence(id)?;
    if evidence.source_id != state.book.base.book_id {
        return Err(invalid("请打开此证据的原材料"));
    }
    let Some(delivery_ref) = &evidence.delivery_ref else {
        let mut view = serde_json::to_value(&evidence).unwrap();
        view["prompt"] = json!(evidence.target.as_ref().map(|t|&t.expected_performance));
        view["assistance_count"] = json!(evidence.assistance_refs.len());
        view["feedback_hidden"] = json!(false);
        return Ok(view);
    };
    let delivery = delivery(state, delivery_ref)?;
    let facts = state
        .user.learning_store()?
        .teaching_activity_events(delivery_ref)?;
    let hidden = frozen_contract(state, &delivery)?
        .is_some_and(|(_, c)| c.feedback == FeedbackPolicy::OnReveal)
        && !facts
            .iter()
            .any(|e| e.kind == TeachingFact::HelpDisplayed && e.payload["action"] == "reveal");
    let mut view = serde_json::to_value(&evidence).unwrap();
    if hidden {
        view["source_quotes"] = Value::Null;
    }
    view["prompt"] = delivery.payload["move"]["prompt"].clone();
    view["assistance_count"] = json!(evidence.assistance_refs.len());
    view["feedback_hidden"] = json!(hidden);
    Ok(view)
}

pub(crate) fn route(state: &PrivateBookContext<'_>, req: Req) -> Reply {
    if req.method != "POST" {
        return agent_method_not_allowed();
    }
    let result = (|| -> Result<Value, ToolError> {
        let body: Value = serde_json::from_str(req.body).map_err(|e| invalid(e.to_string()))?;
        let string = |key: &str| {
            body[key]
                .as_str()
                .ok_or_else(|| invalid(format!("缺少 {key}")))
        };
        match req.url {
            "/tutor/understanding" => {
                if body["rebuild"] == true {
                    let mut store = state.user.learning_store()?;
                    let view = store.state()?;
                    for session in view.sessions.values().filter(|s| {
                        s.material_scope
                            .iter()
                            .any(|m| m.source_id == state.book.base.book_id)
                    }) {
                        store.derive_assessed_evidence(&session.id)?;
                    }
                    store.rebuild_learner_projection(&state.book.base.book_id)?;
                }
                understanding(
                    state,
                    &state.book.base.book_id,
                    body["object_id"].as_str(),
                    body["after"].as_u64().unwrap_or(0),
                )
            }
            "/tutor/evidence" => evidence_view(state, string("evidence_ref")?),
            "/tutor/feedback-displayed" => {
                let action = event(state, string("action_ref")?)?;
                let delivery = referenced_delivery(state, &action.event_id)?;
                let assessment = state
                    .user.learning_store()?
                    .assessment(&action.event_id)?
                    .ok_or_else(|| invalid("判定尚未接纳"))?;
                let facts = state
                    .user.learning_store()?
                    .teaching_activity_events(&delivery.event_id)?;
                let allowed = frozen_contract(state, &delivery)?
                    .is_some_and(|(_, c)| c.feedback == FeedbackPolicy::AfterSubmit)
                    || facts.iter().any(|e| {
                        e.kind == TeachingFact::HelpDisplayed && e.payload["action"] == "reveal"
                    });
                if !allowed || assessment.items.is_empty() {
                    return Err(invalid("此活动尚无可展示的详细反馈"));
                }
                let seen = append(
                    state,
                    &delivery.binding,
                    format!("feedback-seen:{}", action.event_id),
                    TeachingFact::HelpDisplayed,
                    vec![action.event_id.clone()],
                    json!({"delivery_ref":delivery.event_id,"action":"feedback","assessment_ref":action.event_id}),
                    req.now,
                )?;
                Ok(json!({"event_id":seen.event_id}))
            }
            "/tutor/evidence-list" => {
                let refs = state.user.learning_store()?.object_evidence_refs(
                    &state.book.base.book_id,
                    string("object_id")?,
                    body["object_revision"]
                        .as_u64()
                        .ok_or_else(|| invalid("缺少对象版本"))?,
                    string("capability")?,
                    body["before"].as_u64(),
                )?;
                Ok(
                    json!({"refs":refs.iter().map(|(_,id)|id).collect::<Vec<_>>(),"next":(refs.len()==10).then(||refs.last().unwrap().0)}),
                )
            }
            "/tutor/correct" => {
                let previous = state
                    .user.learning_store()?
                    .evidence(string("evidence_ref")?)?;
                if previous.source_id != state.book.base.book_id {
                    return Err(invalid("纠正不属于当前材料"));
                }
                let next = state.user.learning_store()?.correct_evidence(
                    &previous.evidence_id,
                    string("operation_id")?,
                    string("text")?,
                )?;
                refresh_learning(state, &next.session_id, &next.source_id)?;
                Ok(json!({"evidence_ref":next.evidence_id}))
            }
            "/tutor/start" => start_request(state, req.now),
            "/tutor/activities" => {
                let chat = string("session_id")?;
                let turn = string("turn_id")?;
                let reference: Option<runtime::presentation::PresentationRef> =
                    serde_json::from_value(body.get("reference").cloned().unwrap_or(Value::Null))
                        .map_err(|e| invalid(e.to_string()))?;
                // Reconcile a saved answer after a lost DB write before returning its deliveries.
                let turn_ref = AgentTurnRef {
                    session_id: chat.into(),
                    turn_id: turn.into(),
                    user_turn_ordinal: 0,
                };
                record_delivery(state, &turn_ref, req.now)?;
                let rows = state
                    .user.learning_store()?
                    .teaching_deliveries(chat, turn)?;
                let mut views = vec![];
                for saved in rows {
                    if saved.binding.source_id != state.book.base.book_id {
                        return Err(invalid("教学活动归属不符"));
                    }
                    if serde_json::to_value(&reference).unwrap()
                        != saved.payload["move"]["presentation"]
                    {
                        continue;
                    }
                    let facts = state
                        .user.learning_store()?
                        .teaching_activity_events(&saved.event_id)?;
                    views.push(json!({"delivery_ref":saved.event_id,"move":saved.payload["move"],"status":activity_status(state,&saved)?,
                        "assessment":assessment_view(state,&saved,&facts).unwrap_or(json!({"status":"unassessed"})),
                        "attempted":facts.iter().any(|e| e.kind == TeachingFact::LearnerAction && matches!(e.payload["action"].as_str(),Some("submit"|"revise"))),
                        "help_seen":facts.iter().filter(|e| e.kind == TeachingFact::HelpDisplayed).map(|e| &e.payload).collect::<Vec<_>>()}));
                }
                Ok(json!({"activities":views}))
            }
            "/tutor/display" => {
                let saved = delivery(state, string("delivery_ref")?)?;
                let receipt: Option<runtime::presentation::PresentationFollowUp> =
                    serde_json::from_value(body.get("scene").cloned().unwrap_or(Value::Null))
                        .map_err(|e| invalid(e.to_string()))?;
                let scene = scene_snapshot(state, &saved, receipt.as_ref())?;
                // First display per activity is immutable; reopening does not replace its scene.
                let id = format!("display:{}", saved.event_id);
                let previous = state
                    .user.learning_store()?
                    .teaching_activity_events(&saved.event_id)?
                    .into_iter()
                    .find(|e| e.event_id == id);
                let fact = match previous {
                    Some(e) => e,
                    None => append(
                        state,
                        &saved.binding,
                        id,
                        TeachingFact::Displayed,
                        vec![saved.event_id.clone()],
                        json!({"delivery_ref":saved.event_id,"scene":scene}),
                        req.now,
                    )?,
                };
                if let Some(original) = saved.payload["assistance_for"].as_str() {
                    append(
                        state,
                        &saved.binding,
                        format!("help-seen:{}", saved.event_id),
                        TeachingFact::HelpDisplayed,
                        vec![fact.event_id.clone(), original.into()],
                        json!({"delivery_ref":original,"action":"explanation","text":saved.payload["answer"],"reply_ref":saved.event_id}),
                        req.now,
                    )?;
                }
                Ok(json!({"event_id":fact.event_id}))
            }
            "/tutor/action" => behavior(
                state,
                serde_json::from_value(body).map_err(|e| invalid(e.to_string()))?,
                req.now,
            ),
            "/tutor/help-displayed" => {
                let help = event(state, string("help_ref")?)?;
                if help.kind != TeachingFact::HelpDelivered {
                    return Err(invalid("帮助尚未成功交付"));
                }
                let saved = delivery(
                    state,
                    help.payload["delivery_ref"]
                        .as_str()
                        .ok_or_else(|| invalid("帮助归属缺失"))?,
                )?;
                let seen = append(
                    state,
                    &saved.binding,
                    format!("seen:{}", help.event_id),
                    TeachingFact::HelpDisplayed,
                    vec![help.event_id.clone()],
                    help.payload.clone(),
                    req.now,
                )?;
                Ok(json!({"event_id":seen.event_id}))
            }
            "/tutor/reference" => {
                Ok(json!({"context":reference_context(state,string("event_id")?)?}))
            }
            _ => Err(invalid("未知教学接口")),
        }
    })();
    match result {
        Ok(value) => ok_json(&value),
        Err(e) => err_reply(&e),
    }
}
