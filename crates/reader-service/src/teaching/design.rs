//! Design and ordinary-response links share the existing private teaching trace.
use super::*;
use memory::teaching_design::{TeachingDesign, InvitationDisposition};
use runtime::tutor::TeachingParticipation;

pub(super) fn current_design(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<Value, ToolError> {
    Ok(state.user.learning_store_for_workspace(state.learning_workspace_id)?.current_teaching_design(&binding.tutor_session_id, &binding.source_id, &binding.source_revision)?
        .map(|event| json!({"design_ref":event.event_id,"design":event.payload["design"]})).unwrap_or(Value::Null))
}

fn read_design(state: &PrivateBookContext<'_>, binding: &TeachingBinding, id: &str) -> Result<TeachingDesign, ToolError> {
    let event = event(state,id)?;
    if event.kind != TeachingFact::DesignRevised || event.binding.tutor_session_id != binding.tutor_session_id
        || event.binding.source_id != binding.source_id || event.binding.source_revision != binding.source_revision {
        return Err(invalid("教学设计不属于当前会话和来源版本"));
    }
    serde_json::from_value(event.payload["design"].clone()).map_err(|e|invalid(e.to_string()))
}

fn require_scene_fact(state: &PrivateBookContext<'_>, binding: &TeachingBinding, id: &str) -> Result<TeachingEvent, ToolError> {
    let fact = event(state,id)?;
    if fact.binding.tutor_session_id != binding.tutor_session_id || fact.binding.source_id != binding.source_id
        || fact.binding.source_revision != binding.source_revision
        || matches!(fact.kind,TeachingFact::TurnBound|TeachingFact::DesignRevised|TeachingFact::MoveSelected|TeachingFact::HelpDelivered) {
        return Err(invalid("教学依据需要本会话、来源版本中的实际使用事实；理解表达不能由点击或交付代替"));
    }
    Ok(fact)
}

pub(super) fn prepare(state: &PrivateBookContext<'_>, bind: &TeachingEvent, request: &Value, evidence: &[SourceBinding], ranges: &[EvidenceRange]) -> Result<TeachingDesign, TeachingSelectionPreflightError> {
    let binding = &bind.binding;
    let operation = request["operation_id"].as_str().filter(|id|!id.trim().is_empty()).ok_or_else(||invalid("设计修订需要 operation_id"))?;
    let mut input = request["design"].clone();
    input["target"] = json!(bind_target(input["target"].clone(), binding, evidence)?);
    for block in input["blocks"].as_array_mut().into_iter().flatten() {
        for source in block["source_bindings"].as_array_mut().into_iter().flatten() {
            if source.get("source_id").is_none() { source["source_id"]=json!(binding.source_id); }
            if source.get("source_revision").is_none() { source["source_revision"]=json!(binding.source_revision); }
        }
    }
    let design: TeachingDesign = serde_json::from_value(input).map_err(|e|invalid(e.to_string()))?;
    design.validate(binding)?;
    let id = format!("design:{}:{operation}",binding.turn_id);
    // Exact retries keep the accepted design even after it became the latest revision.
    if let Ok(existing) = event(state,&id) {
        if existing.payload["design"] != json!(design) { return Err(invalid("同一设计操作不能改写已保存修订").into()); }
        return Ok(design);
    }
    let previous = current_design(state,binding)?;
    if design.previous_ref.as_deref() != previous["design_ref"].as_str() {
        return Err(invalid("修订需要引用当前设计的 previous_ref；首次设计省略此字段").into());
    }
    for source in design.target.source_bindings.iter().chain(design.blocks.iter().flat_map(|block|&block.source_bindings)) {
        preflight_target_read(state,&source.start_lid,&source.end_lid,ranges)?;
    }
    Ok(design)
}

pub(super) fn accept(state: &PrivateBookContext<'_>, bind: &TeachingEvent, request: &Value, evidence: &[SourceBinding], ranges: &[EvidenceRange], now: &str) -> Result<Value, ToolError> {
    let design=prepare(state,bind,request,evidence,ranges).map_err(|failure|failure.error)?;
    let binding=&bind.binding;
    let id=format!("design:{}:{}",binding.turn_id,request["operation_id"].as_str().unwrap());
    if let Ok(existing)=event(state,&id) {
        return Ok(json!({"design_ref":existing.event_id,"design":design,"learner_context_update":learner_context(state,binding,&active_objects(&turn_assets(state,binding)?),&[],None,&[])?}));
    }
    let mut causes = vec![bind.event_id.clone()];
    if let Some(id) = &design.previous_ref { read_design(state,binding,id)?; causes.push(id.clone()); }
    for id in &design.basis_refs {
        let (_,facts)=grounding::basis(state,binding,id)?;
        // Reusable understanding may originate in another teaching session;
        // durable causal edges still stay within this session's trace.
        causes.extend(facts.into_iter().filter(|fact|fact.binding.tutor_session_id==binding.tutor_session_id).map(|fact|fact.event_id));
    }
    for id in &design.open_question_refs { state.user.learning_store_for_workspace(state.learning_workspace_id)?.learner_expression_fact(binding,id)?; causes.push(id.clone()); }
    if let Some(back) = &design.return_to {
        if let Some(id)=&back.invitation_ref { let prior=referenced_delivery(state,id)?; if prior.binding.tutor_session_id!=binding.tutor_session_id { return Err(invalid("返回邀请不属于当前会话")); } causes.push(prior.event_id); }
        if let Some(id)=&back.scene_ref {
            let fact=require_scene_fact(state,binding,id).map_err(|_|invalid("return_to.scene_ref 需要携带演示现场的教学 event_id，可用含 presentation_follow_up 的用户 usage 事实；不是 saved_state_ref 或 presentation_id"))?;
            if !fact.payload["presentation_follow_up"].is_object() && !fact.payload["scene"].is_object() {
                return Err(invalid("return_to.scene_ref 对应的教学事实没有保存演示现场，请引用含 presentation_follow_up 或 scene 的事实"));
            }
            causes.push(id.clone());
        }
    }
    causes.sort(); causes.dedup();
    let saved = append(state,binding,id,TeachingFact::DesignRevised,causes,json!({"design":design}),now)?;
    Ok(json!({"design_ref":saved.event_id,"design":design,"learner_context_update":learner_context(state,binding,&active_objects(&turn_assets(state,binding)?),&[],None,&[])?,"instruction":"This private design is accepted. Consume it in the next model turn before selecting or authoring support; it is not a delivery or evidence of learner understanding."}))
}

pub(super) fn validate_move(state: &PrivateBookContext<'_>, bind: &TeachingEvent, movement: &TeachingMove) -> Result<Option<TeachingDesign>, ToolError> {
    let binding=&bind.binding;
    let design=if let Some(id)=movement.design_ref.as_deref() {
        let current=current_design(state,binding)?;
        if current["design_ref"]!=id { return Err(invalid("教学动作必须采用当前设计修订")); }
        let design=read_design(state,binding,id)?;
        for reference in &design.basis_refs { grounding::basis(state,binding,reference)?; }
        Some(design)
    } else {
        if movement.assessment.is_some() || movement.practice.is_some() || movement.presentation.is_some()
            || movement.actions.iter().any(|action|action=="submit" || action=="revise") {
            return Err(invalid("练习、正式活动和教学演示需要接纳并采用教案；普通文字问答可直接提供来源目标"));
        }
        if movement.target.is_none() || !movement.block_refs.is_empty() {
            return Err(invalid("不采用教案的问答需要明确的 target，并省略 block_refs"));
        }
        None
    };
    let participation=movement.participation.as_ref().ok_or_else(||invalid("选择教学动作需要 participation，说明继续帮助、邀请尝试或收束的理由"))?;
    let new_task=movement.response_expectation.is_some() || movement.assessment.is_some()
        || movement.actions.iter().any(|action|action=="submit" || action=="revise");
    let reason=match participation {
        TeachingParticipation::Invite {reason,learner_inference,support} => {
            if learner_inference.trim().is_empty() || support.trim().is_empty() {
                return Err(invalid("邀请尝试需明确留给用户推理的部分和已经提供的支持"));
            }
            if movement.response_expectation.is_none() && movement.assessment.is_none() && !movement.actions.iter().any(|action|action=="submit") {
                return Err(invalid("邀请尝试需要 response_expectation 或可提交的活动，只有 question 类型不会建立响应入口"));
            }
            if movement.interaction_intent=="direct_explanation" { return Err(invalid("用户选择直接讲解时不要夹带新的作答任务")); }
            reason
        }
        TeachingParticipation::Support {reason,support,learner_inference} => {
            if support.trim().is_empty() { return Err(invalid("继续帮助需要说明本轮实际补给的连接")); }
            if learner_inference.as_deref().is_some_and(|inference|inference.trim().is_empty()) {
                return Err(invalid("预留推理应说明实际留给用户的连接；没有可预留的推理时使用 null"));
            }
            if new_task { return Err(invalid("继续帮助或收束不能同时创建新的作答任务；需要用户尝试时选择 invite")); }
            reason
        }
        TeachingParticipation::Conclude {reason} => {
            if new_task { return Err(invalid("继续帮助或收束不能同时创建新的作答任务；需要用户尝试时选择 invite")); }
            reason
        }
    };
    if reason.trim().is_empty() { return Err(invalid("参与选择需要依据当前理解和帮助情况说明理由")); }
    if let Some(design)=&design {
        if movement.block_refs.is_empty() { return Err(invalid("采用教案的教学动作需要说明服务的关系")); }
        design.validate_focus(&movement.block_refs)?;
    }
    for reference in &movement.interpretation_refs {
        let (interpretation,_)=state.user.learning_store_for_workspace(state.learning_workspace_id)?.understanding_basis(binding,reference)?;
        if let (Some(evidence_design),Some(id))=(&interpretation.design_ref,movement.design_ref.as_deref()) {
            let mut ancestor=Some(id.to_owned());
            let mut found=false;
            while let Some(reference)=ancestor {
                if &reference==evidence_design {found=true;break;}
                ancestor=read_design(state,binding,&reference)?.previous_ref;
            }
            if !found || !interpretation.block_refs.iter().any(|block|movement.block_refs.contains(block)) {
                return Err(invalid("所采用理解判断不属于当前设计谱系和本轮关系"));
            }
        }
    }
    if let Some(expectation)=&movement.response_expectation {
        if expectation.public_prompt.trim().is_empty() { return Err(invalid("普通表达邀请需要公开问句")); }
        if let Some(design)=&design {
            if expectation.block_refs.is_empty() { return Err(invalid("教案内的表达邀请需要关联关系")); }
            design.validate_blocks(&expectation.block_refs)?;
        }
        if expectation.block_refs.iter().any(|id| !movement.block_refs.contains(id)) {
            return Err(invalid("表达邀请的关系必须属于本次教学动作；不要在邀请中扩大推理范围"));
        }
        if movement.continuation.is_some() || movement.assessment.is_some() { return Err(invalid("普通表达邀请与下一步菜单或正式评分分别交付，避免重复任务")); }
    }
    let pending=bind.payload["learner_context"]["pending_response_invitation"]["delivery_ref"].as_str();
    if let Some(response)=&movement.responds_to {
        for fact in &response.fact_refs { state.user.learning_store_for_workspace(state.learning_workspace_id)?.learner_expression_fact(binding,fact)?; }
        if let Some(invitation)=&response.invitation_ref {
            let original=referenced_delivery(state,invitation)?;
            if original.binding.tutor_session_id!=binding.tutor_session_id || original.payload["move"]["response_expectation"].is_null() {
                return Err(invalid("回应需要当前会话的已交付普通邀请"));
            }
        }
        if pending.is_some() && response.invitation_ref.as_deref()!=pending { return Err(invalid("请明确继续、处理或暂缓当前未接完的表达邀请")); }
        if movement.response_expectation.is_some() && response.invitation_ref.is_some() && response.disposition==InvitationDisposition::Continue {
            return Err(invalid("沿用邀请时不要另发重复邀请；替换时将旧邀请记为 addressed 或 deferred"));
        }
        if matches!(participation,TeachingParticipation::Conclude {..}) && response.invitation_ref.is_some()
            && response.disposition==InvitationDisposition::Continue {
            return Err(invalid("收束时应处理或暂缓现有邀请，不能留下 continue 的待答任务"));
        }
    } else if pending.is_some() { return Err(invalid("当前有未接完的普通邀请，请用 responds_to 说明承接或暂缓")); }
    Ok(design)
}

pub(super) fn invitation_context(state: &PrivateBookContext<'_>, binding: &TeachingBinding) -> Result<Value, ToolError> {
    Ok(state.user.learning_store_for_workspace(state.learning_workspace_id)?.pending_response_invitation_in_chat(binding)?
        .map(|delivery|json!({"delivery_ref":delivery.event_id,"design_ref":delivery.payload["move"]["design_ref"],
            "response_expectation":delivery.payload["move"]["response_expectation"],"target":delivery.payload["move"]["target"]})).unwrap_or(Value::Null))
}

fn presentation_context(presentation: &Value) -> Value {
    if !presentation.is_object() { return Value::Null; }
    json!({"reference":presentation["reference"],"teaching_binding":presentation["content"]["teaching_binding"],
        "title":presentation["content"]["title"],"readable_content_preview":preview(&presentation["content"]["readable_content"],1200),
        "evidence_boundary":"Delivered page content is available support, not proof that the learner read or understood it."})
}

/// Resolve the immutable receipt carried by this fact, including after a focus
/// revision or later interaction with the same page. Rendered results remain
/// scene context; only the learner's authored words supply an expression.
fn scene_context(state: &PrivateBookContext<'_>, fact: &TeachingEvent) -> Result<Value, ToolError> {
    if !fact.payload["presentation_follow_up"].is_object() && !fact.payload["scene"].is_object() { return Ok(Value::Null); }
    let readable=readable_fact(state,fact)?;
    let scene=&readable["scene"];
    let receipt:runtime::presentation::PresentationFollowUp=serde_json::from_value(scene["receipt"].clone()).map_err(|e|invalid(e.to_string()))?;
    let presentation=state.read_presentation(&receipt.session_id,&receipt.reference)?;
    Ok(json!({"receipt":receipt,"presentation":presentation_context(&json!(presentation)),
        "values_preview":preview(&scene["state"]["values"],1200),"visible_step":scene["state"]["visible_step"],
        "observed_result_preview":preview(&scene["state"]["observed_result"],1200),"source_ref_ids":scene["state"]["source_ref_ids"],
        "evidence_boundary":"This saved browser scene records controls and rendered results, not that the learner noticed, explained or understood them. Unedited page prefill is not an independent learner expression."}))
}

/// Project authored words with their own immutable help conditions. Duplicate
/// input facts may share a turn, but distinct words or help receipts stay apart.
fn expression_contexts(state: &PrivateBookContext<'_>, binding: &TeachingBinding, facts: Vec<TeachingEvent>) -> Result<Vec<Value>, ToolError> {
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let mut expressions=Vec::<Value>::new();
    let mut seen_turns=std::collections::BTreeMap::new();
    for fact in facts {
        if fact.learner_expression().is_none() { continue; }
        if fact.binding.tutor_session_id!=binding.tutor_session_id
            || fact.binding.source_id!=binding.source_id || fact.binding.source_revision!=binding.source_revision { continue; }
        let scene=scene_context(state,&fact)?;
        let interpretations=store.teaching_expression_interpretations(&fact.event_id)?.into_iter().map(|e|
            json!({"evidence_ref":e.evidence_id,"design_ref":e.design_ref,"block_refs":e.block_refs,"nature":e.nature,"status":e.status,
                "interpretation":preview(&json!(e.interpretation),600),"teaching_implication":preview(&json!(e.teaching_implication),600),
                "learner_quote":preview(&json!(e.learner_quote),600),"fact_refs":e.fact_refs,"assistance_refs":e.assistance_refs})).collect::<Vec<_>>();
        let turn=(fact.binding.chat_session_id.clone(),fact.binding.turn_id.clone(),
            fact.learner_expression().map(str::to_owned),fact.payload["support_delivery_refs"].to_string(),
            fact.payload["assistance_refs"].to_string(),fact.payload["invitation_context_ref"].to_string());
        if let Some(index)=seen_turns.get(&turn).copied() {
            let expression:&mut Value=&mut expressions[index];
            expression["fact_refs"].as_array_mut().unwrap().push(json!(fact.event_id));
            if expression["scene"].is_null() { expression["scene"]=scene; }
            let existing=expression["interpretations"].as_array_mut().unwrap();
            for interpretation in interpretations {
                if !existing.iter().any(|item|item["evidence_ref"]==interpretation["evidence_ref"]) { existing.push(interpretation); }
            }
            continue;
        }
        seen_turns.insert(turn,expressions.len());
        expressions.push(json!({"fact_ref":fact.event_id,"fact_refs":[fact.event_id],"expression_preview":preview(&json!(fact.learner_expression()),600),
            "response_origin":fact.payload["response_origin"],
            "support_delivery_refs":fact.payload["support_delivery_refs"],"assistance_refs":fact.payload["assistance_refs"],
            "invitation_context_ref":fact.payload["invitation_context_ref"],"scene":scene,"interpretations":interpretations}));
    }
    Ok(expressions)
}

/// The private reasoning allocation belongs to the accepted selection. The
/// delivered public move deliberately omits it; follow its durable causal link.
pub(super) fn support_context(state: &PrivateBookContext<'_>, delivery: &TeachingEvent) -> Result<Value, ToolError> {
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let mut participation=Value::Null;
    for reference in &delivery.causal_refs {
        let selected=store.teaching_event(reference)?;
        if selected.kind==TeachingFact::MoveSelected && selected.binding==delivery.binding {
            participation=selected.payload["move"]["participation"].clone();
            break;
        }
    }
    let expressions=expression_contexts(state,&delivery.binding,store.teaching_supported_expressions(&delivery.event_id)?)?;
    let responding_to=delivery.payload["move"]["responds_to"]["fact_refs"].as_array().into_iter().flatten()
        .filter_map(Value::as_str).map(|id|store.teaching_event(id)).collect::<Result<Vec<_>,_>>()?;
    let responding_to_expressions=expression_contexts(state,&delivery.binding,responding_to)?;
    let activity=store.teaching_activity_events(&delivery.event_id)?;
    let teacher_actions=activity.iter().filter(|fact|fact.payload["usage"]["kind"]=="teacher_presentation_action")
        .rev().take(6).map(|fact|json!({"fact_ref":fact.event_id,"actor":"teacher","action":fact.payload["action"],
            "before_receipt":fact.payload["before_receipt"],"after_receipt":fact.payload["scene"]["receipt"]})).collect::<Vec<_>>();
    let displayed_help=activity.into_iter().rev()
        .filter(|fact|fact.kind==TeachingFact::HelpDisplayed).take(4).map(|fact|Ok(json!({
            "fact_ref":fact.event_id,"text_preview":preview(&fact.payload["text"],600),"scene":scene_context(state,&fact)?,
            "actor":fact.payload["actor"],"action_ref":fact.payload["action_ref"],"before_receipt":fact.payload["before_receipt"]})))
        .collect::<Result<Vec<Value>,ToolError>>()?;
    Ok(json!({"delivery_ref":delivery.event_id,"design_ref":delivery.payload["move"]["design_ref"],
        "original_ref":{"kind":"chat_turn","session_id":delivery.binding.chat_session_id,"turn_id":delivery.binding.turn_id},
        "block_refs":delivery.payload["move"]["block_refs"],"target":delivery.payload["move"]["target"],
        "participation":participation,"answer_preview":preview(&delivery.payload["answer"],1200),
        "response_expectation":delivery.payload["move"]["response_expectation"],"presentation":delivery.payload["move"]["presentation"],
        "presentation_support":presentation_context(&delivery.payload["presentation"]),
        "displayed_help":displayed_help,"teacher_actions":teacher_actions,"responding_to_expressions":responding_to_expressions,"learner_expressions":expressions}))
}

/// Resolve only the return links the teacher actually saved. These are earlier
/// questions and their support conditions, not evidence that the learner can
/// already return to the wider task. Exact facts remain readable through trace.
pub(super) fn return_context(state: &PrivateBookContext<'_>, design: &Value) -> Result<Value, ToolError> {
    let question_refs:Vec<_>=design["open_question_refs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let back=&design["return_to"];
    if question_refs.is_empty() && !back.is_object() { return Ok(Value::Null); }
    let mut invitation_refs=Vec::<String>::new();
    if let Some(id)=back["invitation_ref"].as_str() { invitation_refs.push(id.to_owned()); }
    let mut support_refs=Vec::<String>::new();
    let mut questions=Vec::new();
    for id in question_refs.iter().take(4) {
        let fact=event(state,id)?;
        if let Some(invitation)=fact.payload["invitation_context_ref"].as_str() {
            if !invitation_refs.iter().any(|id|id==invitation) { invitation_refs.push(invitation.to_owned()); }
        }
        for reference in ["support_delivery_refs","assistance_refs"].iter()
            .flat_map(|field|fact.payload[*field].as_array().into_iter().flatten()).filter_map(Value::as_str) {
            if !support_refs.iter().any(|id|id==reference) { support_refs.push(reference.to_owned()); }
        }
        questions.push(json!({"fact_ref":fact.event_id,"expression_preview":preview(&json!(fact.learner_expression()),600),
            "invitation_context_ref":fact.payload["invitation_context_ref"],"support_delivery_refs":fact.payload["support_delivery_refs"],
            "assistance_refs":fact.payload["assistance_refs"],"scene":scene_context(state,&fact)?}));
    }
    let scene=if let Some(id)=back["scene_ref"].as_str() {
        let fact=event(state,id)?;
        if let Some(invitation)=fact.payload["invitation_context_ref"].as_str() {
            if !invitation_refs.iter().any(|id|id==invitation) { invitation_refs.push(invitation.to_owned()); }
        }
        for reference in ["support_delivery_refs","assistance_refs"].iter()
            .flat_map(|field|fact.payload[*field].as_array().into_iter().flatten()).filter_map(Value::as_str) {
            if !support_refs.iter().any(|id|id==reference) { support_refs.push(reference.to_owned()); }
        }
        Some(json!({"fact_ref":fact.event_id,"presentation_follow_up":fact.payload["presentation_follow_up"],
            "scene":scene_context(state,&fact)?,"expression_preview":preview(&json!(fact.learner_expression()),600),
            "invitation_context_ref":fact.payload["invitation_context_ref"],"support_delivery_refs":fact.payload["support_delivery_refs"],
            "assistance_refs":fact.payload["assistance_refs"]}))
    } else { None };
    let mut invitations=Vec::new();
    for id in invitation_refs.iter().take(3) {
        let delivery=referenced_delivery(state,id)?;
        invitations.push(json!({"delivery_ref":delivery.event_id,"design_ref":delivery.payload["move"]["design_ref"],
            "block_refs":delivery.payload["move"]["block_refs"],"target":delivery.payload["move"]["target"],
            "response_expectation":delivery.payload["move"]["response_expectation"],
            "prompt_preview":preview(&delivery.payload["move"]["prompt"],600),"answer_preview":preview(&delivery.payload["answer"],1200),
            "presentation":presentation_context(&delivery.payload["presentation"])}));
    }
    let mut support=Vec::new();
    for id in support_refs.iter().rev().filter(|id|!invitation_refs.contains(id)).take(3) {
        let fact=event(state,id)?;
        let text=if fact.kind==TeachingFact::MessageDelivered {&fact.payload["answer"]} else {&fact.payload["text"]};
        support.push(json!({"event_id":fact.event_id,"kind":fact.kind,"content_preview":preview(text,600),
            "target":fact.payload["move"]["target"],"block_refs":fact.payload["move"]["block_refs"],
            "presentation":presentation_context(&fact.payload["presentation"]),"scene":scene_context(state,&fact)?}));
    }
    Ok(json!({"block_refs":back["block_refs"],"open_question_refs":question_refs,"open_questions":questions,
        "invitation_refs":invitation_refs,"invitations":invitations,"support_refs":support_refs,"support":support,"scene":scene}))
}

/// Input acceptance freezes the prior support, without classifying this message as an answer.
pub(super) fn freeze_input_support(state: &PrivateBookContext<'_>, binding: &TeachingBinding, facts: &mut [TeachingEvent], request: &Value, interaction: Option<&runtime::tutor_continuation::ResolvedInteraction>) -> Result<(), ToolError> {
    let invitation = if let Some(reference)=request["teaching_ref"].as_str() { Some(referenced_delivery(state,reference)?) }
        else if let Some(action)=interaction.filter(|a|!a.user_supplement.trim().is_empty()) {
            pending_activity_help(state,action)?.first().map(|id|referenced_delivery(state,id)).transpose()?
                .or(state.user.learning_store_for_workspace(state.learning_workspace_id)?.pending_response_invitation_in_chat(binding)?)
        } else { state.user.learning_store_for_workspace(state.learning_workspace_id)?.pending_response_invitation_in_chat(binding)? };
    let (mut help,mut support)=if let Some(invitation)=&invitation {
        let prior=state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_activity_events(&invitation.event_id)?;
        let help:Vec<_>=prior.iter().filter(|fact|fact.kind==TeachingFact::HelpDisplayed).map(|fact|fact.event_id.clone()).collect();
        let support:Vec<_>=std::iter::once(invitation.event_id.clone()).chain(prior.iter().filter(|fact|fact.kind==TeachingFact::MessageDelivered).map(|fact|fact.event_id.clone())).collect();
        (help,support)
    } else {
        let support=state.user.learning_store_for_workspace(state.learning_workspace_id)?.latest_chat_teaching_support(&binding.tutor_session_id,&binding.source_id,&binding.source_revision,&binding.chat_session_id)?;
        let mut help=Vec::new();
        for delivery in &support {
            for fact in state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_activity_events(&delivery.event_id)? {
                if fact.kind==TeachingFact::HelpDisplayed && !help.contains(&fact.event_id) { help.push(fact.event_id); }
            }
        }
        (help,support.into_iter().map(|delivery|delivery.event_id).collect())
    };
    // A follow-up can revisit an older page after another chat explanation.
    // Include help from that exact page alongside the current chat support.
    if let Some(receipt)=request.get("presentation_follow_up").filter(|value|value.is_object()) {
        let receipt:runtime::presentation::PresentationFollowUp=serde_json::from_value(receipt.clone()).map_err(|e|invalid(e.to_string()))?;
        for delivery in state.user.learning_store_for_workspace(state.learning_workspace_id)?
            .teaching_deliveries(&receipt.session_id,&receipt.turn_id)?.into_iter().filter(|fact|
                fact.binding.tutor_session_id==binding.tutor_session_id
                    && fact.payload["move"]["presentation"]==json!(receipt.reference)) {
            if !support.contains(&delivery.event_id) { support.push(delivery.event_id.clone()); }
            for fact in state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_activity_events(&delivery.event_id)? {
                if fact.kind==TeachingFact::HelpDisplayed && !help.contains(&fact.event_id) { help.push(fact.event_id); }
            }
        }
    }
    // Admission supplied exact course facts; preserve help from the left pane
    // alongside the same chat's assistance when interpreting a follow-up.
    for raw in request["course_context"]["prior_help"].as_array().into_iter().flatten() {
        let fact: TeachingEvent = serde_json::from_value(raw.clone()).map_err(|e| invalid(e.to_string()))?;
        if fact.binding.source_id != binding.source_id || fact.binding.source_revision != binding.source_revision { continue; }
        if fact.kind == TeachingFact::HelpDisplayed && !help.contains(&fact.event_id) { help.push(fact.event_id.clone()); }
        if !support.contains(&fact.event_id) { support.push(fact.event_id); }
    }
    if support.is_empty() { return Ok(()); }
    // A practice card can forward its submitted answer or reasoning after the
    // solution has appeared. Both receipts for that text belong to the earlier
    // submission, even though the chat has acquired more help by this point.
    let submission = facts.iter().find(|fact| fact.binding.turn_id == binding.turn_id
        && fact.kind == TeachingFact::LearnerAction && fact.payload["submission_field"].is_string()).cloned();
    for fact in facts.iter_mut().filter(|fact|fact.binding.turn_id==binding.turn_id && (fact.kind==TeachingFact::LearnerAction
        || fact.kind==TeachingFact::UsageObserved && matches!(fact.payload["usage"]["kind"].as_str(),Some("user_message"|"reading_action")))) {
        if let Some(invitation)=&invitation { fact.payload["invitation_context_ref"]=json!(invitation.event_id); }
        if let Some(original) = submission.as_ref().filter(|original| fact.event_id == original.event_id
            || fact.learner_expression().is_some_and(|text| Some(text) == original.payload["response"].as_str())) {
            fact.payload["assistance_refs"] = original.payload["assistance_refs"].clone();
            fact.payload["support_delivery_refs"] = original.payload["support_delivery_refs"].clone();
            fact.payload["attempt"] = original.payload["attempt"].clone();
            continue;
        }
        fact.payload["assistance_refs"]=json!(help);
        fact.payload["support_delivery_refs"]=json!(support);
    }
    Ok(())
}

pub(super) fn response_status(state: &PrivateBookContext<'_>, saved: &TeachingEvent) -> Result<Option<&'static str>,ToolError> {
    if !saved.payload["move"]["response_expectation"].is_object() { return Ok(None); }
    let mut after=0;
    let mut status="open";
    loop {
        let events=state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_events(&saved.binding.tutor_session_id,after,100)?;
        if events.is_empty() { break; }
        for (seq,event) in events {
            after=seq;
            if event.kind==TeachingFact::MessageDelivered && event.payload["move"]["responds_to"]["invitation_ref"]==saved.event_id {
                status=match event.payload["move"]["responds_to"]["disposition"].as_str() { Some("addressed")=>"addressed",Some("deferred")=>"deferred",_=>status };
            }
        }
    }
    Ok(Some(status))
}
