use runtime::{
    run_context::CancellationToken,
    run_events::{ActivityStatus, RunEvents},
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Child,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};

pub(super) struct PermissionContext<'a> {
    pub profile: &'a Path,
    pub turn_id: &'a str,
    pub events: &'a RunEvents,
}

struct Pending {
    profile: PathBuf,
    turn_id: String,
    request_id: String,
    card: Value,
    answer: Option<mpsc::Sender<Option<String>>>,
}

static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn permission_response(profile: &Path, input: &Value) -> Result<Value, String> {
    let turn_id = input["turn_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("缺少回合标识")?;
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    let active = pending
        .iter_mut()
        .find(|p| p.profile == profile && p.turn_id == turn_id && p.answer.is_some());
    match input["action"].as_str() {
        Some("get") => Ok(json!({"permission":active.map(|p| &p.card)})),
        Some("respond") => {
            let p = active
                .filter(|p| Some(p.request_id.as_str()) == input["request_id"].as_str())
                .ok_or("该授权请求已结束")?;
            let choice = match input.get("option_id") {
                Some(Value::Null) => None,
                Some(Value::String(option))
                    if p.card["options"].as_array().is_some_and(|options| {
                        options
                            .iter()
                            .any(|o| o["optionId"].as_str() == Some(option))
                    }) =>
                {
                    Some(option.clone())
                }
                _ => return Err("请选择 Agent 提供的选项".into()),
            };
            p.answer
                .take()
                .ok_or("该授权请求已结束")?
                .send(choice)
                .map_err(|_| "该授权请求已结束")?;
            Ok(json!({"ok":true}))
        }
        _ => Err("未知授权操作".into()),
    }
}

pub(super) fn choose(
    context: &PermissionContext<'_>,
    params: &Value,
    cancel: &CancellationToken,
    child: &mut Child,
    timeout: Duration,
) -> Value {
    let tool = &params["toolCall"];
    let options: Vec<_> = params["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|o| {
            o["optionId"].as_str().is_some_and(|id| !id.is_empty())
                && o["name"].is_string()
                && o["kind"].is_string()
        })
        .collect();
    if options.is_empty() || cancel.is_cancelled() {
        return json!({"outcome":"cancelled"});
    }
    let details = ["rawInput", "locations"]
        .into_iter()
        .filter_map(|key| {
            tool.get(key).filter(|value| !value.is_null()).map(|value| {
                format!(
                    "{key}: {}",
                    serde_json::to_string_pretty(value).unwrap_or_default()
                )
            })
        })
        .collect::<Vec<_>>()
        .join("\n");
    let request_id = NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string();
    let card = json!({"turn_id":context.turn_id,"request_id":request_id,"title":tool["title"].as_str().unwrap_or("Agent 工具"),
        "kind":tool["kind"].as_str().unwrap_or("other"),"details":details,"options":options});
    let (answer, receiver) = mpsc::channel();
    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Pending {
            profile: context.profile.to_owned(),
            turn_id: context.turn_id.to_owned(),
            request_id: request_id.clone(),
            card,
            answer: Some(answer),
        });
    // The request must be discoverable before publishing the activity to subscribers.
    let event = context
        .events
        .begin("tool", "acp.permission", "等待本机 Agent 工具授权", true);
    let deadline = Instant::now() + timeout;
    let choice = loop {
        if cancel.is_cancelled()
            || Instant::now() >= deadline
            || matches!(child.try_wait(), Ok(Some(_)))
        {
            break None;
        }
        match receiver.recv_timeout(Duration::from_millis(75)) {
            Ok(answer) => break answer,
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => break None,
        }
    };
    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|p| p.request_id != request_id);
    let (status, outcome) = match choice.filter(|_| !cancel.is_cancelled()) {
        Some(id) => {
            let rejected = options.iter().any(|o| {
                o["optionId"] == id
                    && matches!(o["kind"].as_str(), Some("reject_once" | "reject_always"))
            });
            (
                if rejected {
                    ActivityStatus::Rejected
                } else {
                    ActivityStatus::Succeeded
                },
                json!({"outcome":"selected","optionId":id}),
            )
        }
        None => (ActivityStatus::Cancelled, json!({"outcome":"cancelled"})),
    };
    context.events.finish(event, status, None, None, None);
    outcome
}
