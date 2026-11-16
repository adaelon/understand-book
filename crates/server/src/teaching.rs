//! Server-owned teaching delivery, cross-chat references and explicit learner actions.
use crate::*;
use memory::assessment::{
    AssessmentContract, AssessmentStatus, FeedbackPolicy, ResponseAssessment, ScoringRule,
};
use memory::learning::{
    TutorAction, TutorMaterial, TutorMaterialRole, TutorMutation, TutorSessionStatus,
};
use memory::teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact, TeachingTarget};
use runtime::tutor::TeachingMove;
use runtime::run_context::TeachingSelectionPreflightError;
mod design;
mod practice;
pub(crate) mod question;
mod continuity;
mod closed_book;
mod corrections;
mod grounding;
pub(crate) mod timeline;
mod page_help;
mod assessment_sources;
mod board_delivery;
mod native_board_selection;
#[cfg(test)]
#[path = "tests/native_board_selection_tests.rs"]
mod native_board_selection_tests;
pub(crate) use page_help::record_presentation_action;
pub(crate) use board_delivery::record_board_local_delivery;

fn preview(value: &Value, limit: usize) -> Value {
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    json!({"text":text.chars().take(limit).collect::<String>(),"truncated":text.chars().count()>limit})
}

fn workspace_goal_revision(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<Option<u32>, ToolError> {
    let admitted = state.user.agent_history.sessions.iter().find(|session|session.id==binding.chat_session_id)
        .and_then(|session|session.turns.iter().find(|turn|turn.turn_id==binding.turn_id))
        .and_then(|turn|turn.admission_input.as_ref().and_then(|input|input.learning_workspace_goal.as_ref())
            .or_else(||turn.domain.scene.as_ref().and_then(|scene|scene.workspace_goal.as_ref())))
        .map(|goal|goal.revision);
    if admitted.is_some() { return Ok(admitted); }
    if let Some(revision) = bound(state, &binding.turn_id)?.and_then(|event| event.payload["workspace_goal_revision"].as_u64()) {
        return Ok(Some(revision as u32));
    }
    let view=state.user.learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)?.state()?;
    Ok(view.sessions.get(&binding.tutor_session_id).and_then(|session|session.workspace_goal_revision)
        .or(view.workspace_goal_revision))
}
fn trace_summary(event: &TeachingEvent) -> Value {
    if event.kind == TeachingFact::AgentWorkReviewed {
        return json!({"event_id":event.event_id,"binding":event.binding,"kind":event.kind,
            "request":preview(&event.payload["original_request"],600),
            "decision":event.payload["review"]["decision"],"revision":preview(&event.payload["review"]["revision"],1000),
            "agent_work":event.payload["review"]["agent_work"],"occurred_at":event.occurred_at});
    }
    json!({"event_id":event.event_id,"binding":event.binding,"kind":event.kind,"causal_refs":event.causal_refs,
        "action":event.payload["action"],"attempt":event.payload["attempt"],"assistance_count":event.payload["assistance_refs"].as_array().map_or(0,Vec::len),"response_preview":preview(&event.payload["response"],600),"move_id":event.payload["move"]["move_id"],
        "prompt_preview":preview(&event.payload["move"]["prompt"],600),"target":event.payload["move"]["target"],
        "design_ref":event.payload["move"]["design_ref"],"block_refs":event.payload["move"]["block_refs"],
        "responds_to":event.payload["move"]["responds_to"],"response_expectation":event.payload["move"]["response_expectation"],
        "invitation_context_ref":event.payload["invitation_context_ref"],"support_delivery_refs":event.payload["support_delivery_refs"],
        "response_origin":event.payload["response_origin"],
        "evidence_ref":event.payload["evidence_ref"],
        "presentation_follow_up":event.payload["presentation_follow_up"],
        "scene_receipt":event.payload["scene"]["receipt"],
        "scene_visible_step":event.payload["scene"]["state"]["visible_step"],
        "answer_preview":preview(&event.payload["answer"],1200),"help_preview":preview(&event.payload["text"],600),
        "original_ref":event.payload["original_ref"],"usage":event.payload["usage"],
        "interaction":event.payload.get("interaction").filter(|v|v.is_object()).map(|v|json!({
            "action":v["action"],"accepted_request":preview(&v["accepted_request"],600),"answer_ref":v["answer_ref"],
            "answer_preview":preview(&v["answer_text"],600),"answer_origin":"assistant","source_bindings":v["source_bindings"]})),
        "continuation":matches!(event.kind,TeachingFact::ContinuationDisplayed|TeachingFact::ContinuationChosen).then_some(&event.payload),
        "occurred_at":event.occurred_at})
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
    if fact.kind == TeachingFact::LearnerAction {
        value["assessment"] = json!(state.user.learning_store_for_workspace(state.learning_workspace_id)?
            .assessment(&fact.event_id)?);
    }
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
    }
    // The input receipt identifies the observation made at that time. Never use
    // the page's latest state when reconstructing a learner expression.
    if !fact.payload["presentation_follow_up"].is_null() {
        let receipt = serde_json::from_value(fact.payload["presentation_follow_up"].clone()).map_err(|e|invalid(e.to_string()))?;
        value["scene"] = serde_json::to_value(state.read_presentation_state(&receipt)?).map_err(|e|invalid(e.to_string()))?;
    } else if fact.payload["scene"].is_object() {
        value["scene"] = fact.payload["scene"].clone();
    }
    Ok(value)
}

fn event(state: &PrivateBookContext<'_>, id: &str) -> Result<TeachingEvent, ToolError> {
    state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_event(id)
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
    append_user(state.user, state.learning_workspace_id, binding, id, kind, causes, payload, now)
}
fn append_user(
    user: &crate::user_runtime::UserRuntime,
    learning_workspace_id: Option<&str>,
    binding: &TeachingBinding,
    id: String,
    kind: TeachingFact,
    causes: Vec<String>,
    payload: Value,
    now: &str,
) -> Result<TeachingEvent, ToolError> {
    user.learning_store_for_workspace(learning_workspace_id)?.append_teaching(&TeachingEvent {
            event_id: id,
            binding: binding.clone(),
            kind,
            causal_refs: causes,
            payload,
            occurred_at: now.into(),
        })
}
fn bound(state: &PrivateBookContext<'_>, turn: &str) -> Result<Option<TeachingEvent>, ToolError> {
    bound_user(state.user, state.learning_workspace_id, turn)
}
pub(crate) fn bound_user(user: &crate::user_runtime::UserRuntime, learning_workspace_id: Option<&str>, turn: &str) -> Result<Option<TeachingEvent>, ToolError> {
    match user.learning_store_for_workspace(learning_workspace_id)?.teaching_event(&format!("binding:{turn}")) {
        Ok(event) => Ok(Some(event)),
        Err(e) if e.error_code == "TEACHING_INVALID" => Ok(None),
        Err(e) => Err(e),
    }
}

