//! Business availability and terminal request facts; no private chat content is reported.
use crate::{authorization::Authorization, model_rates::ModelRates, AgentAssistantStatus, AgentChatSession, AgentChatTurn};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub(crate) fn model_health(rates: Option<&ModelRates>, load_error: Option<&str>, storage_ready: bool,
    binding: Option<(&str, &str)>, now: i64) -> Value {
    let problem = |code: &str, message: &str| json!({"status":"unavailable","can_answer":false,
        "code":code,"message":message,"checked_at":now,"coverage_until":null,
        "model":binding.map(|(_,model)|model)});
    let Some((provider, model)) = binding else {
        return problem("PROVIDER_NOT_CONFIGURED", "尚未配置问答模型");
    };
    if let Some(code) = load_error {
        return problem(code, match code {
            "MODEL_RATE_FILE_UNREADABLE" => "服务账号无法读取模型费率文件，请检查文件是否存在及读取权限",
            "MODEL_RATE_FILE_INVALID_JSON" => "模型费率文件格式错误，服务未能加载",
            _ => "模型费率配置无效，请检查重叠时段、版本号和必填项",
        });
    }
    if !storage_ready { return problem("MODEL_SPEND_STORAGE_UNAVAILABLE", "模型计费存储尚未就绪"); }
    let Some(rates) = rates else { return problem("MODEL_RATE_CONFIGURATION_INVALID", "模型费率未加载"); };
    let mut matching: Vec<_> = rates.rates.iter().filter(|r|r.provider==provider && r.model==model).collect();
    matching.sort_by_key(|r|r.starts_at);
    if matching.is_empty() { return problem("MODEL_RATE_MODEL_MISMATCH", "当前供应商和模型没有对应费率"); }
    let Some(index) = matching.iter().position(|r|r.starts_at<=now && now<r.expires_at) else {
        let expired = matching.last().is_some_and(|r|r.expires_at<=now);
        return problem(if expired {"MODEL_RATE_EXPIRED"} else {"MODEL_RATE_COVERAGE_GAP"},
            if expired {"模型费率已到期，所有新的模型调用均被停止"} else {"当前时刻没有适用费率，模型调用被停止"});
    };
    let mut until = matching[index].expires_at;
    for next in &matching[index+1..] {
        if next.starts_at != until { break; }
        until = next.expires_at;
    }
    let remaining = until - now;
    let (status, code, message) = if remaining <= 86400 {
        ("critical", "MODEL_RATE_EXPIRES_WITHIN_DAY", "费率将在 24 小时内失效，需要立即续接")
    } else if remaining <= 7*86400 {
        ("warning", "MODEL_RATE_EXPIRES_WITHIN_WEEK", "费率将在 7 天内失效，请安排续接")
    } else { ("ready", "MODEL_READY", "问答模型与计费配置可用") };
    json!({"status":status,"can_answer":true,"code":code,"message":message,"checked_at":now,
        "coverage_until":until,"remaining_seconds":remaining,"model":model})
}

fn storage(error: rusqlite::Error) -> ToolError { crate::allowance_admin::storage(error) }
fn probe(db: &Connection, owner: &str, key: &str) -> Result<bool, ToolError> {
    if !key.starts_with("ops-probe-") { return Ok(false); }
    db.query_row("SELECT is_admin FROM users WHERE user_id=?",[owner],|r|r.get(0)).map_err(storage)
}
fn stage(code: Option<&str>) -> &'static str {
    match code {
        Some(c) if c.contains("UNSAVED") || c.contains("STORAGE") || c.contains("PERSIST") => "save",
        Some(c) if c.starts_with("MODEL_RATE") || c.starts_with("MODEL_ALLOWANCE") || c.contains("ALLOWANCE") || c=="PROVIDER_NOT_CONFIGURED" => "preflight",
        Some(c) if c.starts_with("PROVIDER") || c.starts_with("MODEL_") => "model",
        Some(c) if c.contains("PRESENTATION") || c.contains("BOARD") || c.contains("DELIVERY") => "delivery",
        Some(_) => "execution",
        None => "delivery",
    }
}
fn account_block(code: &str) -> bool {
    code.contains("ALLOWANCE")
}
fn recovery_scope(code: Option<&str>, stage: &str, owner: &str, book: &str) -> String {
    let code=code.unwrap_or("");
    if stage=="save" { format!("save:{owner}") }
    else if code.contains("BOARD") { "board".into() }
    else if code.contains("PRESENTATION") { "presentation".into() }
    else if code.starts_with("WEB_") { "web".into() }
    else if stage=="model" || stage=="preflight" { "model".into() }
    else if code.starts_with("VIDEO_") { format!("video:{book}") }
    else { format!("execution:{book}") }
}
fn successful_activity(turn: &AgentChatTurn, name: &str) -> bool {
    turn.run_summary.as_ref().and_then(|s|s.activities.as_ref()).is_some_and(|activities|
        activities.iter().any(|a|a.name==name && a.status==runtime::run_events::ActivityStatus::Succeeded))
}
fn video_attempted(turn: &AgentChatTurn) -> bool {
    turn.video_question.is_some() || turn.run_summary.as_ref().is_some_and(|s|s.trace.iter().any(|t|t.tool.starts_with("video.")))
}

