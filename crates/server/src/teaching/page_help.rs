//! Page help joins the same input-time assistance history as chat help.
use super::*;

/// A teacher's completed operation is usage, and visible teaching regions are
/// help. Neither is a learner submission. Receipts keep both scenes immutable.
pub(crate) fn record_presentation_action(
    state: &PrivateBookContext<'_>,
    command: &runtime::presentation_interact::PresentationCommand,
    before: &runtime::presentation::PresentationFollowUp,
    after: &runtime::presentation::PresentationFollowUp,
    support: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    use runtime::presentation_interact::{InteractRequest, PresentationAction};
    let InteractRequest::Act { action, .. } = &command.request else { return Ok(Value::Null); };
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let deliveries = store.teaching_deliveries(&command.session_id, &command.delivery_turn_id)?
        .into_iter().filter(|fact| fact.payload["move"]["presentation"] == json!(after.reference)).collect::<Vec<_>>();
    let binding = deliveries.first().map(|fact| fact.binding.clone())
        .or(bound(state, &command.run_turn_id)?.map(|fact| fact.binding));
    let Some(binding) = binding else { return Ok(Value::Null); };
    let version = state.read_presentation(&command.session_id, &after.reference)?;
    let scene = state.read_presentation_state(after)?;
    let previous = state.read_presentation_state(before)?;
    let mut shown = Vec::new();
    for item in support.as_array().into_iter().flatten() {
        let region_id = item["region_id"].as_str().ok_or_else(|| invalid("演示帮助缺少区域"))?;
        let region = version.content.teaching_binding.as_ref().and_then(|binding|
            binding.regions.iter().find(|region| region.region_id == region_id))
            .ok_or_else(|| invalid("演示帮助需要该版本声明的教学区域"))?;
        let relevant = match action {
            PresentationAction::Highlight { .. } => false,
            PresentationAction::Reveal { region_id: target } | PresentationAction::Annotate { region_id: target } => target == region_id,
            _ => region.state_fields.iter().any(|key| previous.state.values["page"][key] != scene.state.values["page"][key]),
        };
        let text = item["text"].as_str().map(str::trim).filter(|text| !text.is_empty())
            .ok_or_else(|| invalid("演示帮助内容为空"))?;
        if !relevant || !scene.state.observed_result.contains(text) {
            return Err(invalid("帮助内容不属于本次动作的可见教学现场"));
        }
        shown.push(json!({"region_id":region_id,"text":text,"block_refs":region.block_refs}));
    }
    let action_ref = format!("presentation-action:{}", command.command_id);
    let operation = append(state, &binding, action_ref.clone(), TeachingFact::UsageObserved,
        deliveries.iter().map(|fact| fact.event_id.clone()).collect(),
        json!({"usage":{"kind":"teacher_presentation_action"},"actor":"teacher","command_id":command.command_id,
            "run_turn_id":command.run_turn_id,"action":action,"before_receipt":before,"scene":scene,
            "delivery_ref":deliveries.first().map(|fact| &fact.event_id)}), now)?;
    let mut help_refs = Vec::new();
    if !shown.is_empty() {
        let text = shown.iter().filter_map(|item| item["text"].as_str()).collect::<Vec<_>>().join("\n");
        for delivery in &deliveries {
            let payload = json!({"delivery_ref":delivery.event_id,"action":if matches!(action, PresentationAction::Reveal{..}|PresentationAction::Annotate{..}) {"reveal"} else {"demonstration"},
                "actor":"teacher","origin":"presentation_interaction","text":text,"regions":shown,
                "action_ref":action_ref,"before_receipt":before,"scene":scene});
            let offered = append(state, &delivery.binding, format!("help:{action_ref}:{}", delivery.event_id),
                TeachingFact::HelpDelivered, vec![operation.event_id.clone(), delivery.event_id.clone()], payload.clone(), now)?;
            let seen = append(state, &delivery.binding, format!("seen:{}", offered.event_id),
                TeachingFact::HelpDisplayed, vec![offered.event_id], payload, now)?;
            help_refs.push(seen.event_id);
        }
    }
    Ok(json!({"action_ref":operation.event_id,"delivery_refs":deliveries.iter().map(|fact|&fact.event_id).collect::<Vec<_>>(),
        "assistance_refs":help_refs,"actor":"teacher"}))
}

pub(super) fn record(state: &PrivateBookContext<'_>, body: &Value, now: &str) -> Result<Value, ToolError> {
    let delivery_ref = body["delivery_ref"].as_str().ok_or_else(|| invalid("缺少教学交付引用"))?;
    let saved = delivery(state, delivery_ref)?;
    let receipt: runtime::presentation::PresentationFollowUp = serde_json::from_value(body["presentation_follow_up"].clone())
        .map_err(|_| invalid("页面帮助需要已保存的现场"))?;
    let scene = scene_snapshot(state, &saved, Some(&receipt))?;
    let text = body["text"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| invalid("页面帮助内容为空"))?;
    if !scene["state"]["observed_result"].as_str().unwrap_or_default().contains(text) {
        return Err(invalid("帮助文字不在这次保存的可见现场中"));
    }
    let action = match body["kind"].as_str() {
        Some("hint") => "request_hint",
        Some("explanation") => "reveal",
        _ => return Err(invalid("未知页面帮助类型")),
    };
    let id = format!("page-help:{}:{}:{}", saved.event_id, receipt.state_revision, action);
    let payload = json!({"delivery_ref":saved.event_id,"action":action,"text":text,"scene":scene,"origin":"presentation"});
    let offered = append(state, &saved.binding, id.clone(), TeachingFact::HelpDelivered,
        vec![saved.event_id.clone()], payload.clone(), now)?;
    let seen = append(state, &saved.binding, format!("seen:{id}"), TeachingFact::HelpDisplayed,
        vec![offered.event_id], payload, now)?;
    Ok(json!({"event_id":seen.event_id}))
}