/// The runtime supplies an actual completed review; the host owns its scope.
/// Saving happens before either accepting or returning a revision request.
pub(crate) fn record_agent_review(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, record: Value) -> Result<Value, ToolError> {
    let bound = bound(state,&turn.turn_id)?.ok_or_else(||invalid("Agent review has no bound teaching turn"))?;
    if bound.binding.chat_session_id != turn.session_id {
        return Err(invalid("Agent review does not belong to this chat"));
    }
    let mut store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    store.append_agent_work_review(&bound.binding,&bound.event_id,record,&crate::multi_user_host::now().to_string())?;
    store.agent_working_state(&bound.binding)
}
pub(crate) fn agent_working_state(state: &PrivateBookContext<'_>, turn: &AgentTurnRef) -> Result<Value, ToolError> {
    let Some(bound)=bound(state,&turn.turn_id)? else { return Ok(Value::Null); };
    state.user.learning_store_for_workspace(state.learning_workspace_id)?.agent_working_state(&bound.binding)
}
fn current(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<bool, ToolError> {
    if closed_book::active(state, &binding.chat_session_id)?.is_some() { return Ok(true); }
    let Some(turn) = bound(state, &binding.turn_id)? else { return Ok(false); };
    Ok(turn.payload["status"] == "practice" && check_source(state, binding).is_ok()
        && workspace_goal_revision(state, binding)? == state.user.learning_store_for_workspace(state.learning_workspace_id)?.state()?.workspace_goal_revision)
}
pub(crate) fn turn_active(state: &PrivateBookContext<'_>, turn: &str) -> Result<bool, ToolError> {
    match bound(state, turn)? {
        Some(event) if event.payload["inquiry_course"].is_object() => {
            let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
            let run = store.course_run(&event.binding.tutor_session_id)?;
            Ok(run.progress.status == memory::course::CourseStatus::Active
                && store.state()?.workspace_goal_revision.is_none_or(|revision| revision == run.progress.goal_revision))
        }
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

pub(crate) fn missing_target_read(book: &Book, start: &str, end: &str, ranges: &[EvidenceRange]) -> Result<Option<(EvidenceRange, usize)>, ToolError> {
    let nodes = &book.base.lid_nodes;
    let node = |lid: &str| nodes.iter().find(|n| n.lid == lid);
    let (Some(first), Some(last)) = (node(start), node(end)) else {
        return Err(invalid(format!("目标原文位置不存在：{start}..{end}")));
    };
    if first.span.start > last.span.start || first.span.start >= last.span.end {
        return Err(invalid(format!("目标原文范围顺序无效：{start}..{end}")));
    }
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
    let source: Vec<u16> = book.text(start, Some(end))?.encode_utf16().collect();
    let check_gap = |a: usize, b: usize| -> Option<(EvidenceRange, usize)> {
        let gap = &source[a - first.span.start..b - first.span.start];
        if let Some(offset) = gap.iter().position(|unit|
            !char::from_u32(u32::from(*unit)).is_some_and(char::is_whitespace))
        {
            let position = a + offset;
            let missing = nodes.iter().filter(|n| n.span.start <= position && position < n.span.end)
                .min_by_key(|n| (n.span.end - n.span.start, std::cmp::Reverse(n.path.len())))
                .map(|n| n.lid.as_str()).unwrap_or(start);
            return Some((EvidenceRange { start_lid: missing.into(), end_lid: missing.into(), ranges: Vec::new() }, position));
        }
        None
    };
    let mut covered = first.span.start;
    for (a, b) in reads {
        if b <= covered || a >= last.span.end { continue; }
        // Adjacent paragraph LIDs can leave source separators outside both spans.
        // Only whitespace may bridge a gap; unread body text still requires a read.
        if a > covered { if let Some(gap) = check_gap(covered, a) { return Ok(Some(gap)); } }
        covered = covered.max(b).min(last.span.end);
        if covered >= last.span.end { return Ok(None); }
    }
    Ok(check_gap(covered, last.span.end))
}


fn preflight_target_read(state: &PrivateBookContext<'_>, start: &str, end: &str, ranges: &[EvidenceRange]) -> Result<(), TeachingSelectionPreflightError> {
    if let Some((missing, position)) = missing_target_read(state.book, start, end, ranges)? {
        return Err(TeachingSelectionPreflightError {
            error: invalid(format!("本轮尚未读取目标 {start}..{end} 的正文：LID {}，原文 UTF-16 位置 {position}；请用 book.text 读取缺失段落", missing.start_lid)),
            missing_source_read: Some(missing),
        });
    }
    Ok(())
}

fn require_target_read_this_turn(state: &PrivateBookContext<'_>, start: &str, end: &str, ranges: &[EvidenceRange]) -> Result<(), ToolError> {
    preflight_target_read(state, start, end, ranges).map_err(|failure| failure.error)
}

#[cfg(test)]
#[path = "tests/music_teaching_target_tests.rs"]
mod music_target_tests;
#[cfg(test)]
#[path = "tests/video_teaching_target_tests.rs"]
mod video_target_tests;

fn bind_target(mut input: Value, binding: &TeachingBinding, evidence: &[SourceBinding]) -> Result<memory::teaching::TeachingTarget, ToolError> {
    let source_refs: Vec<String> = input.as_object_mut().and_then(|fields| fields.remove("source_ref_ids"))
        .map(serde_json::from_value).transpose().map_err(|e| invalid(format!("教学来源引用无效：{e}")))?.unwrap_or_default();
    let matching_source = |source: &SourceBinding| source.book_id == binding.source_id
        && source.source_revision.as_deref() == Some(binding.source_revision.as_str());
    let mut media: Vec<memory::teaching::TeachingMediaBinding> = input.as_object_mut().and_then(|fields| fields.remove("media_bindings"))
        .filter(|value| !value.is_null()).map(serde_json::from_value).transpose()
        .map_err(|e| invalid(format!("视频来源绑定无效：{e}")))?.unwrap_or_default();
    let mut music: Vec<memory::teaching::TeachingMusicBinding> = input.as_object_mut().and_then(|fields| fields.remove("music_bindings"))
        .filter(|value| !value.is_null()).map(serde_json::from_value).transpose()
        .map_err(|e| invalid(format!("音乐来源绑定无效：{e}")))?.unwrap_or_default();
    // Preflight returns normalized targets to the runtime reviewer. Re-check those
    // exact bindings against the same turn ledger when the reviewed request commits.
    for saved in &media {
        if saved.source_id != binding.source_id || saved.source_revision != binding.source_revision
            || !evidence.iter().any(|source| matching_source(source) && source.media.as_ref().is_some_and(|m| m.target == saved.target)) {
            return Err(invalid("视频教学来源必须来自本轮已读并登记的 source.present 引用"));
        }
    }
    for saved in &music {
        if saved.source_id != binding.source_id || saved.source_revision != binding.source_revision
            || !evidence.iter().any(|source| matching_source(source) && source.music.as_ref().is_some_and(|m| m.target == saved.target)) {
            return Err(invalid("音乐教学来源必须来自本轮已读并登记的 source.present 引用"));
        }
    }
    for id in source_refs {
        let source = evidence.iter().find(|source| source.source_ref_id == id && matching_source(source))
            .ok_or_else(|| invalid("教学目标的 source_ref_ids 未在本轮读取，或不属于本轮资料版本"))?;
        if let Some(snapshot) = &source.media {
            if !media.iter().any(|saved| saved.target == snapshot.target) {
                media.push(memory::teaching::TeachingMediaBinding { source_id: binding.source_id.clone(), source_revision: binding.source_revision.clone(), target: snapshot.target.clone() });
            }
        } else if let Some(snapshot) = &source.music {
            if !music.iter().any(|saved| saved.target == snapshot.target) {
                music.push(memory::teaching::TeachingMusicBinding { source_id: binding.source_id.clone(), source_revision: binding.source_revision.clone(), target: snapshot.target.clone() });
            }
        } else {
            return Err(invalid("source_ref_ids 需要视频或音乐来源；书籍正文请使用 source_bindings"));
        }
    }
    if !media.is_empty() { input["media_bindings"] = json!(media); }
    if !music.is_empty() { input["music_bindings"] = json!(music); }
    for source in input["source_bindings"].as_array_mut().into_iter().flatten() {
        if let Some(fields) = source.as_object_mut() {
            for (key, expected) in [("source_id", &binding.source_id), ("source_revision", &binding.source_revision)] {
                if let Some(actual) = fields.get(key) {
                    if actual.as_str() != Some(expected.as_str()) {
                        return Err(invalid(format!("教学目标 {key} 与本轮绑定不一致；请省略 source_id/source_revision，由运行时绑定当前原文。搜索结果的 source_revision 是检索索引版本，不能用作教学原文版本")));
                    }
                } else {
                    fields.insert(key.into(), json!(expected));
                }
            }
        }
    }
    let target: memory::teaching::TeachingTarget = serde_json::from_value(input).map_err(|e| invalid(e.to_string()))?;
    target.validate(binding)?;
    Ok(target)
}

/// Re-reading audio creates a new review receipt for the same original clip.
/// The inherited target must retain its source scope but use this turn's receipt.
fn same_media_scope(a: &base_schema::EvidenceTarget, b: &base_schema::EvidenceTarget) -> bool {
    use base_schema::EvidenceTarget;
    match (a, b) {
        (EvidenceTarget::Audio { publication: ap, location: al, .. }, EvidenceTarget::Audio { publication: bp, location: bl, .. }) => ap == bp && al == bl,
        (EvidenceTarget::Joint { members: a }, EvidenceTarget::Joint { members: b }) => a.len() == b.len()
            && a.iter().zip(b).all(|(a,b)| a.support_role == b.support_role && same_media_scope(&a.target,&b.target)),
        _ => a == b,
    }
}

fn refresh_inherited_media(target: &mut TeachingTarget, binding: &TeachingBinding, evidence: &[SourceBinding]) -> Result<(), ToolError> {
    for saved in target.media_bindings.iter_mut().flatten() {
        let current = evidence.iter().filter(|source| source.book_id == binding.source_id
            && source.source_revision.as_deref() == Some(binding.source_revision.as_str()))
            .filter_map(|source| source.media.as_ref()).find(|read| same_media_scope(&saved.target, &read.target))
            .ok_or_else(|| invalid("本轮尚未重新读取继承的视频目标；请按原范围读取并登记 source.present 来源"))?;
        saved.target = current.target.clone();
    }
    Ok(())
}

fn validate_move_intent(movement: &TeachingMove, context: &Value) -> Result<(), ToolError> {
    if !["direct_explanation", "guided_inquiry", "neutral"].contains(&movement.interaction_intent.as_str()) {
        return Err(invalid("move.interaction_intent 无效：只能使用 direct_explanation、guided_inquiry 或 neutral"));
    }
    let neutral_repair = "若没有明确教学方式，使用 interaction_intent=neutral、intent_origin=neutral；讲解动作仍可使用 kind=explain，并按已保存学习目标提供帮助";
    match movement.intent_origin.as_str() {
        "current_request" => {
            let quote = movement.intent_quote.as_deref().filter(|quote| !quote.trim().is_empty())
                .ok_or_else(|| invalid("move.intent_quote 缺失：intent_origin 为 current_request 时，请引用当前请求中的非空原话后重新 select"))?;
            if !context["current_request"].as_str().is_some_and(|request| request.contains(quote)) {
                return Err(invalid(format!("move.intent_quote 不属于本轮 current_request，请复制当前请求原话；不要把历史目标或界面按钮当成当前原话。{neutral_repair}")));
            }
        }
        "current_action" => {
            if movement.intent_quote.is_some() {
                return Err(invalid("intent_origin=current_action 由冻结的界面操作决定；请删除 intent_quote，界面操作不是用户原话"));
            }
            if !serde_json::from_value::<runtime::tutor_continuation::ResolvedInteraction>(context["current_action"].clone())
                .is_ok_and(|action| action.validates_current_action(&movement.interaction_intent, &movement.kind)) {
                return Err(invalid(format!("当前 current_action 不支持这组 interaction_intent/kind；请按冻结的界面操作选择。{neutral_repair}")));
            }
        }
        "session_default" => {
            let Some(expected) = context["default_teaching_intent"].as_str() else {
                return Err(invalid(format!("当前 default_teaching_intent=null，没有会话默认教学方式，不能使用 intent_origin=session_default；已保存学习目标不等于默认教学方式。{neutral_repair}")));
            };
            if movement.interaction_intent != expected {
                return Err(invalid(format!("intent_origin=session_default 必须使用 interaction_intent={expected}；本轮明确原话若覆盖默认方式，请改用 current_request 并提供确切 intent_quote")));
            }
        }
        "confirmed_preference" => {
            if !context["default_teaching_intent"].is_null() {
                return Err(invalid("本轮已有 default_teaching_intent，会话默认优先于偏好；请用 session_default 及对应方式，或用当前明确原话和 current_request"));
            }
            if !context["confirmed_preferences"].as_array().into_iter().flatten()
                .any(|preference| preference["fact_id"].as_str() == movement.preference_ref.as_deref()) {
                return Err(invalid(format!("move.preference_ref 不属于本轮 confirmed_preferences，请引用真实 fact_id。{neutral_repair}")));
            }
        }
        "neutral" => {}
        _ => return Err(invalid("move.intent_origin 无效：请从当前工具合同列出的真实意图来源中选择")),
    }
    if movement.interaction_intent == "direct_explanation" && movement.kind != "explain" {
        return Err(invalid("interaction_intent=direct_explanation 要求 kind=explain；请直接解释当前请求，不新增作答任务"));
    }
    Ok(())
}
pub(crate) fn active_objects(map: &Value) -> Vec<Value> {
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
    let mut store = state.user.learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)?;
    let mut view = store.state()?;
    if ready["status"] != "ready" {
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
                    learning_purpose: None,
                },
            },
            now,
        )?;
    }
    Ok(
        json!({"started":true,"state":view,"message":"请结合我的学习目标、当前阅读位置和之前的对话，接着帮我理解。"}),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FrozenTeachingPreparation {
    #[serde(default)]
    pub learning_workspace_id: Option<String>,
    pub context: Option<Value>,
    pub events: Vec<TeachingEvent>,
}
impl FrozenTeachingPreparation {
    /// append_teaching compares the complete payload and reuses the original receipt.
    pub(crate) fn resume(&self, user: &crate::user_runtime::UserRuntime) -> Result<(), ToolError> {
        let mut learning = user.learning_store_for_workspace(self.learning_workspace_id.as_deref())?;
        for event in &self.events { learning.append_teaching(event)?; }
        Ok(())
    }
}
pub(crate) fn prepare(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, request: &Value, now: &str) -> Result<Option<Value>, ToolError> {
    let interaction = state.user.agent_history.sessions.iter().find(|s| s.id == turn.session_id)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == turn.turn_id))
        .and_then(|t| t.domain.interaction.as_ref());
    let frozen = freeze_prepare(state, turn, request, interaction, now)?;
    frozen.resume(state.user)?;
    crate::teaching_coach::ensure_turn(state.user, state.learning_workspace_id, turn, frozen.context.as_ref(), now)?;
    Ok(frozen.context)
}

pub(crate) fn freeze_prepare(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    request: &Value,
    interaction: Option<&runtime::tutor_continuation::ResolvedInteraction>,
    now: &str,
) -> Result<FrozenTeachingPreparation, ToolError> {
    let mut events = Vec::new();
    if let Some(id) = request["teaching_ref"].as_str() {
        let original = referenced_delivery(state, id)?;
        let prior = state
            .user.learning_store_for_workspace(state.learning_workspace_id)?
            .teaching_activity_events(&original.event_id)?;
        let mut binding = original.binding.clone();
        binding.chat_session_id = turn.session_id.clone();
        binding.turn_id = turn.turn_id.clone();
        events.push(TeachingEvent {
            event_id: format!("response:{}", turn.turn_id),
            binding,
            kind: TeachingFact::LearnerAction,
            causal_refs: vec![id.into(), original.event_id.clone()],
            payload: json!({"delivery_ref":original.event_id,"action":"chat_response","response":request["message"],"response_origin":request["teaching_response_origin"],"reference":id,"assessment":"unassessed",
                "assistance_refs":prior.iter().filter(|e|e.kind==TeachingFact::HelpDisplayed).map(|e|&e.event_id).collect::<Vec<_>>(),
                "attempt":prior.iter().filter(|e|e.kind==TeachingFact::LearnerAction && matches!(e.payload["action"].as_str(),Some("submit"|"revise"|"chat_response"))).count()+1}),
            occurred_at: now.into(),
        });
    }
    prepare_observation(state, turn, request, interaction, now, events)
}