pub(crate) fn save_outcome(db: &Connection, owner: &str, session: &AgentChatSession, turn: &AgentChatTurn, now: i64) -> Result<(), ToolError> {
    if turn.status == AgentAssistantStatus::PendingAssistant { return Ok(()); }
    let code = turn.error.as_ref().map(|e|e.error_code.as_str())
        .or_else(|| turn.outcome.as_ref().and_then(|o|o.warning.as_deref()));
    let outcome = if crate::admin_usage::delivered(session, turn) { "delivered" }
        else if turn.status == AgentAssistantStatus::Cancelled { "cancelled" }
        else if code.is_some_and(account_block) { "account_blocked" }
        else if turn.status == AgentAssistantStatus::Failed { "failed" }
        else { "partial" };
    write_turn(db, owner, &turn.turn_id, outcome, stage(code), code, now)?;
    let mut scope=recovery_scope(code,stage(code),owner,&session.book_id);
    if scope.starts_with("execution:") && video_attempted(turn) { scope=format!("video:{}",session.book_id); }
    if scope=="web" {
        if let Some(name)=turn.run_summary.as_ref().and_then(|s|s.activities.as_ref()).and_then(|a|a.iter().rev().find(|a|a.name.starts_with("web.") && a.error_code.is_some())).map(|a|&a.name) {
            scope=format!("tool:{name}");
        }
    }
    db.execute("UPDATE operational_outcomes SET recovery_scope=? WHERE user_id=? AND turn_id=?",params![scope,owner,turn.turn_id]).map_err(storage)?;
    if outcome=="delivered" {
        let at: Option<i64>=db.query_row("SELECT finished_at FROM operational_outcomes WHERE user_id=? AND turn_id=?",params![owner,turn.turn_id],|r|r.get(0)).optional().map_err(storage)?;
        if let Some(at)=at {
            let mut scopes=vec!["model".into(),format!("save:{owner}")];
            if turn.board_delivery.is_some() { scopes.push("board".into()); }
            if turn.outcome.as_ref().and_then(|o|o.answer_view.as_ref()).is_some_and(|v|v.parts.iter().any(|p|matches!(p,runtime::orchestrator::AgentAnswerPart::Presentation { .. }))) { scopes.push("presentation".into()); }
            for name in ["web.search","web.fetch"] { if successful_activity(turn,name) { scopes.push(format!("tool:{name}")); } }
            if successful_activity(turn,"web.search") && successful_activity(turn,"web.fetch") { scopes.push("web".into()); }
            if video_attempted(turn) {
                if successful_activity(turn,"video.read_frames") || successful_activity(turn,"video.review_audio")
                    || successful_activity(turn,"book.text") || successful_activity(turn,"book.text_batch")
                    || (successful_activity(turn,"library.read") && turn.source_bindings.iter().any(|s|s.book_id==session.book_id)) {
                    scopes.push(format!("video:{}",session.book_id));
                }
            } else { scopes.push(format!("execution:{}",session.book_id)); }
            for scope in scopes {
                db.execute("INSERT INTO operational_successes(scope,finished_at) VALUES(?,?) ON CONFLICT(scope) DO UPDATE SET finished_at=max(finished_at,excluded.finished_at)",params![scope,at]).map_err(storage)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn write_turn(db: &Connection, owner: &str, turn: &str, outcome: &str, stage: &str, code: Option<&str>, now: i64) -> Result<(), ToolError> {
    let row: Option<(String,Option<String>,i64)> = db.query_row(
        "SELECT a.client_request_id,a.book_id,COALESCE(e.occurred_at,?) FROM run_admissions a LEFT JOIN admin_usage_events e ON e.user_id=a.owner_user_id AND e.event_ref=a.turn_id AND e.kind='question' WHERE a.owner_user_id=? AND a.turn_id=?",
        params![now,owner,turn],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
    let Some((key,book,accepted)) = row else { return Ok(()); };
    let is_probe = probe(db, owner, &key)?;
    db.execute("INSERT INTO operational_outcomes(user_id,request_id,turn_id,book_id,accepted_at,finished_at,outcome,stage,error_code,is_probe) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(user_id,request_id) DO UPDATE SET turn_id=excluded.turn_id,book_id=excluded.book_id,accepted_at=excluded.accepted_at,finished_at=CASE WHEN operational_outcomes.outcome=excluded.outcome AND operational_outcomes.error_code IS excluded.error_code THEN operational_outcomes.finished_at ELSE excluded.finished_at END,outcome=excluded.outcome,stage=excluded.stage,error_code=excluded.error_code,is_probe=excluded.is_probe",
        params![owner,key,turn,book,accepted,now,outcome,stage,code,is_probe]).map_err(storage)?;
    let scope=recovery_scope(code,stage,owner,book.as_deref().unwrap_or(""));
    db.execute("UPDATE operational_outcomes SET recovery_scope=? WHERE user_id=? AND request_id=?",params![scope,owner,key]).map_err(storage)?;
    Ok(())
}
pub(crate) fn rejection(db: &Connection, owner: &str, input: &Value, error: &ToolError, now: i64) -> Result<(), ToolError> {
    let Some(key) = input["client_request_id"].as_str().filter(|k|!k.is_empty() && k.len()<=128) else { return Ok(()); };
    // A repeated request or a polling response does not rewrite the original run's result.
    let admitted: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND client_request_id=?)",params![owner,key],|r|r.get(0)).map_err(storage)?;
    if admitted { return Ok(()); }
    let outcome = if account_block(&error.error_code) { "account_blocked" }
        else if matches!(error.category.as_str(),"validation"|"conflict"|"authentication"|"authorization"|"not_found"|"cancelled") { "rejected" }
        else { "failed" };
    db.execute("INSERT INTO operational_outcomes(user_id,request_id,accepted_at,finished_at,outcome,stage,error_code,is_probe,recovery_scope) VALUES(?,?,?,?,?,'admission',?,?,?) ON CONFLICT(user_id,request_id) DO NOTHING",
        params![owner,key,now,now,outcome,error.error_code,probe(db,owner,key)?,recovery_scope(Some(&error.error_code),stage(Some(&error.error_code)),owner,input["published_book_ref"]["book_id"].as_str().unwrap_or(""))]).map_err(storage)?;
    Ok(())
}

pub(crate) fn report(access: &Authorization, now: i64) -> Result<Value, ToolError> {
    let model = access.runs.operational_health(&access.spend, now);
    let (root, outcomes, recent, last_probe, signals) = {
        let store = access.admin.control.lock().unwrap();
        let db = &store.connection;
        let start = (now+28800).div_euclid(86400)*86400-28800;
        let mut grouped = db.prepare("SELECT outcome,is_probe,count(*),count(DISTINCT user_id) FROM operational_outcomes WHERE finished_at>=? AND finished_at<? GROUP BY outcome,is_probe").map_err(storage)?;
        let outcomes = grouped.query_map(params![start,start+86400],|r|Ok(json!({"outcome":r.get::<_,String>(0)?,"is_probe":r.get::<_,bool>(1)?,"count":r.get::<_,i64>(2)?,"users":r.get::<_,i64>(3)?}))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let mut query = db.prepare("SELECT o.user_id,u.email,o.request_id,o.turn_id,o.book_id,o.accepted_at,o.finished_at,o.outcome,o.stage,o.error_code,o.is_probe FROM operational_outcomes o JOIN users u USING(user_id) WHERE o.finished_at>=? AND o.outcome<>'delivered' ORDER BY o.finished_at DESC LIMIT 50").map_err(storage)?;
        let recent = query.query_map([now-86400],|r|Ok(json!({"user_id":r.get::<_,String>(0)?,"email":r.get::<_,Option<String>>(1)?,"request_id":r.get::<_,String>(2)?,"turn_id":r.get::<_,Option<String>>(3)?,"book_id":r.get::<_,Option<String>>(4)?,"accepted_at":r.get::<_,i64>(5)?,"finished_at":r.get::<_,i64>(6)?,"outcome":r.get::<_,String>(7)?,"stage":r.get::<_,String>(8)?,"error_code":r.get::<_,Option<String>>(9)?,"is_probe":r.get::<_,bool>(10)?}))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let last_probe = db.query_row("SELECT turn_id,finished_at,outcome,error_code FROM operational_outcomes WHERE is_probe=1 ORDER BY finished_at DESC LIMIT 1",[],|r|Ok(json!({"turn_id":r.get::<_,Option<String>>(0)?,"finished_at":r.get::<_,i64>(1)?,"outcome":r.get::<_,String>(2)?,"error_code":r.get::<_,Option<String>>(3)?}))).optional().map_err(storage)?;
        let mut failures = db.prepare("SELECT COALESCE(error_code,'DELIVERY_INCOMPLETE'),stage,count(*),count(DISTINCT user_id),max(finished_at),recovery_scope FROM operational_outcomes WHERE is_probe=0 AND finished_at>=? AND (outcome IN ('failed','unsaved') OR (outcome='partial' AND error_code IS NOT NULL)) GROUP BY error_code,stage,recovery_scope").map_err(storage)?;
        let grouped_failures = failures.query_map([now-900],|r|Ok(json!({"error_code":r.get::<_,String>(0)?,"stage":r.get::<_,String>(1)?,"count":r.get::<_,i64>(2)?,"users":r.get::<_,i64>(3)?,"last_at":r.get::<_,i64>(4)?,"recovery_scope":r.get::<_,String>(5)?}))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let mut q=db.prepare("SELECT scope,finished_at FROM operational_successes").map_err(storage)?;
        let successes=q.query_map([],|r|Ok((r.get::<_,String>(0)?,json!(r.get::<_,i64>(1)?)))).map_err(storage)?.collect::<Result<serde_json::Map<String,Value>,_>>().map_err(storage)?;
        let pending=db.query_row("SELECT count(*),coalesce(sum(reserved_micro_cny),0),min(created_at) FROM billable_usage WHERE state='pending' AND created_at<?",[now-86400],|r|Ok(json!({"count":r.get::<_,i64>(0)?,"reserved_micro_cny":r.get::<_,i64>(1)?,"oldest_at":r.get::<_,Option<i64>>(2)?}))).map_err(storage)?;
        let mut blocked = db.prepare("SELECT user_id,max(finished_at) FROM operational_outcomes WHERE outcome='account_blocked' AND is_probe=0 AND finished_at>=? GROUP BY user_id").map_err(storage)?;
        let owners = blocked.query_map([now-86400],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let mut accounts = Vec::new();
        for (owner, at) in owners {
            let current = crate::allowance_admin::current(db, &owner, now)?;
            let available = current["validity_state"] == "active" && (current["balance"]["unlimited"] == true || current["balance"]["available_micro_cny"].as_i64().is_some_and(|v|v>0));
            if !available { accounts.push(json!({"user_id":owner,"last_at":at})); }
        }
        let stuck: i64 = db.query_row("SELECT count(*) FROM run_admissions a JOIN admin_usage_events e ON e.user_id=a.owner_user_id AND e.event_ref=a.turn_id AND e.kind='question' WHERE a.dispatch_state IN ('queued','claimed') AND e.occurred_at<?",[now-1800],|r|r.get(0)).map_err(storage)?;
        (store.writer.root().to_path_buf(), outcomes, recent, last_probe, json!({"failures":grouped_failures,"successful_scopes":successes,"pending_charges":pending,"blocked_accounts":accounts,"stuck_requests":stuck}))
    };
    let watchdog = std::fs::read(root.join("operations-watchdog-status.json")).ok()
        .and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok());
    Ok(json!({"checked_at":now,"model":model,"web":crate::web_usage::health(&root,now),"outcomes_today":outcomes,"recent_outcomes":recent,
        "recent_limit":50,"recent_window_seconds":86400,"last_probe":last_probe,"signals":signals,"watchdog":watchdog}))
}

#[cfg(test)]
#[path="operations_tests.rs"]
mod tests;
