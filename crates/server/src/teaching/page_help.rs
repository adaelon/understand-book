//! Page help joins the same input-time assistance history as chat help.
use super::*;

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