/// Ordinary use has an observation scope, not a persisted/active TutorSession.
fn prepare_observation(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, request: &Value,
    interaction: Option<&runtime::tutor_continuation::ResolvedInteraction>, now: &str,
    mut events: Vec<TeachingEvent>) -> Result<FrozenTeachingPreparation, ToolError> {
    let course = request["course_context"].as_object().map(|_| &request["course_context"]["lesson"]);
    let response_ref = request["teaching_ref"].as_str().or_else(|| interaction.and_then(|action| match &action.action {
        runtime::tutor_continuation::InteractionAction::UiChoice { teaching_choice, .. } => Some(teaching_choice.delivery_ref.as_str()),
        _ => None,
    }));
    let response = response_ref.map(|id| referenced_delivery(state, id)).transpose()?;
    let practice_response = match response.as_ref() {
        Some(delivery) if delivery.payload["move"]["practice"].is_object()
            || frozen_contract(state, delivery)?.is_some()
            || bound(state, &delivery.binding.turn_id)?.is_some_and(|turn| turn.payload["status"] == "practice") => Some(delivery),
        _ => None,
    };
    let practice = course.is_none() && (interaction.is_some_and(|action| action.requests_practice()) || practice_response.is_some());
    let ready = crate::tutor_api::tutor_source_readiness(&state.book,&state.book_dir);
    let closed = closed_book::active(state, &turn.session_id)?;
    let binding = TeachingBinding {
        tutor_session_id:closed.as_ref().map(|v|v.owner_id.clone()).or_else(||course.and_then(|lesson|lesson["progress"]["run_id"].as_str()).map(str::to_owned))
            .unwrap_or_else(|| practice_response.map(|delivery| delivery.binding.tutor_session_id.clone())
                .unwrap_or_else(|| if practice { format!("practice:{}", turn.session_id) } else { format!("observation:{}", state.book.base.book_id) })),
        session_revision:course.and_then(|lesson|lesson["progress"]["revision"].as_u64()).unwrap_or(0) as u32, control_revision:0,
        source_id:state.book.base.book_id.clone(), source_revision:state.book.source_fingerprint().into(),
        map_revision:practice.then(|| ready["teaching_assets"]["teaching_map_revision"].as_str().map(str::to_owned)).flatten(),
        chat_session_id:turn.session_id.clone(), turn_id:turn.turn_id.clone(),
    };
    let assets = if binding.map_revision.is_some() { read_map(state, &binding)? } else { accepted_assets(state,&ready) };
    let objects = active_objects(&assets);
    events.push(TeachingEvent { event_id:format!("assets:{}",turn.turn_id), binding:binding.clone(),kind:TeachingFact::TurnBound,
        causal_refs:vec![],payload:assets,occurred_at:now.into() });
    events.extend(usage_facts(state,&binding,request,interaction,now)?);
    design::freeze_input_support(state,&binding,&mut events,request,interaction)?;
    let mut recent = state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_recent(&binding.tutor_session_id,8)?;
    for fact in events.iter().filter(|e| matches!(e.kind, TeachingFact::UsageObserved | TeachingFact::LearnerAction)) { recent.insert(0,fact.clone()); }
    recent.truncate(8);
    let mut learner = learner_context(state,&binding,&objects,&[],request["teaching_ref"].as_str(),&events)?;
    let confirmed:Vec<_>=state.user.store.resolve_profile_facts(&ProfileResolutionContext {
        book_id:Some(binding.source_id.clone()), learning_workspace_id:state.learning_workspace_id.map(str::to_owned),
        content_profile:Some(current_content_profile(&state.book).into()), now:Some(now.into()), ..Default::default()
    }).into_iter().filter(|fact|fact.status==memory::FactStatus::Confirmed).take(16).collect();
    let preferences:Vec<_>=confirmed.iter().filter(|fact|matches!(fact.payload,memory::ProfilePayload::ExplanationPreference(_)))
        .map(|fact|json!({"fact_id":fact.fact_id,"preference":fact.payload})).collect();
    learner["confirmed_preferences"]=json!(preferences);
    learner["confirmed_background"]=json!(confirmed.iter().filter(|fact|matches!(fact.payload,memory::ProfilePayload::Background(_) | memory::ProfilePayload::Capability(_)))
        .map(|fact|json!({"fact_id":fact.fact_id,"claim":preview(&json!(fact.payload),1000)})).collect::<Vec<_>>());
    if let Some(original) = practice_response {
        let check = state.user.learning_store_for_workspace(state.learning_workspace_id)?.practice_check_verdict(&original.event_id)?;
        learner["responding_to_practice"] = json!({"delivery_ref":original.event_id,"practice":original.payload["move"]["practice"],"check":check});
    }
    if practice { learner["practice_history"] = practice::history_view(state)?; }
    learner["current_request"] = request["message"].clone();
    learner["recent_facts"] = json!(recent.iter().map(trace_summary).collect::<Vec<_>>());
    learner["uninterpreted_fact_refs"] = uninterpreted_facts(state, &binding, &recent)?;
    let interpretation_refs=learner["interpretations"].as_array().into_iter().flatten()
        .filter_map(|item|item["evidence_ref"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    learner["learner_grounding"]=grounding::context(state,&binding,&learner["current_design"]["design"],&[],&events,&interpretation_refs)?;
    // A prepared course owns its teaching lifecycle. Its run is already the
    // lesson plan and trace owner; never create a competing generic TutorSession.
    let goal_revision = state.user.learning_store_for_workspace(state.learning_workspace_id)?.state()?.workspace_goal_revision;
    let course_active = course.is_some_and(|lesson| lesson["progress"]["status"] == "active"
        && goal_revision.is_none_or(|revision|lesson["progress"]["goal_revision"] == revision));
    let mut context = json!({"status":if course_active {"course"} else if practice {"practice"} else {"observing"},"binding":binding,"learner_context":learner,"readiness":if course.is_some(){json!("ready")}else{ready["status"].clone()},
        "agent_working_state":state.user.learning_store_for_workspace(state.learning_workspace_id)?.agent_working_state(&binding)?,
        "teacher_corrections":state.user.learning_store_for_workspace(state.learning_workspace_id)?.teacher_correction_hint(&binding)?,
        "reason":if course.is_some(){json!("已载入当前课程，沿用本课目标与学习记录")}else{ready["reason"].clone()},"reference":request["teaching_ref"]});
    if practice {
        context["current_design"] = design::current_design(state, &binding)?;
        context["teaching_assets"] = ready["teaching_assets"].clone();
        context["objects"] = json!(objects.iter().take(16).collect::<Vec<_>>());
    }
    let view = state.user.learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)?.state()?;
    context["confirmed_preferences"] = json!(preferences);
    context["object_count"] = json!(objects.len());
    context["current_request"] = request["message"].clone();
    context["workspace_goal_revision"] = json!(view.workspace_goal_revision);
    if let Some(goal) = view.control.current_tutor_session_id.as_ref().and_then(|id| view.sessions.get(id))
        .filter(|goal| goal.status != TutorSessionStatus::Ended) {
        context["user_intent"] = json!(goal.user_intent);
        context["constraints"] = json!(goal.explicit_constraints);
        context["learner_context"]["user_intent"] = json!(goal.user_intent);
        context["learner_context"]["constraints"] = json!(goal.explicit_constraints);
    }
    if let Some(course) = course {
        context["workspace_goal_revision"] = course["progress"]["goal_revision"].clone();
        context["user_intent"] = course["summary"]["objective"].clone();
    }
    if let Some(closed) = closed {
        let question=closed.visible_turn_ids.first().and_then(|id|state.user.agent_history.sessions.iter()
            .find(|session|session.id==turn.session_id).and_then(|session|session.turns.iter().find(|item|&item.turn_id==id)))
            .and_then(|item|item.outcome.as_ref()).and_then(|outcome|outcome.answer.as_ref());
        context["closed_book"] = json!(closed);
        context["closed_book"]["current_question"]=json!(question);
    }
    attach_current_input(interaction, request, &mut context);
    events.push(TeachingEvent { event_id:format!("binding:{}",turn.turn_id),binding,kind:TeachingFact::TurnBound,
        causal_refs:vec![],payload:context.clone(),occurred_at:now.into() });
    Ok(FrozenTeachingPreparation { learning_workspace_id: state.learning_workspace_id.map(str::to_owned), context:Some(context),events })
}

fn uninterpreted_facts(state: &PrivateBookContext<'_>, binding: &TeachingBinding, recent: &[TeachingEvent]) -> Result<Value, ToolError> {
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    Ok(json!(recent.iter().filter(|e| e.binding.source_id == binding.source_id
        && e.binding.source_revision == binding.source_revision && e.learner_expression().is_some())
        .map(|e|store.fact_has_interpretation(&e.event_id).map(|seen|(!seen).then_some(e.event_id.clone())))
        .collect::<Result<Vec<_>,ToolError>>()?.into_iter().flatten().collect::<Vec<_>>()))
}

fn attach_current_input(interaction: Option<&runtime::tutor_continuation::ResolvedInteraction>, request: &Value, context: &mut Value) {
    if request["course_context"].is_object() { context["inquiry_course"] = request["course_context"].clone(); }
    if request["knowledge_focus"].is_object() {context["knowledge_focus"]=request["knowledge_focus"].clone();}
    if request["socratic_dialogue"].is_object() {
        context["socratic_dialogue"] = request["socratic_dialogue"].clone();
    }
    if let Some(interaction) = interaction {
        context["current_action"] = json!(interaction);
        context["learner_context"]["current_action"] = json!(interaction);
        if let Some(purpose) = interaction.learning_purpose {
            context["learning_purpose"] = json!(purpose);
            context["learner_context"]["learning_purpose"] = json!(purpose);
        }
        // User quotes continue to refer only to actual supplementary user text.
        context["current_request"] = json!(interaction.user_supplement);
        context["learner_context"]["current_request"] = json!(interaction.user_supplement);
    }
}

/// Reference the existing private owners. Reading snapshots retain their original
/// count/time; they do not claim a source revision the reading ledger never stored.
fn usage_facts(state: &PrivateBookContext<'_>, binding: &TeachingBinding, request: &Value, interaction: Option<&runtime::tutor_continuation::ResolvedInteraction>, now: &str) -> Result<Vec<TeachingEvent>, ToolError> {
    let mut facts = Vec::new();
    let mut add = |id: String, chat: &str, turn: &str, payload: Value, at: &str| -> Result<(), ToolError> {
        if facts.iter().any(|e: &TeachingEvent| e.event_id == id) { return Ok(()); }
        match state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_event(&id) {
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
    let goal_revision=workspace_goal_revision(state,binding)?;
    let mut sessions: Vec<_> = state.user.agent_history.sessions.iter()
        .filter(|s| s.matches_scope(state.learning_workspace_id,&binding.source_id)).collect();
    sessions.sort_by(|a,b|a.updated_at.cmp(&b.updated_at));
    let recent: Vec<_> = sessions.into_iter().flat_map(|s| s.turns.iter().map(move |t| (s,t)))
        .filter(|(session,turn)| turn.published_book_ref.as_ref().map(|r|r.book_id.as_str())
            .unwrap_or(&session.book_id)==binding.source_id).rev().take(12).collect();
    for (session, turn) in recent.into_iter().rev() {
        if turn.turn_id == binding.turn_id { continue; }
        if bound(state,&turn.turn_id)?.is_some_and(|original|
            original.binding.source_id!=binding.source_id || original.binding.source_revision!=binding.source_revision) {
            continue;
        }
        let original_goal=turn.admission_input.as_ref().and_then(|input|input.learning_workspace_goal.as_ref())
            .or_else(||turn.domain.scene.as_ref().and_then(|scene|scene.workspace_goal.as_ref())).map(|goal|goal.revision);
        if original_goal != goal_revision { continue; }
        let original = json!({"kind":"chat_turn","book_id":session.book_id,"session_id":session.id,"turn_id":turn.turn_id});
        add(format!("usage:{}:user:{}", binding.tutor_session_id, turn.turn_id), &session.id, &turn.turn_id,
            json!({"original_ref":original,"usage":{"kind":if turn.domain.interaction.is_some() { "reading_action" } else { "user_message" }},
                "interaction":turn.domain.interaction,"response":turn.user_authored_text(),"presentation_follow_up":turn.presentation_follow_up,
                "question_anchor_lid":turn.question_anchor_lid,"teaching_ref":turn.teaching_ref,
                "workspace_goal_revision":original_goal,"published_book_ref":turn.published_book_ref}), &session.updated_at)?;
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
            "usage":{"kind":if interaction.is_some() { "reading_action" } else { "user_message" }},"interaction":interaction,
            "response":interaction.map(|a|a.user_supplement.as_str()).or_else(||request["message"].as_str()),"response_origin":request["teaching_response_origin"],"presentation_follow_up":request["presentation_follow_up"]}), now)?;
    Ok(facts)
}

struct PreparedSelection {
    movement: TeachingMove,
    retained_moves: Vec<TeachingMove>,
    object_versions: Vec<Value>,
    scoring_sources: BTreeMap<String, String>,
    scoring_source_refs: BTreeMap<String, memory::assessment::AssessmentSource>,
    delivery_move_refs: Vec<String>,
    selected_id: String,
    delivery_scope: Option<Value>,
}

impl PreparedSelection {
    fn event(&self, bind: &TeachingEvent, occurred_at: &str) -> TeachingEvent {
        let mut event = TeachingEvent {
            event_id: self.selected_id.clone(),
            binding: bind.binding.clone(),
            kind: TeachingFact::MoveSelected,
            causal_refs: vec![bind.event_id.clone()],
            payload: json!({"move":self.movement,"delivery_move_refs":self.delivery_move_refs,
                "object_versions":self.object_versions,"scoring_sources":self.scoring_sources,"scoring_source_refs":self.scoring_source_refs,
                "scorer_version":"assessment.v1","retry_policy":"append_attempt","help_policy":"preserve_exposure"}),
            occurred_at: occurred_at.into(),
        };
        if let Some(scope) = &self.delivery_scope { event.payload["delivery_scope"] = scope.clone(); }
        event
    }
}

fn inherited_selection_target(
    state:&PrivateBookContext<'_>,
    binding:&TeachingBinding,
    design_ref:Option<&str>,
    block_refs:&[String],
    design:&memory::teaching_design::TeachingDesign,
    evidence:&[SourceBinding],
) -> Result<TeachingTarget,ToolError> {
    let (deliveries,_)=state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_learning_opportunity(
        &binding.tutor_session_id,&binding.source_id,&binding.source_revision,design_ref,&design.focus_block_refs)?;
    let local=deliveries.iter().find(|delivery|delivery.payload["move"]["design_ref"].as_str()==design_ref
            && block_refs.iter().all(|block|delivery.payload["move"]["block_refs"].as_array()
                .is_some_and(|refs|refs.iter().any(|reference|reference.as_str()==Some(block.as_str()))))
            && delivery.payload["move"]["target"].is_object())
        .map(|delivery|serde_json::from_value::<TeachingTarget>(delivery.payload["move"]["target"].clone())
            .map_err(|error|invalid(error.to_string()))).transpose()?;
    let mut target=local.unwrap_or_else(||design.target.clone());
    refresh_inherited_media(&mut target,binding,evidence)?;
    Ok(target)
}

/// Share all deterministic validation and normalization with the eventual
/// append, so selection saves exactly the move validated by the host.
fn prepare_select(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    bind: &TeachingEvent,
    request: &Value,
    evidence: &[SourceBinding],
    ranges: &[EvidenceRange],
) -> Result<PreparedSelection, TeachingSelectionPreflightError> {
    fn invalid(message: impl Into<String>) -> TeachingSelectionPreflightError {
        memory::teaching::invalid(message).into()
    }
    // Native input references one inspected candidate. Expand its known
    // metadata before scope and typed validation, so a compact board_part uses
    // exactly the same support, focus and source checks as an ordinary move.
    let native = request.get("native_board_selection").map(|_|native_board_selection::bind(state,turn,bind,request,evidence)).transpose()?;
    let request = native.as_ref().unwrap_or(request);
    let binding = &bind.binding;
    let delivery_scope = runtime::tutor::validate_delivery_scope(request,
        state.user.agent_history.sessions.iter().find(|session|session.id==turn.session_id)
            .and_then(|session|session.turns.iter().find(|saved|saved.turn_id==turn.turn_id))
            .is_some_and(|saved|saved.domain.board_context.is_some()))?;
    let mut move_input = request["move"].clone();
    if let Some(target) = move_input.get_mut("target").filter(|value| !value.is_null()) {
        *target = serde_json::to_value(bind_target(target.clone(), binding, evidence)?).unwrap();
    }
    for item in move_input.pointer_mut("/assessment/rule/items").and_then(Value::as_array_mut).into_iter().flatten() {
        if let Some(target)=item.get_mut("target").filter(|v|!v.is_null()) {
            *target=json!(bind_target(target.clone(),binding,evidence)?);
        }
    }
    if bind.payload["current_action"].is_object() {
        if let Some(capability) = move_input["target"].get("capability").cloned() {
            move_input["capability"] = capability;
        }
    }
    if let (Some((intent, origin)), Some(fields)) =
        (runtime::tutor::action_intent(&bind.payload), move_input.as_object_mut())
    {
        fields.insert("interaction_intent".into(), json!(intent));
        fields.insert("intent_origin".into(), json!(origin));
        fields.remove("intent_quote");
        fields.remove("preference_ref");
    }
    let mut movement: TeachingMove = serde_json::from_value(move_input)
        .map_err(|e| invalid(e.to_string()))?;
    if let Some(reference)=movement.practice.as_ref().and_then(|practice|practice.revisit_note.as_ref()) {
        if !movement.note_refs.contains(reference) {movement.note_refs.push(reference.clone());}
    }
    let adopted_design=design::validate_move(state,bind,&movement)?;
    if movement.delivery_text.as_deref().is_none_or(|text|text.trim().is_empty()) {
        return Err(invalid("选择教学动作需要实际 delivery_text 正文；请在接纳前完成本轮要交付的帮助和邀请"));
    }
    if movement.target.is_none() {
        let adopted_design=adopted_design.as_ref().ok_or_else(||invalid("直接问答需要本轮的来源目标"))?;
        movement.target=Some(inherited_selection_target(state,binding,movement.design_ref.as_deref(),&movement.block_refs,adopted_design,evidence)?);
    }
    if let Some(continuation) = &movement.continuation {
        continuation.validate().map_err(invalid)?;
        let target = movement.target.as_ref().ok_or_else(|| invalid("下一步建议必须继承本轮已读教学目标"))?;
        let selected = state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_turn_events(&binding.tutor_session_id, &turn.turn_id)?;
        if selected.iter().any(|e| e.kind == TeachingFact::MoveSelected && !e.payload["move"]["continuation"].is_null()) {
            return Err(invalid("同一回合最多交付一组下一步建议"));
        }
        let pending = pending_activity(state)?;
        if movement.assessment.is_some()
            || selected.iter().any(|e| e.kind == TeachingFact::MoveSelected && !e.payload["move"]["assessment"].is_null())
            || pending.as_ref().map(|delivery| activity_target_overlaps(state, delivery, target)).transpose()?.unwrap_or(false)
        {
            return Err(invalid("当前正式活动尚未完成，请省略下一步建议并沿用原活动帮助"));
        }
        for text in std::iter::once(&target.learning_focus).chain(continuation.options.iter().flat_map(|o| [&o.label, &o.purpose])) {
            runtime::orchestrator::compile_presentation_text(text, evidence, &state.messages)
                .map_err(|_| invalid("下一步建议公开文字来源无效"))?;
        }
    }
    for id in &movement.interpretation_refs {
        let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
        let (interpretation,_) = store.understanding_basis(binding,id)?;
        if let (Some(scope), Some(target)) = (&interpretation.target, &movement.target) {
            if !targets_overlap(state, scope, target) { return Err(invalid("所采用解释与本次教学范围不相交")); }
        }
    }
    validate_move_intent(&movement, &bind.payload)?;
    if let Some(input) = &movement.answer_input {
        if !movement.actions.iter().any(|a| a == "submit")
            || !movement.actions.iter().any(|a| a == "revise")
            || movement.response_expectation.is_some()
            || movement.practice.as_ref().is_some_and(|p| matches!(p.entry,
                runtime::practice::PracticeEntry::Board | runtime::practice::PracticeEntry::Presentation))
            || !input.matches_assessment(movement.assessment.as_ref())
        {
            return Err(invalid("answer_input 需要 submit、revise 和匹配的独立评分标准。选择题的公开选项 ID 应与 choices 一致，单选恰有一个正确项；计算题使用 open。"));
        }
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
        && movement.practice.is_none()
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
        for media in target.media_bindings.iter().flatten() {
            if !evidence.iter().any(|source| source.book_id == binding.source_id
                && source.source_revision.as_deref() == Some(binding.source_revision.as_str())
                && source.media.as_ref().is_some_and(|read| read.target == media.target)) {
                return Err(invalid("本轮尚未重新读取继承的视频目标；请按原范围读取并登记 source.present 来源"));
            }
        }
        for music in target.music_bindings.iter().flatten() {
            if !evidence.iter().any(|source| source.book_id == binding.source_id
                && source.source_revision.as_deref() == Some(binding.source_revision.as_str())
                && source.music.as_ref().is_some_and(|read| read.target == music.target)) {
                return Err(invalid("本轮尚未重新读取继承的音乐目标；请按原范围读取并登记 source.present 来源"));
            }
        }
        if target.capability != movement.capability
            || target.object_refs.len() != movement.object_ids.len()
            || movement.object_ids.iter().any(|id| !target.object_refs.iter().any(|r| &r.object_id == id))
            || target.object_refs.iter().any(|r| !objects.iter().any(|o|
                o["ref"]["source_id"] == r.source_id && o["ref"]["object_id"] == r.object_id
                    && o["object_revision"].as_u64() == Some(r.object_revision)))
        {
            return Err(invalid("教学目标的能力、对象与所用修订不一致"));
        }
        for source in &target.source_bindings {
            preflight_target_read(state, &source.start_lid, &source.end_lid, ranges)?;
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
    let object_labels: Vec<String> = objects.iter()
        .filter(|o| movement.object_ids.iter().any(|id| o["ref"]["object_id"] == *id))
        .filter_map(|o| o["meaning"].as_str().map(str::to_owned)).collect();
    practice::validate_select(state, &bind.payload["current_action"], &movement, &object_labels, ranges)?;
    // Public prompt/help use the same provenance compiler as an answer.
    runtime::orchestrator::compile_presentation_text(
        &movement.prompt,
        evidence,
        &state.messages,
    )
    .map_err(|_| invalid("教学文字来源无效"))?;
    runtime::orchestrator::compile_presentation_text(movement.delivery_text.as_deref().unwrap(),evidence,&state.messages)
        .map_err(|issues|invalid(format!("教学正文来源无效（move.delivery_text）；请修正以下诊断指向的正文片段后重新 select：\n{}",
            serde_json::to_string(&issues).unwrap())))?;
    let practice_hints = movement.practice.iter().flat_map(|p| p.hints.iter().map(|h| &h.text));
    for text in movement.hint.iter().chain(movement.explanation.iter()).chain(practice_hints) {
        runtime::orchestrator::compile_presentation_text(text, evidence, &state.messages)
            .map_err(|_| invalid("帮助文字来源无效"))?;
    }
    if let Some(expectation)=&movement.response_expectation {
        runtime::orchestrator::compile_presentation_text(&expectation.public_prompt,evidence,&state.messages)
            .map_err(|_|invalid("普通邀请文字来源无效"))?;
    }
    let mut scoring_sources = BTreeMap::new();
    let mut scoring_source_refs = BTreeMap::new();
    if let Some(contract) = &movement.assessment {
        contract.validate()?;
        if !movement.actions.iter().any(|a| a == "submit") {
            return Err(invalid("评分活动必须允许提交"));
        }
        for lid in &contract.source_lids {
            preflight_target_read(state, lid, lid, ranges)?;
            scoring_sources.insert(lid.clone(), state.book.text(lid, Some(lid))?);
        }
        scoring_source_refs = assessment_sources::freeze(contract, movement.target.as_ref(), binding, evidence)?;
        if serde_json::to_string(&scoring_sources).unwrap().len()
            + serde_json::to_string(&scoring_source_refs).unwrap().len() > 24000 {
            return Err(invalid("评分来源超出单次评估预算，请缩小活动"));
        }
    }
    let object_versions: Vec<_> = objects.iter().filter(|o| movement.object_ids.iter().any(|id| o["ref"]["object_id"] == *id))
        .map(|o| json!({"ref":o["ref"],"object_revision":o["object_revision"],"meaning":o["meaning"]})).collect();
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    if let Some(target)=&mut movement.target {
        store.bind_focus(target,binding,&format!("focus:move:{}:{}",turn.turn_id,movement.move_id))?;
    }
    for reference in &movement.note_refs {
        let note=state.user.store.personal_note(reference)?;
        if reference.revision!=Some(note.value.revision) { return Err(invalid("理解路径需要确切笔记版本")); }
    }
    if let Some(AssessmentContract{rule:memory::assessment::ScoringRule::Open{items},..})=&mut movement.assessment {
        for item in items {
            if item.note_refs.iter().any(|note|!movement.note_refs.contains(note)) {return Err(invalid("评分项只能引用本次实际采用的笔记版本"));}
            if let Some(target)=&mut item.target {
                if target.object_refs.len()>1 || target.object_refs.iter().any(|r|!movement.target.as_ref().is_some_and(|t|t.object_refs.contains(r))) {
                    return Err(invalid("原子评分项只能对应一个实际采用的对象版本"));
                }
                for source in &target.source_bindings {preflight_target_read(state,&source.start_lid,&source.end_lid,ranges)?;}
                store.bind_focus(target,binding,&format!("focus:criterion:{}:{}:{}",turn.turn_id,movement.move_id,item.id))?;
            }
        }
    }
    let mut delivery_move_refs:Vec<String>=serde_json::from_value(request.get("delivery_move_refs").cloned().unwrap_or(json!([]))).map_err(|e|invalid(e.to_string()))?;
    let mut included=std::collections::BTreeSet::new();
    delivery_move_refs.retain(|reference|included.insert(reference.clone()));
    let selected_id=format!("move:{}:{}", turn.turn_id, movement.move_id);
    let mut retained_moves=Vec::new();
    for id in &delivery_move_refs {
        let selected=event(state,id)?;
        if selected.kind!=TeachingFact::MoveSelected || selected.binding!=*binding { return Err(invalid("待交付集合只接受本轮实际选择引用")); }
        if selected.payload["move"]["design_ref"]!=json!(movement.design_ref) { return Err(invalid("改案后需要重新选择沿用的支持，不能静默交付旧设计动作")); }
        if id!=&selected_id {
            let retained:TeachingMove=serde_json::from_value(selected.payload["move"].clone()).map_err(|e|invalid(e.to_string()))?;
            // Retaining a move adopts its learner claims again. A corrected
            // interpretation cannot stay in use through an earlier selection.
            design::validate_move(state,bind,&retained)?;
            if let Some(scope) = &delivery_scope {
                runtime::tutor::validate_delivery_scope(&json!({"operation":"select","delivery_scope":scope,"move":retained}), true)?;
            }
            retained_moves.push(retained);
        }
    }
    if !delivery_move_refs.contains(&selected_id) { delivery_move_refs.push(selected_id.clone()); }
    if movement.practice.is_some() && retained_moves.iter().any(|retained| retained.practice.is_some()) {
        return Err(invalid("同一次交付只出一道练习题；等用户作答或继续后再出下一道"));
    }
    let prepared = PreparedSelection { movement, retained_moves, object_versions, scoring_sources, scoring_source_refs, delivery_move_refs, selected_id, delivery_scope };
    // Reject a changed saved fact before model review. The append transaction
    // repeats the same check, so preflight does not reserve or write anything.
    match event(state, &prepared.selected_id) {
        Ok(previous) => prepared.event(bind, &previous.occurred_at).validate_unchanged_retry(&previous)?,
        Err(error) if error.error_code == "TEACHING_INVALID" => {},
        Err(error) => return Err(error.into()),
    }
    Ok(prepared)
}

pub(crate) fn preflight_step(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    request: &Value,
    evidence: &[SourceBinding],
    ranges: &[EvidenceRange],
) -> Result<Value, TeachingSelectionPreflightError> {
    if !matches!(request["operation"].as_str(),Some("design"|"select")) {
        return Err(invalid("教学预检只接受 design 或 select 操作").into());
    }
    let bind = bound(state, &turn.turn_id)?.ok_or_else(|| invalid("本轮学习上下文尚未就绪"))?;
    if !current(state, &bind.binding)? {
        return Err(invalid("此操作需要本轮明确的练习请求，请直接回答当前问题").into());
    }
    if request["operation"] == "design" {
        let mut normalized=request.clone();
        normalized["design"]=json!(design::prepare(state,&bind,request,evidence,ranges)?);
        return Ok(normalized);
    }
    let prepared = prepare_select(state, turn, &bind, request, evidence, ranges)?;
    let mut normalized = request.clone();
    let (current_design,interpretation_refs)=if prepared.movement.design_ref.is_some() {
        let current_design=design::current_design(state,&bind.binding)?;
        let focus_blocks:Vec<String>=serde_json::from_value(current_design["design"]["focus_block_refs"].clone()).map_err(|e|invalid(e.to_string()))?;
        let references=state.user.learning_store_for_workspace(state.learning_workspace_id)?.current_design_interpretations(
            &bind.binding.tutor_session_id,&bind.binding.source_id,&bind.binding.source_revision,
            current_design["design_ref"].as_str(),&focus_blocks,12)?.into_iter().map(|(_,e)|e.evidence_id).collect::<Vec<_>>();
        (current_design,references)
    } else { (Value::Null,Vec::new()) };
    let mut movements=prepared.retained_moves;
    movements.push(prepared.movement.clone());
    normalized["learner_grounding"]=grounding::context(state,&bind.binding,
        &current_design["design"],&movements,&[],&interpretation_refs)?;
    normalized["move"] = serde_json::to_value(prepared.movement).unwrap();
    normalized.as_object_mut().unwrap().remove("native_board_selection");
    if let Some(scope)=prepared.delivery_scope { normalized["delivery_scope"]=scope; }
    else { normalized.as_object_mut().unwrap().remove("delivery_scope"); }
    normalized["committed_deliveries"] = board_delivery::review_context(state.user, turn);
    Ok(normalized)
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
        None => return Err(invalid("本轮学习上下文尚未就绪")),
    };
    let binding = &bind.binding;
    let observation_operation = matches!(request["operation"].as_str(),Some("trace"|"understanding"|"corrections"|"closed_book"))
        || (request["operation"] == "evidence" && request["nature"] == "hypothesis");
    if !observation_operation && !current(state, binding)? {
        return Err(invalid(
            "此操作需要本轮明确的练习请求，请直接回答当前问题",
        ));
    }
    match request["operation"].as_str() {
        Some("closed_book") => closed_book::propose(state,binding,&request,&occurred_at),
        Some("corrections") => corrections::step(state,binding,&request,ranges,&occurred_at),
        Some("design") => design::accept(state,&bind,&request,evidence,ranges,&occurred_at),
        Some("evidence") if request["nature"] == "hypothesis" => accept_hypothesis(state, binding, &request, evidence, ranges),
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
                let mut page = understanding(
                    state,
                    &binding.source_id,
                    request["object_id"].as_str(),
                    request["after"].as_u64().unwrap_or(0),
                )?;
                // Tool lists are previews; exact evidence remains chunk-readable.
                // The UI uses the full strings from understanding directly.
                for item in page["interpretations"].as_array_mut().unwrap() {
                    for field in ["interpretation", "teaching_implication", "learner_quote", "correction"] {
                        if item[field].is_null() { continue; }
                        let excerpt = preview(&item[field], 600);
                        item[field] = excerpt["text"].clone();
                        item[format!("{field}_truncated")] = excerpt["truncated"].clone();
                    }
                }
                Ok(page)
            }
        }
        Some("trace") => {
            check_activity_source(state, binding)?;
            if let Some(id) = request["event_id"].as_str() {
                let fact = event(state, id)?;
                if (fact.binding.tutor_session_id != binding.tutor_session_id && !matches!(bind.payload["status"].as_str(),Some("observing" | "course"))) || fact.binding.source_id != binding.source_id
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
            let rows = state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_events(
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
            let prepared =
                prepare_select(state, turn, &bind, &request, evidence, ranges).map_err(|failure| failure.error)?;
            let saved = state.user.learning_store_for_workspace(state.learning_workspace_id)?
                .append_teaching(&prepared.event(&bind, &occurred_at))?;
            let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
            if let Some(target)=&prepared.movement.target {store.remember_focus(target,binding)?;}
            if let Some(AssessmentContract{rule:memory::assessment::ScoringRule::Open{items},..})=&prepared.movement.assessment {
                for target in items.iter().filter_map(|item|item.target.as_ref()) {store.remember_focus(target,binding)?;}
            }
            let delivery_draft=delivery_draft(state,&saved)?;
            Ok(
                json!({"selected":saved.event_id,"design_ref":prepared.movement.design_ref,"delivery_move_refs":prepared.delivery_move_refs,"delivery_scope":prepared.delivery_scope,"delivery_draft":delivery_draft,"instruction":if prepared.delivery_scope.is_some() {
                    "This complete local support has been accepted for board.author commit only. Commit its exact body and versions, then continue the remaining original request. It does not complete the turn or open a learner invitation."
                } else if prepared.movement.continuation.is_some() {
                    "Deliver the selected explanation fully. Leave the optional continuation work for the reader's choice; do not execute those options or duplicate their menu in the final answer. Saved candidate is not displayed. This selection is immutable. Retry unchanged input with the same move_id; revise with a new move_id and omit superseded selections from delivery_move_refs."
                } else { "Deliver this move in the final answer. Saved candidate is not displayed. This selection is immutable. Retry unchanged input with the same move_id; revise with a new move_id and omit superseded selections from delivery_move_refs." }}),
            )
        }
        _ => Err(invalid("未知教学操作")),
    }
}

/// Materialize the exact accepted set, never a request candidate. A later
/// selection replaces this set unless it explicitly retains earlier moves.
fn delivery_draft(state: &PrivateBookContext<'_>, selected: &TeachingEvent) -> Result<Value, ToolError> {
    let references:Vec<String>=serde_json::from_value(selected.payload["delivery_move_refs"].clone()).map_err(|e|invalid(e.to_string()))?;
    let mut moves=Vec::new();
    let mut texts=Vec::new();
    for reference in &references {
        let accepted=event(state,reference)?;
        let movement=&accepted.payload["move"];
        let mut text=movement["delivery_text"].as_str().filter(|text|!text.trim().is_empty())
            .ok_or_else(||invalid("待交付动作缺少已接纳的教学正文，请重新选择该动作"))?.to_owned();
        if let Some(prompt)=movement["response_expectation"]["public_prompt"].as_str().filter(|prompt|!text.contains(*prompt)) {
            text.push_str("\n\n");text.push_str(prompt);
        }
        texts.push(text);
        moves.push(movement.clone());
    }
    let mut draft=json!({"delivery_move_refs":references,"text":texts.join("\n\n"),"moves":moves});
    if let Some(scope)=selected.payload.get("delivery_scope") { draft["delivery_scope"]=scope.clone(); }
    Ok(draft)
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
            && (o.answer.as_deref().is_some_and(|s| !s.trim().is_empty())
                || o.answer_view.as_ref().is_some_and(|v| v.parts.iter().any(|p| match p {
                    AgentAnswerPart::Markdown { text } | AgentAnswerPart::SocraticQuestion { text, .. } => !text.trim().is_empty(),
            AgentAnswerPart::MusicExample { example } => !example.label.is_empty(),
                    AgentAnswerPart::Presentation { .. } => true,
                    AgentAnswerPart::Sources { .. } => false,
                })))
            && !crate::observability::lifecycle::delivery_failed(o)
            && !saved
                .delivery_diagnostics
                .as_ref()
                .and_then(|d| d.repair.as_ref())
                .is_some_and(|r| !r.issues.is_empty())
    }) else {
        return Ok(());
    };
    let learning_workspace_id = session.learning_workspace_id.as_deref();
    let source_book_id = saved.published_book_ref.as_ref().map(|r|r.book_id.as_str()).unwrap_or(&session.book_id);
    crate::cross_book::record_delivery(user,session,saved,outcome,now)?;
    let inferred_help_ref=if saved.teaching_ref.is_none() {
        if let Some(bind)=bound_user(user,learning_workspace_id,&turn.turn_id)? {
            let user_fact=format!("usage:{}:user:{}",bind.binding.tutor_session_id,turn.turn_id);
            let candidate=user.learning_store_for_workspace(learning_workspace_id)?.teaching_event(&user_fact).ok()
                .and_then(|fact|fact.payload["invitation_context_ref"].as_str().map(str::to_owned));
            candidate.filter(|id|user.learning_store_for_workspace(learning_workspace_id).and_then(|store|store.teaching_event(id)).is_ok_and(|fact|!fact.payload["assessment_contract_ref"].is_null()))
        } else {None}
    } else {None};
    if let Some(id) = saved.teaching_ref.as_ref().or(inferred_help_ref.as_ref()) {
        let original = referenced_user_delivery(user, learning_workspace_id, source_book_id, id)?;
        let mut binding = original.binding.clone();
        binding.chat_session_id = turn.session_id.clone();
        binding.turn_id = turn.turn_id.clone();
        // A reply about an existing activity remains observable help even in ordinary mode.
        append_user(
            user,
            learning_workspace_id,
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
    let Some(bind) = bound_user(user, learning_workspace_id, &turn.turn_id)? else {
        return Ok(());
    };
    let request_ref = format!("usage:{}:user:{}", bind.binding.tutor_session_id, turn.turn_id);
    let causes = user.learning_store_for_workspace(learning_workspace_id)?.teaching_event(&request_ref).ok().map(|e|vec![e.event_id]).unwrap_or_default();
    let delivery_usage_id=format!("usage:{}:assistant:{}",bind.binding.tutor_session_id,turn.turn_id);
    let mut delivery_usage=json!({"original_ref":{"kind":"chat_turn","book_id":source_book_id,"session_id":session.id,"turn_id":saved.turn_id},
        "usage":{"kind":"assistant_delivery"},"incomplete":outcome.incomplete,"answer":outcome.answer,"answer_view":outcome.answer_view,"source_bindings":saved.source_bindings});
    // Recovery can replay a pre-existing delivery from before this field was
    // recorded. Preserve that immutable receipt; it predates new corrections.
    if user.learning_store_for_workspace(learning_workspace_id)?.teaching_event(&delivery_usage_id)
        .is_ok_and(|old| old.payload.get("incomplete").is_none()) {
        delivery_usage.as_object_mut().unwrap().remove("incomplete");
    }
    append_user(user, learning_workspace_id, &bind.binding, delivery_usage_id,
        TeachingFact::UsageObserved, causes, delivery_usage, now)?;
    let events = user
        .learning_store_for_workspace(learning_workspace_id)?
        .teaching_turn_events(&bind.binding.tutor_session_id, &turn.turn_id)?;
    if events.iter().any(|e| e.payload["outside"] == true) {
        return Ok(());
    }
    // A committed answer remains a delivery fact after pause, including recovery of a failed trace write.
    // Modern selections require the exact durable final set; interrupted work never opens invitations.
    if outcome.incomplete {
        crate::teaching_coach::after_delivery(user, turn, now)?;
        return Ok(());
    }
    closed_book::delivered(user, learning_workspace_id, &bind.binding, now)?;
    for selected in events
        .iter()
        .filter(|e| e.kind == TeachingFact::MoveSelected
            && (outcome.teaching_delivery_move_refs.contains(&e.event_id)
                || (outcome.teaching_delivery_move_refs.is_empty() && e.payload["move"]["design_ref"].is_null())))
    {
        // A committed board part already froze this move's exact support.
        // The final cumulative answer cannot replace it or count it twice.
        if board_delivery::move_was_delivered(user, learning_workspace_id, selected)? { continue; }
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
        practice::redact(&mut public_move);
        public_move.as_object_mut().unwrap().remove("hint");
        public_move.as_object_mut().unwrap().remove("explanation");
        public_move.as_object_mut().unwrap().remove("assessment");
        public_move.as_object_mut().unwrap().remove("participation");
        public_move.as_object_mut().unwrap().remove("coach_ref");
        public_move.as_object_mut().unwrap().remove("coach_adjustment");
        public_move.as_object_mut().unwrap().remove("delivery_text");
        // A later selection in the same turn may have opened a formal activity.
        if outcome.incomplete || events.iter().any(|e| e.kind == TeachingFact::MoveSelected && !e.payload["move"]["assessment"].is_null()) {
            public_move.as_object_mut().unwrap().remove("continuation");
        }
        append_user(
            user,
            learning_workspace_id,
            &bind.binding,
            format!("delivery:{}:{}", turn.turn_id, movement.move_id),
            TeachingFact::MessageDelivered,
            vec![selected.event_id.clone()],
            json!({"move":public_move,"object_versions":selected.payload["object_versions"],"assessment_contract_ref":movement.assessment.as_ref().map(|_| &selected.event_id),"answer":outcome.answer,"answer_view":outcome.answer_view,"source_bindings":saved.source_bindings,"presentation":presentation}),
            now,
        )?;
    }
    crate::teaching_coach::after_delivery(user, turn, now)?;
    Ok(())
}

fn referenced_delivery(state: &PrivateBookContext<'_>, id: &str) -> Result<TeachingEvent, ToolError> {
    referenced_user_delivery(state.user, state.learning_workspace_id, &state.book.base.book_id, id)
}
fn referenced_user_delivery(user: &crate::user_runtime::UserRuntime, learning_workspace_id: Option<&str>, book_id: &str, id: &str) -> Result<TeachingEvent, ToolError> {
    let store = user.learning_store_for_workspace(learning_workspace_id)?;
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
    if check_activity_source(state, &saved.binding).is_err() {
        return Ok("unavailable");
    }
    Ok("active")
}

pub(crate) fn continuation_view(state: &PrivateBookContext<'_>, saved: &TeachingEvent) -> Result<Option<runtime::tutor_continuation::TutorContinuationView>, ToolError> {
    use runtime::tutor_continuation::{ContinuationAvailability, TeachingContinuation, TutorContinuationView};
    let Some(value) = saved.payload["move"].get("continuation").filter(|v| !v.is_null()) else { return Ok(None); };
    let continuation: TeachingContinuation = serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
    let target: TeachingTarget = serde_json::from_value(saved.payload["move"]["target"].clone()).map_err(|e| invalid(e.to_string()))?;
    let session = state.user.agent_history.sessions.iter().find(|s| s.id == saved.binding.chat_session_id);
    let head = session.and_then(|s| s.turns.last()).map(|t| t.turn_id.as_str());
    let accepted = session.and_then(|s| s.turns.iter().find(|t| t.domain.interaction.as_ref().is_some_and(|a|
        matches!(&a.action, runtime::tutor_continuation::InteractionAction::UiChoice { teaching_choice, .. } if teaching_choice.delivery_ref == saved.event_id))));
    let availability = if let Some(turn) = accepted {
        if let Some(event) = &turn.domain.choice_event { state.user.learning_store_for_workspace(state.learning_workspace_id)?.append_teaching(event)?; }
        ContinuationAvailability::Consumed { accepted_turn_id:turn.turn_id.clone() }
    } else if check_source(state, &saved.binding).is_err()
        || crate::tutor_api::tutor_source_readiness(state.book, state.book_dir)["status"] != "ready" {
        ContinuationAvailability::Unavailable
    } else if workspace_goal_revision(state, &saved.binding)? != state.user.learning_store_for_workspace(state.learning_workspace_id)?.state()?.workspace_goal_revision || head != Some(saved.binding.turn_id.as_str()) {
        ContinuationAvailability::Stale
    } else {
        ContinuationAvailability::Eligible
    };
    Ok(Some(TutorContinuationView { delivery_ref:saved.event_id.clone(), origin_turn_id:saved.binding.turn_id.clone(),
        target_summary:target.learning_focus, continuation, expected_chat_head:saved.binding.turn_id.clone(), availability }))
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
    reasoning: Option<String>,
    #[serde(default)]
    response_origin: Option<Value>,
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
        None if !saved.payload["move"]["presentation"].is_null()
            && !saved.payload["move"]["response_expectation"].is_object() => {
            Err(invalid("演示活动需要提交时的现场"))
        }
        None => Ok(Value::Null),
    }
}

fn behavior(state: &PrivateBookContext<'_>, request: BehaviorRequest, now: &str) -> Result<Value, ToolError> {
    let saved = delivery(state, &request.delivery_ref)?;
    let id = format!("action:{}", request.operation_id);
    let scene = scene_snapshot(state, &saved, request.scene.as_ref())?;
    let mut payload = json!({"delivery_ref":saved.event_id,"action":request.action,"response":request.response,"response_origin":request.response_origin,"scene":scene,"assessment":"unassessed"});
    if let Some(text) = &request.reasoning { payload["reasoning"] = json!(text); }
    let old = state
        .user.learning_store_for_workspace(state.learning_workspace_id)?
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
        if matches!(request.action.as_str(), "submit" | "revise") {
            question::validate_answer(&saved.payload["move"]["answer_input"], request.response.as_deref().unwrap_or(""))?;
        }
    }
    // Help and attempt conditions are frozen at submission, not reconstructed after a reveal.
    if let Some(previous) = old.iter().find(|e| e.event_id == id) {
        payload["assistance_refs"] = previous.payload["assistance_refs"].clone();
        payload["attempt"] = previous.payload["attempt"].clone();
    } else {
        payload["assistance_refs"] = json!(old
            .iter()
            .filter(|e| e.kind == TeachingFact::HelpDisplayed && practice::is_assistance(e))
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
        let practice_hint = if request.action == "request_hint" {
            practice::next_hint(&selected, &old, &request.operation_id)?
        } else { None };
        let mut released = match practice_hint {
            Some(hint) => hint,
            None => {
                let key = if request.action == "request_hint" { "hint" } else { "explanation" };
                let text = selected.payload["move"][key]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| invalid("帮助内容当前不可用"))?;
                json!({"action":request.action,"text":text})
            }
        };
        released["delivery_ref"] = json!(saved.event_id);
        let offered = append(
            state,
            &saved.binding,
            format!("help:{}", request.operation_id),
            TeachingFact::HelpDelivered,
            vec![action.event_id.clone()],
            released.clone(),
            now,
        )?;
        let mut view = released;
        view["event_id"] = json!(offered.event_id);
        help = Some(view);
    }
    let assessment = assess_action(state, &saved, &action).ok().flatten();
    // A correct practice answer concludes the problem and releases its solution.
    if assessment.as_ref().is_some_and(|a| a.status == AssessmentStatus::Correct) {
        help = practice::release_concluded_solution(state, &saved, &action, now)?;
    }
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

/// Only the latest delivered teaching focus in the current session can own a
/// pending question. Historical unanswered questions do not block other reading.
pub(crate) fn pending_activity_help(state: &PrivateBookContext<'_>, interaction: &runtime::tutor_continuation::ResolvedInteraction) -> Result<Vec<String>, ToolError> {
    let Some(delivery) = pending_activity(state)? else { return Ok(vec![]); };
    let facts = state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_activity_events(&delivery.event_id)?;
    let answer_match = interaction.answer_ref.as_ref().is_some_and(|answer|
        state.selected_chat == Some(delivery.binding.chat_session_id.as_str()) && answer.turn_id == delivery.binding.turn_id
        || facts.iter().any(|f| f.kind == TeachingFact::MessageDelivered
            && Some(f.binding.chat_session_id.as_str()) == state.selected_chat && f.binding.turn_id == answer.turn_id));
    let scope_match = if let Some(target) = &interaction.target { activity_target_overlaps(state, &delivery, target)? }
        else { activity_overlaps(state, &delivery, &interaction.source_bindings)? };
    Ok(if answer_match || scope_match { vec![delivery.event_id] } else { vec![] })
}

fn pending_activity(state: &PrivateBookContext<'_>) -> Result<Option<TeachingEvent>, ToolError> {
    let store = state.user.learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)?;
    let Some(chat) = state.selected_chat else { return Ok(None); };
    let Some(delivery) = store.latest_chat_teaching_activity(chat, &state.book.base.book_id)? else { return Ok(None); };
    if delivery.binding.source_revision != state.book.source_fingerprint() { return Ok(None); }
    if frozen_contract(state, &delivery)?.is_none() { return Ok(None); }
    let facts = store.teaching_activity_events(&delivery.event_id)?;
    if facts.iter().any(|f| f.kind == TeachingFact::LearnerAction
        && matches!(f.payload["action"].as_str(), Some("submit" | "revise" | "skip"))) { return Ok(None); }
    Ok(Some(delivery))
}

fn activity_overlaps(state: &PrivateBookContext<'_>, delivery: &TeachingEvent, sources: &[memory::teaching::TeachingSourceBinding]) -> Result<bool, ToolError> {
    let target: Option<TeachingTarget> = serde_json::from_value(delivery.payload["move"]["target"].clone()).unwrap_or(None);
    let contract = if target.is_none() { frozen_contract(state, delivery)?.map(|(_, contract)| contract) } else { None };
    Ok(sources.iter().any(|s| {
        if let Some(target) = &target { target.source_bindings.iter().any(|t|
            source_ranges_overlap(state, &s.start_lid, &s.end_lid, &t.start_lid, &t.end_lid)) }
        else { contract.as_ref().is_some_and(|contract| contract.source_lids.iter()
            .any(|lid| source_ranges_overlap(state,&s.start_lid,&s.end_lid,lid,lid))) }
    }))
}

fn activity_target_overlaps(state: &PrivateBookContext<'_>, delivery: &TeachingEvent, target: &TeachingTarget) -> Result<bool, ToolError> {
    let saved: Option<TeachingTarget> = serde_json::from_value(delivery.payload["move"]["target"].clone()).unwrap_or(None);
    if let Some(saved) = saved { Ok(targets_overlap(state, &saved, target)) }
    else { activity_overlaps(state, delivery, &target.source_bindings) }
}

fn assess_action(
    state: &PrivateBookContext<'_>,
    delivery: &TeachingEvent,
    action: &TeachingEvent,
) -> Result<Option<ResponseAssessment>, ToolError> {
    let mut store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
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
    let contract = frozen_contract(state, delivery)?.map(|(_, c)| c);
    let solution_seen = facts.iter().any(|e| e.kind == TeachingFact::HelpDisplayed && e.payload["action"] == "reveal");
    let feedback_visible = solution_seen || contract.as_ref().is_some_and(|c| c.feedback == FeedbackPolicy::AfterSubmit);
    let items = match (&assessment, contract.as_ref().map(|c| &c.rule)) {
        (Some(a), Some(ScoringRule::Open {items})) if feedback_visible && !solution_seen &&
            delivery.payload["move"]["answer_input"]["kind"] == "worked_solution" && a.status != AssessmentStatus::Correct => {
            // A private evaluator explains its judgment against a key. Before a reveal,
            // show the learner's demonstrated lines and the author's solution-free retry prompts.
            let mut feedback = a.items.iter().map(|item| json!({"id":item.id,"verdict":item.verdict,
                "response_quote":item.response_quote,"retry":true,"reason":items.iter().find(|i| i.id == item.id)
                    .and_then(|i| i.retry_prompt.as_deref()).unwrap_or("")})).collect::<Vec<_>>();
            feedback.sort_by_key(|item| item["verdict"] != "supported");
            json!(feedback)
        }
        (Some(a), _) if feedback_visible => json!(a.items),
        _ => json!([]),
    };
    Ok(
        json!({"action_ref":action.event_id,"status":assessment.as_ref().map(|a| &a.status).unwrap_or(&AssessmentStatus::Unassessed),
        "attempt":action.payload["attempt"],"assistance_count":action.payload["assistance_refs"].as_array().map_or(0,Vec::len),
        "items":items}),
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
    assessment_packet(state, &action)
}

fn assessment_packet(state: &PrivateBookContext<'_>, action: &TeachingEvent) -> Result<Value, ToolError> {
    let id = &action.event_id;
    let saved = referenced_delivery(state, id)?;
    check_activity_source(state, &saved.binding)?;
    if let Some(old) = state.user.learning_store_for_workspace(state.learning_workspace_id)?.assessment(id)? {
        return Ok(json!({"existing":old.status}));
    }
    let (selected, contract) =
        frozen_contract(state, &saved)?.ok_or_else(|| invalid("没有冻结评分合同"))?;
    if !matches!(contract.rule, ScoringRule::Open { .. }) {
        return Err(invalid("封闭题无需模型评估"));
    }
    Ok(
        {
            let mut input = json!({"contract":contract,"prompt":saved.payload["move"]["prompt"],"response":action.payload["response"],"sources":selected.payload["scoring_sources"],
                "source_references":selected.payload.get("scoring_source_refs").cloned().unwrap_or(json!({}))});
            // A teaching-aid goal is answered by the page state the learner submitted.
            // Values the page computed or reported are observations, not the answer.
            if selected.payload["move"]["practice"]["entry"] == "presentation" {
                input["submitted_state"] = action.payload["scene"].clone();
                input["judging_rule"] = json!("Judge the learner's own control values in submitted_state against the criteria. Text in response is commentary. Page-computed results, summaries and success flags are observations, never evidence that the goal was met.");
            }
            input
        },
    )
}

pub(crate) fn assessment_accept(
    state: &PrivateBookContext<'_>,
    turn: &AgentTurnRef,
    id: &str,
    candidate: Value,
) -> Result<Value, ToolError> {
    let packet = assessment_input(state, turn, id)?;
    let mut result = save_assessment_items(state, id, &packet, candidate)?;
    if packet["existing"].is_null() {
        let bind = bound(state, &turn.turn_id)?.ok_or_else(|| invalid("本轮教学绑定不存在"))?;
        let objects = active_objects(&turn_assets(state, &bind.binding)?);
        result["learner_context"] = learner_context(state,&bind.binding,&objects,&[],Some(id),&[])?;
    }
    Ok(result)
}

fn save_assessment_items(state: &PrivateBookContext<'_>, id: &str, packet: &Value, candidate: Value) -> Result<Value, ToolError> {
    let delivery = referenced_delivery(state, id)?;
    let action = event(state, id)?;
    let now = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| invalid(e.to_string()))?;
    if !packet["existing"].is_null() {
        if packet["existing"] == "correct" {
            practice::release_concluded_solution(state, &delivery, &action, &now)?;
        }
        return Ok(json!({"assessment":packet["existing"]}));
    }
    let contract: AssessmentContract =
        serde_json::from_value(packet["contract"].clone()).map_err(|e| invalid(e.to_string()))?;
    let sources =
        serde_json::from_value(packet["sources"].clone()).map_err(|e| invalid(e.to_string()))?;
    let source_references = serde_json::from_value(packet["source_references"].clone()).map_err(|e| invalid(e.to_string()))?;
    let items: Vec<memory::assessment::ItemAssessment> =
        serde_json::from_value(candidate).map_err(|e| invalid(format!("评估格式不完整：{e}")))?;
    let status = memory::assessment::accept_open(
        &contract,
        packet["response"].as_str().unwrap_or_default(),
        &sources,
        &source_references,
        &items,
    )?;
    state
        .user.learning_store_for_workspace(state.learning_workspace_id)?
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
    if status == AssessmentStatus::Correct {
        practice::release_concluded_solution(state, &delivery, &action, &now)?;
    }
    refresh_learning(
        state,
        &delivery.binding.tutor_session_id,
        &delivery.binding.source_id,
    )?;
    Ok(json!({"assessment":status,"action_ref":id}))
}

fn refresh_learning(state: &PrivateBookContext<'_>, session: &str, source: &str) -> Result<(), ToolError> {
    let mut store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
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

fn media_targets_overlap(a: &base_schema::EvidenceTarget, b: &base_schema::EvidenceTarget) -> bool {
    use base_schema::EvidenceTarget;
    match (a, b) {
        (EvidenceTarget::Joint { members }, _) => members.iter().any(|member| media_targets_overlap(&member.target, b)),
        (_, EvidenceTarget::Joint { members }) => members.iter().any(|member| media_targets_overlap(a, &member.target)),
        (EvidenceTarget::Text { publication: ap, transcript: at, text_range: ar },
            EvidenceTarget::Text { publication: bp, transcript: bt, text_range: br }) =>
            ap == bp && at == bt && ar.lid == br.lid && ar.range.start < br.range.end && br.range.start < ar.range.end,
        (EvidenceTarget::Visual { publication: ap, frames: af, regions: ar },
            EvidenceTarget::Visual { publication: bp, frames: bf, regions: br }) => ap == bp && (
                af.iter().any(|frame| bf.contains(frame) || br.iter().any(|region| region.frame == *frame))
                || bf.iter().any(|frame| ar.iter().any(|region| region.frame == *frame))
                || ar.iter().any(|a| br.iter().any(|b| a.frame == b.frame && {
                    let [ax,ay,aw,ah] = a.rect_normalized;
                    let [bx,by,bw,bh] = b.rect_normalized;
                    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
                }))),
        (EvidenceTarget::Audio { publication: ap, location: al, .. },
            EvidenceTarget::Audio { publication: bp, location: bl, .. }) =>
            ap == bp && al.video == bl.video && al.start_ms < bl.end_ms && bl.start_ms < al.end_ms,
        _ => false,
    }
}

fn targets_overlap(state: &PrivateBookContext<'_>, a: &TeachingTarget, b: &TeachingTarget) -> bool {
    fn text_media_overlap(state: &PrivateBookContext<'_>, text: &memory::teaching::TeachingSourceBinding, media: &base_schema::EvidenceTarget) -> bool {
        match media {
            base_schema::EvidenceTarget::Text { text_range,.. } => source_ranges_overlap(state,
                &text.start_lid,&text.end_lid,&text_range.lid,&text_range.lid),
            base_schema::EvidenceTarget::Joint { members } => members.iter().any(|member| text_media_overlap(state,text,&member.target)),
            _ => false,
        }
    }
    a.source_bindings.iter().any(|a| b.source_bindings.iter().any(|b|
        a.source_id == b.source_id && a.source_revision == b.source_revision
        && source_ranges_overlap(state, &a.start_lid, &a.end_lid, &b.start_lid, &b.end_lid)))
        || a.source_bindings.iter().any(|a| b.media_bindings.iter().flatten().any(|b|
            a.source_id == b.source_id && a.source_revision == b.source_revision && text_media_overlap(state,a,&b.target)))
        || b.source_bindings.iter().any(|b| a.media_bindings.iter().flatten().any(|a|
            a.source_id == b.source_id && a.source_revision == b.source_revision && text_media_overlap(state,b,&a.target)))
        || a.media_bindings.iter().flatten().any(|a| b.media_bindings.iter().flatten().any(|b|
            a.source_id == b.source_id && a.source_revision == b.source_revision && media_targets_overlap(&a.target, &b.target)))
        || a.music_bindings.iter().flatten().any(|a| b.music_bindings.iter().flatten().any(|b|
            a.source_id == b.source_id && a.source_revision == b.source_revision
            && a.target.publication == b.target.publication && a.target.audio.recording == b.target.audio.recording
            && a.target.audio.start_ms < b.target.audio.end_ms && b.target.audio.start_ms < a.target.audio.end_ms))
}

pub(crate) fn accept_hypothesis(state: &PrivateBookContext<'_>, binding: &TeachingBinding, request: &Value, evidence: &[SourceBinding], ranges: &[EvidenceRange]) -> Result<Value, ToolError> {
    accept_hypothesis_with_assistance(state, binding, request, evidence, ranges, &[])
}

fn record_interpretation(
    state: &PrivateBookContext<'_>, binding: &TeachingBinding,
    evidence: &memory::learning_evidence::LearningEvidence,
) -> Result<(), ToolError> {
    let now = time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)
        .map_err(|error| invalid(error.to_string()))?;
    append(state, binding, format!("evidence-accepted:{}:{}", binding.turn_id, evidence.evidence_id),
        TeachingFact::EvidenceInterpreted, evidence.fact_refs.clone(),
        json!({"evidence_ref":evidence.evidence_id,"origin":"system_interpretation"}), &now)?;
    Ok(())
}

/// Background repair supplements only prior help verified against the original
/// expression's durable sequence; it never rewrites the expression itself.
pub(crate) fn accept_hypothesis_with_assistance(state: &PrivateBookContext<'_>, binding: &TeachingBinding, request: &Value, evidence: &[SourceBinding], ranges: &[EvidenceRange], prior_help: &[String]) -> Result<Value, ToolError> {
    use memory::learning_evidence::{EvidenceNature, LearningEvidence};
    let required = |key: &str| request[key].as_str().filter(|v|!v.trim().is_empty()).ok_or_else(||invalid(format!("缺少 {key}")));
    let mut target = bind_target(request["target"].clone(), binding, evidence)?;
    let objects=active_objects(&turn_assets(state,binding)?);
    if target.object_refs.iter().any(|r|!objects.iter().any(|o|o["ref"]["object_id"]==r.object_id && o["object_revision"].as_u64()==Some(r.object_revision))) {
        return Err(invalid("理解目标的对象修订不属于本轮资料"));
    }
    for source in &target.source_bindings {
        require_target_read_this_turn(state, &source.start_lid, &source.end_lid, ranges)?;
    }
    let fact_refs: Vec<String> = serde_json::from_value(request["fact_refs"].clone()).map_err(|e|invalid(e.to_string()))?;
    let mut quote = String::new();
    let mut assistance = Vec::new();
    let mut support = Vec::new();
    for id in &fact_refs {
        let fact = state.user.learning_store_for_workspace(state.learning_workspace_id)?.learner_expression_fact(binding,id)?;
        if quote.is_empty() { quote = fact.learner_expression().unwrap_or_default().into(); }
        support.extend(fact.payload["support_delivery_refs"].as_array().into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)));
        assistance.extend(fact.payload["assistance_refs"].as_array().into_iter().flatten().filter_map(|v|v.as_str().map(str::to_owned)));
        // Explicit foreground observations use the same input-time help as the
        // background coach, including ordinary answers without a teaching move.
        support.extend(state.user.learning_store_for_workspace(state.learning_workspace_id)?
            .prior_chat_help_for_expression(&fact)?.into_iter().map(|help| help.event_id));
        if fact.kind == TeachingFact::UsageObserved && fact.learner_expression().is_some() {
            // A referenced chat response has both a usage observation and a
            // LearnerAction. Its help exposure was frozen by that original action,
            // not by whichever fact the model happened to cite.
            match event(state, &format!("response:{}", fact.binding.turn_id)) {
                Ok(response) if response.binding.tutor_session_id == fact.binding.tutor_session_id
                    && response.binding.source_id == fact.binding.source_id
                    && response.binding.source_revision == fact.binding.source_revision
                    && response.binding.chat_session_id == fact.binding.chat_session_id => {
                    assistance.extend(response.payload["assistance_refs"].as_array().into_iter().flatten()
                        .filter_map(|v|v.as_str().map(str::to_owned)));
                    support.extend(response.payload["support_delivery_refs"].as_array().into_iter().flatten()
                        .filter_map(|v|v.as_str().map(str::to_owned)));
                }
                Ok(_) => {},
                Err(error) if error.error_code == "TEACHING_INVALID" => {},
                Err(error) => return Err(error),
            }
        }
    }
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let earliest = fact_refs.iter().map(|id| store.teaching_event_sequence(id)).collect::<Result<Vec<_>,_>>()?
        .into_iter().min().ok_or_else(|| invalid("缺少使用事实依据"))?;
    // Explicit teaching references can cross chats. Only the exact help already
    // frozen on the original expression grants that scope, never a new model ref.
    let frozen_help: std::collections::BTreeSet<_> = assistance.iter().chain(&support).cloned().collect();
    for id in prior_help {
        let help = store.teaching_event(id)?;
        let delivered = matches!(help.kind, TeachingFact::MessageDelivered | TeachingFact::HelpDisplayed)
            || help.kind == TeachingFact::UsageObserved && help.payload["usage"]["kind"] == "assistant_delivery";
        let same_frozen_course = if frozen_help.contains(id) && help.binding.tutor_session_id != binding.tutor_session_id {
            let original = event(state, &format!("binding:{}", binding.turn_id))?;
            let course = original.payload["course_run"].as_str()
                .or_else(|| original.payload["inquiry_course"]["lesson"]["progress"]["run_id"].as_str());
            if let Some(course) = course {
                help.binding.tutor_session_id == course || event(state, &format!("binding:{}", help.binding.turn_id))?
                    .payload["inquiry_course"]["lesson"]["progress"]["run_id"].as_str() == Some(course)
            } else { false }
        } else { false };
        let owned_chat = help.binding.chat_session_id == binding.chat_session_id
            || frozen_help.contains(id) && help.binding.tutor_session_id == binding.tutor_session_id || same_frozen_course;
        if !delivered || !owned_chat
            || help.binding.source_id != binding.source_id || help.binding.source_revision != binding.source_revision
            || store.teaching_event_sequence(id)? >= earliest {
            return Err(invalid("理解观察只能关联原表达之前同一来源版本、同一对话或原表达已引用的实际帮助"));
        }
        if help.kind == TeachingFact::HelpDisplayed { assistance.push(id.clone()); } else { support.push(id.clone()); }
    }
    assistance.sort(); assistance.dedup();
    support.sort(); support.dedup();
    store.bind_focus(&mut target,binding,&format!("focus:interpretation:{}",required("operation_id")?))?;
    let understanding:memory::understanding::UnderstandingObservation=serde_json::from_value(request.get("understanding").cloned().unwrap_or(json!({}))).map_err(|e|invalid(format!("理解说明无效：{e}")))?;
    if !understanding.knowledge_links.is_empty() {
        let publication=state.user.agent_history.sessions.iter().find(|s|s.id==binding.chat_session_id)
            .and_then(|s|s.turns.iter().find(|t|t.turn_id==binding.turn_id)).and_then(|t|t.published_book_ref.as_ref());
        let teaching_context=event(state,&format!("binding:{}",binding.turn_id))?.payload;
        let publication = publication.map(|p| json!(p)).unwrap_or_else(||teaching_context["publication"].clone());
        let candidates=crate::knowledge_tree::candidates(state.book,state.book_dir,&json!({"publication":publication,"source_bindings":evidence,"verified_read_ranges":ranges,"teaching_context":teaching_context}))?;
        crate::knowledge_tree::validate_links(&understanding.knowledge_links,&candidates,&quote)?;
    }
    let note_refs=crate::personal_note_context::evidence_notes(state.user,binding,request.get("note_links"))?;
    let evidence = LearningEvidence { note_refs, criterion_refs:Vec::new(), understanding,
        evidence_id:format!("interpretation:{}",required("operation_id")?),
        action_ref:fact_refs.first().cloned().ok_or_else(||invalid("缺少使用事实依据"))?, delivery_ref:None,
        session_id:binding.tutor_session_id.clone(),source_id:binding.source_id.clone(),source_revision:binding.source_revision.clone(),map_revision:binding.map_revision.clone(),
        object_id:None,object_revision:None,nature:EvidenceNature::Hypothesis,
        label:target.learning_focus.clone(),capability:target.capability.clone(),target:Some(target),fact_refs,
        design_ref:request["design_ref"].as_str().map(str::to_owned),
        block_refs:serde_json::from_value(request.get("block_refs").cloned().unwrap_or(json!([]))).map_err(|e|invalid(e.to_string()))?,
        teaching_implication:required("teaching_implication")?.into(),
        support_delivery_refs:support,assistance_refs:assistance,attempt:0,assessment_ref:None,status:AssessmentStatus::Unassessed,
        interpretation:required("interpretation")?.into(),learner_quote:quote,interpreter_version:"resident.hypothesis.v2".into(),
        supersedes:request["supersedes"].as_str().map(str::to_owned),correction:None,corrected_at:None,source_quotes:json!([]),
    };
    let accepted = state.user.learning_store_for_workspace(state.learning_workspace_id)?.append_evidence(&evidence)?;
    record_interpretation(state, binding, &accepted)?;
    refresh_learning(state,&binding.tutor_session_id,&binding.source_id)?;
    if event(state,&format!("binding:{}",binding.turn_id))?.payload["course_run"].is_string() {
        return Ok(json!({"evidence_ref":accepted.evidence_id,"nature":accepted.nature,"teaching_implication":accepted.teaching_implication}));
    }
    Ok(json!({"evidence_ref":accepted.evidence_id,"nature":accepted.nature,"teaching_implication":accepted.teaching_implication,
        "learner_context_update":learner_context(state,binding,&active_objects(&turn_assets(state,binding)?),&[],None,&[])?,
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
    let evidence = memory::learning_evidence::LearningEvidence { note_refs:Vec::new(), criterion_refs:Vec::new(), understanding:Default::default(),
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
        design_ref:delivery.payload["move"]["design_ref"].as_str().map(str::to_owned),
        block_refs:serde_json::from_value(delivery.payload["move"].get("block_refs").cloned().unwrap_or(json!([]))).map_err(|e|invalid(e.to_string()))?,
        fact_refs: vec![action.event_id.clone(), delivery.event_id.clone()],
        teaching_implication: "按来源辨析表现选择后续支持".into(),
        label: object["meaning"].as_str().unwrap_or(object_id).into(),
        capability: "evaluation".into(),
        assistance_refs: serde_json::from_value(action.payload["assistance_refs"].clone())
            .unwrap_or_default(),
        support_delivery_refs: serde_json::from_value(action.payload.get("support_delivery_refs").cloned().unwrap_or(json!([]))).unwrap_or_default(),
        attempt: action.payload["attempt"].as_u64().unwrap_or(1),
        assessment_ref: None,
        status: AssessmentStatus::Uncertain,
        interpretation: interpretation.into(),
        learner_quote: quote.into(),
        interpreter_version: "source.discrimination.v1".into(),
        supersedes,
        correction: None,
        corrected_at: None,
        source_quotes: json!(sources),
    };
    let accepted = state.user.learning_store_for_workspace(state.learning_workspace_id)?.append_evidence(&evidence)?;
    record_interpretation(state, binding, &accepted)?;
    refresh_learning(state, &binding.tutor_session_id, &binding.source_id)?;
    Ok(
        json!({"evidence_ref":accepted.evidence_id,"status":"observed_source_discrimination","mastery":"unknown",
            "learner_context_update":learner_context(state,binding,&active_objects(&turn_assets(state,binding)?),&[],None,&[])?}),
    )
}

fn learner_context(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    objects: &[Value],
    focus: &[String],
    reference: Option<&str>,
    pending_facts: &[TeachingEvent],
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
    // No invented object target: first turn starts unknown until the Agent chooses one.
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let (projection, watermark) = store.learner_projection(&binding.source_id)?;
    // Keep the original turn's candidate facts, but recompute which ones now
    // have an interpretation. New design/help events must not shift that input
    // window or rewrite the facts and assistance that were frozen with it.
    let input = bound(state, &binding.turn_id)?;
    let mut uninterpreted_fact_refs = Vec::new();
    for id in input.as_ref().into_iter().flat_map(|event| event.payload["learner_context"]["uninterpreted_fact_refs"].as_array().into_iter().flatten()).filter_map(Value::as_str) {
        if !store.fact_has_interpretation(id)? { uninterpreted_fact_refs.push(id.to_owned()); }
    }
    let current_design=design::current_design(state,binding)?;
    let focus_blocks:Vec<String>=serde_json::from_value(current_design["design"].get("focus_block_refs").cloned().unwrap_or(json!([]))).map_err(|e|invalid(e.to_string()))?;
    let (delivered_support,related_interpretation_refs)=store.teaching_learning_opportunity(&binding.tutor_session_id,&binding.source_id,&binding.source_revision,current_design["design_ref"].as_str(),&focus_blocks)?;
    if reference.is_none() && current_design["design"]["target"].is_object() {
        // Keep the smaller step actually delivered under this revision. An
        // unconsumed selection is not a new focus; a new design starts from its
        // own target until it has an actual delivery.
        current_target=delivered_support.iter()
            .find(|delivery| delivery.payload["move"]["design_ref"]==current_design["design_ref"]
                && delivery.payload["move"]["target"].is_object())
            .map(|delivery|delivery.payload["move"]["target"].clone())
            .unwrap_or_else(||current_design["design"]["target"].clone());
        targets=current_target["object_refs"].as_array().into_iter().flatten()
            .filter_map(|object|object["object_id"].as_str().map(str::to_owned)).collect();
    }
    if current_target.is_null() {
        if let Some(delivery)=store.latest_teaching_focus(&binding.tutor_session_id,&binding.source_id)?
            .filter(|delivery|delivery.binding.source_revision==binding.source_revision) {
            current_target=delivery.payload["move"]["target"].clone();
            if targets.is_empty() {
                targets=delivery.payload["move"]["object_ids"].as_array().into_iter().flatten()
                    .filter_map(|id|id.as_str().map(str::to_owned)).collect();
            }
        }
    }
    let pending_invitation=design::invitation_context(state,binding)?;
    let lineage_support=store.teaching_lineage_support(&binding.tutor_session_id,&binding.source_id,&binding.source_revision,current_design["design_ref"].as_str())?;
    let support_context=lineage_support.iter().map(|delivery|design::support_context(state,delivery)).collect::<Result<Vec<_>,_>>()?;
    let learning_opportunity=json!({"design_ref":current_design["design_ref"],"focus_block_refs":focus_blocks,
        "delivered_support":support_context,
        "interpretation_refs":related_interpretation_refs,"pending_response_invitation":pending_invitation,
        "return_context":design::return_context(state,&current_design["design"])?,
        "continuity":continuity::context(state,binding,&current_design,&related_interpretation_refs,pending_facts,reference)?,
        "unknown_when_absent":true});
    let mut interpretations=Vec::new();
    for (_,e) in store.current_design_interpretations(&binding.tutor_session_id,&binding.source_id,&binding.source_revision,current_design["design_ref"].as_str(),&focus_blocks,12)? {
        match store.understanding_basis(binding,&e.evidence_id) {
            Ok(_) => {},
            Err(error) if error.error_code=="TEACHING_INVALID" => continue,
            Err(error) => return Err(error),
        }
        let mut item=json!({"evidence_ref":e.evidence_id,"nature":e.nature,"target":e.target,"interpretation":preview(&json!(e.interpretation),600),
            "teaching_implication":preview(&json!(e.teaching_implication),600),"learner_quote":preview(&json!(e.learner_quote),600),"correction":e.correction,
            "fact_refs":e.fact_refs,"design_ref":e.design_ref,"block_refs":e.block_refs,"status":e.status,"assistance_refs":e.assistance_refs,"support_delivery_refs":e.support_delivery_refs,"attempt":e.attempt});
        timeline::merge_metadata(state,&e,&mut item)?;
        interpretations.push(item);
    }
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
        .take(6).map(|p|json!({"object_id":p["object_id"],"capability":p["capability"],"last_status":p["last_status"],"assistance_count":p["assistance_refs"].as_array().map_or(0,Vec::len)+p["support_delivery_refs"].as_array().map_or(0,Vec::len),"attempt":p["attempt"],"evidence_ref":p["evidence_ref"]})).collect();
    let mut candidates = store.teaching_recent(&binding.tutor_session_id, 8)?;
    candidates.extend(pending_facts.iter().filter(|fact| fact.kind == TeachingFact::LearnerAction).cloned());
    if let Some(id) = reference {
        let referenced = event(state, id)?;
        if !candidates.iter().any(|e| e.event_id == referenced.event_id) { candidates.push(referenced); }
    }
    let mut pending_assessment_refs = Vec::new();
    for action in candidates.iter().filter(|e| e.kind == TeachingFact::LearnerAction
        && e.binding.tutor_session_id == binding.tutor_session_id
        && e.binding.source_id == binding.source_id && e.binding.source_revision == binding.source_revision
        && matches!(e.payload["action"].as_str(), Some("submit" | "revise"))) {
        if store.assessment(&action.event_id)?.is_some() { continue; }
        let delivery = referenced_delivery(state, &action.event_id)?;
        if frozen_contract(state, &delivery)?.is_some_and(|(_, contract)| matches!(contract.rule, ScoringRule::Open { .. })) {
            pending_assessment_refs.push(action.event_id.clone());
        }
    }
    let pending = pending_activity(state)?.map(|delivery| {
        let mut view = json!({"delivery_ref":delivery.event_id,
            "target":delivery.payload["move"]["target"],"prompt":preview(&delivery.payload["move"]["prompt"],600)});
        // The delivered move holds only the public practice view.
        if delivery.payload["move"]["practice"].is_object() { view["practice"] = delivery.payload["move"]["practice"].clone(); }
        view
    });
    let interpretation_refs=interpretations.iter().filter_map(|item|item["evidence_ref"].as_str().map(str::to_owned)).collect::<Vec<_>>();
    let learner_grounding=grounding::context(state,binding,&current_design["design"],&[],&[],&interpretation_refs)?;
    Ok(
        json!({"known_focuses":store.focus_candidates(&binding.source_id,&binding.source_revision,24)?,"entries":entries,"interpretations":interpretations,"learner_grounding":learner_grounding,"uninterpreted_fact_refs":uninterpreted_fact_refs,"current_design":current_design,"learning_opportunity":learning_opportunity,"pending_response_invitation":pending_invitation,"path_progress":paths,"current_target":current_target,"pending_activity":pending,"pending_assessment_refs":pending_assessment_refs,"unknown_when_absent":true,"evidence_watermark":watermark,"projection_watermark":projection.as_ref().map(|p|p.evidence_watermark),
        "stale":projection.as_ref().is_none_or(|p|p.evidence_watermark!=watermark),"details":"Recent interpretations only; tutor.step understanding with after pages current records. evidence_ref reads exact details; trace pages earlier facts and event_id reads exact content."}),
    )
}

pub(crate) fn understanding(
    state: &PrivateBookContext<'_>,
    source: &str,
    object: Option<&str>,
    after: u64,
) -> Result<Value, ToolError> {
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
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
    let interpretations = store.current_interpretations(source,state.book.source_fingerprint(),after,13)?;
    let summaries:Vec<_>=store.understanding_summaries(source,state.book.source_fingerprint())?
        .into_iter().filter(|item|item.later_evidence_ref.is_none()).collect();
    let more = after as usize + 12 < total || interpretations.len() > 12 || after as usize+12<summaries.len();
    let summaries:Vec<_>=summaries.into_iter().skip(after as usize).take(12).collect();
    let interpretations=interpretations.into_iter().take(12).map(|(_,e)| {
        let mut item=json!({"evidence_id":e.evidence_id,"nature":e.nature,"label":e.label,"target":e.target,"interpretation":e.interpretation,"teaching_implication":e.teaching_implication,"correction":e.correction,"learner_quote":e.learner_quote,"fact_refs":e.fact_refs,"assistance_refs":e.assistance_refs,"support_delivery_refs":e.support_delivery_refs,"attempt":e.attempt,"status":e.status});
        timeline::merge_metadata(state,&e,&mut item)?;
        Ok(item)
    }).collect::<Result<Vec<_>,ToolError>>()?;
    let imported_records = state.learning_workspace_id
        .map(|id| crate::learning_record_api::linked_summaries(state.user, id)).transpose()?.unwrap_or_default();
    Ok(
        json!({"rows":rows,"current_understanding":summaries,"interpretations":interpretations,"order":"newest_expression_first","imported_records":imported_records,"next":more.then_some(after+12),"evidence_watermark":watermark,"projection_watermark":projection.as_ref().map(|p|p.evidence_watermark),"stale":projection.as_ref().is_none_or(|p|p.evidence_watermark!=watermark),"estimator_version":projection.as_ref().map(|p|&p.estimator_version)}),
    )
}

pub(crate) fn evidence_view(state: &PrivateBookContext<'_>, id: &str) -> Result<Value, ToolError> {
    retained_evidence_view(state.user, state.learning_workspace_id, &state.book.base.book_id, id)
}

/// Evidence remains readable from its accepted learning scope after the Reader moves on.
/// Both the teaching UI and board delivery use the same displayed-feedback boundary.
pub(crate) fn retained_evidence_view(
    user: &crate::user_runtime::UserRuntime,
    learning_workspace_id: Option<&str>,
    source_id: &str,
    id: &str,
) -> Result<Value, ToolError> {
    let store = user.learning_store_for_workspace(learning_workspace_id)?;
    let evidence = store.evidence(id)?;
    if evidence.source_id != source_id {
        return Err(invalid("请打开此证据的原材料"));
    }
    let (prompt, hidden) = if let Some(delivery_ref) = &evidence.delivery_ref {
        let delivery = store.teaching_event(delivery_ref)?;
        if delivery.kind != TeachingFact::MessageDelivered || delivery.binding.source_id != source_id {
            return Err(invalid("教学交付不属于当前材料"));
        }
        let contract: Option<AssessmentContract> = delivery.payload["assessment_contract_ref"].as_str()
            .map(|id| {
                let selected = store.teaching_event(id)?;
                serde_json::from_value(selected.payload["move"]["assessment"].clone())
                    .map_err(|e| invalid(format!("评分合同不可读：{e}")))
            }).transpose()?;
        let hidden = contract.is_some_and(|contract| contract.feedback == FeedbackPolicy::OnReveal)
            && !store.teaching_activity_events(delivery_ref)?.iter()
                .any(|event| event.kind == TeachingFact::HelpDisplayed && event.payload["action"] == "reveal");
        (delivery.payload["move"]["prompt"].clone(), hidden)
    } else {
        (json!(evidence.target.as_ref().map(|target| &target.expected_performance)), false)
    };
    let mut view = serde_json::to_value(&evidence).unwrap();
    if hidden {
        view["source_quotes"] = Value::Null;
    }
    view["prompt"] = prompt;
    view["assistance_count"] = json!(evidence.assistance_refs.len());
    view["feedback_hidden"] = json!(hidden);
    view.as_object_mut().unwrap().extend(timeline::metadata(user, learning_workspace_id, &evidence)?.as_object().unwrap().clone());
    timeline::expressions(user, learning_workspace_id, &evidence, &mut view)?;
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
            "/tutor/closed-book" => closed_book::route(state,&body,req.now),
            "/tutor/page-help" => page_help::record(state,&body,req.now),
            "/tutor/understanding" => {
                if body["rebuild"] == true {
                    let mut store = state.user.learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)?;
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
            "/tutor/timeline" => timeline::page(state,string("evidence_ref")?,body["after"].as_u64().unwrap_or(0)),
            "/tutor/feedback-displayed" => {
                let action = event(state, string("action_ref")?)?;
                let delivery = referenced_delivery(state, &action.event_id)?;
                let assessment = state
                    .user.learning_store_for_workspace(state.learning_workspace_id)?
                    .assessment(&action.event_id)?
                    .ok_or_else(|| invalid("判定尚未接纳"))?;
                let facts = state
                    .user.learning_store_for_workspace(state.learning_workspace_id)?
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
                let refs = state.user.learning_store_for_workspace(state.learning_workspace_id)?.object_evidence_refs(
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
                    .user.learning_store_for_workspace(state.learning_workspace_id)?
                    .evidence(string("evidence_ref")?)?;
                if previous.source_id != state.book.base.book_id {
                    return Err(invalid("纠正不属于当前材料"));
                }
                let next = state.user.learning_store_for_workspace(state.learning_workspace_id)?.correct_evidence(
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
                    .user.learning_store_for_workspace(state.learning_workspace_id)?
                    .teaching_deliveries(chat, turn)?;
                let mut views = vec![];
                for saved in rows {
                    if saved.binding.source_id != state.book.base.book_id {
                        return Err(invalid("教学活动归属不符"));
                    }
                    if serde_json::to_value(&reference).unwrap()
                        != saved.payload["move"]["presentation"]
                        && !(reference.is_none() && saved.payload["move"]["response_expectation"].is_object())
                    {
                        continue;
                    }
                    let facts = state
                        .user.learning_store_for_workspace(state.learning_workspace_id)?
                        .teaching_activity_events(&saved.event_id)?;
                    views.push(json!({"delivery_ref":saved.event_id,"move":saved.payload["move"],"status":activity_status(state,&saved)?,
                        "response_invitation_status":design::response_status(state,&saved)?,
                        "continuation":continuation_view(state,&saved)?,
                        "assessment":assessment_view(state,&saved,&facts).unwrap_or(json!({"status":"unassessed"})),
                        "steps":question::steps(&facts),
                        "attempts":question::attempts(&facts),
                        "attempted":facts.iter().any(|e| e.kind == TeachingFact::LearnerAction && matches!(e.payload["action"].as_str(),Some("submit"|"revise"))),
                        "help_seen":facts.iter().filter(|e| e.kind == TeachingFact::HelpDisplayed).map(|e| &e.payload).collect::<Vec<_>>(),
                        "help":facts.iter().rev().find(|e| e.kind == TeachingFact::HelpDelivered && e.payload["action"] != "ask_step").map(|e| {
                            let mut help = e.payload.clone(); help["event_id"] = json!(e.event_id); help
                        }),
                        "practice_check":if saved.payload["move"]["practice"].is_object() {
                            json!(state.user.learning_store_for_workspace(state.learning_workspace_id)?.practice_check_verdict(&saved.event_id)?)
                        } else { Value::Null }}));
                }
                Ok(json!({"activities":views}))
            }
            "/tutor/continuation-displayed" => {
                let input: memory::teaching::ContinuationDisplayed = serde_json::from_value(body)
                    .map_err(|e| invalid(e.to_string()))?;
                let saved = delivery(state, &input.delivery_ref)?;
                let view = continuation_view(state, &saved)?.ok_or_else(|| invalid("原交付没有建议"))?;
                if state.selected_chat != Some(saved.binding.chat_session_id.as_str())
                    || input.option_ids.is_empty()
                    || input.recommended_option_id != view.continuation.recommended_option_id
                    || input.option_ids.iter().collect::<BTreeSet<_>>().len() != input.option_ids.len()
                    || input.option_ids.iter().any(|id| !view.continuation.options.iter().any(|o| &o.option_id == id)) {
                    return Err(invalid("建议展示与原交付不符"));
                }
                // One immutable receipt per actual visible ordered subset, including
                // reopening the same set in the presentation workspace.
                let order = input.option_ids.iter().map(|id| view.continuation.options.iter()
                    .position(|o| &o.option_id == id).unwrap().to_string()).collect::<Vec<_>>().join("-");
                let fact = append(state, &saved.binding, format!("continuation-display:{}:{order}",saved.event_id),
                    TeachingFact::ContinuationDisplayed, vec![saved.event_id.clone()], json!(input), req.now)?;
                Ok(json!({"event_id":fact.event_id}))
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
                    .user.learning_store_for_workspace(state.learning_workspace_id)?
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
