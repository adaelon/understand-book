use crate::{agent_method_not_allowed, AppState, Reply, Req};
use memory::learning::TutorMutation;
use serde_json::json;

pub(crate) fn route(state: &mut AppState, req: Req) -> Reply {
    let result = (|| {
        let mut learning = state.user.learning_store()?;
        if req.url == "/tutor/state" && req.method == "GET" {
            return learning.state();
        }
        let request: TutorMutation =
            serde_json::from_str(req.body).map_err(|error| read_tools::ToolError {
                error_code: "TUTOR_ACTION_INVALID".into(),
                category: "validation".into(),
                message: error.to_string(),
            })?;
        learning.mutate(&request, req.now)
    })();
    match result {
        Ok(state) => Reply {
            status: 200,
            body: serde_json::to_string(&state).unwrap(),
        },
        Err(error) => Reply {
            status: if error.category == "conflict" {
                409
            } else if error.category == "validation" {
                400
            } else {
                503
            },
            body: json!(error).to_string(),
        },
    }
}

pub(crate) fn dispatch(state: &mut AppState, req: Req) -> Reply {
    if !matches!(req.url, "/tutor/state" | "/tutor/mutate" | "/tutor/readiness") {
        return crate::teaching::route(&state.private_context(), req);
    }
    if req.url == "/tutor/readiness" {
        if req.method != "GET" { return agent_method_not_allowed(); }
        return readiness(state);
    }
    if (req.url == "/tutor/state" && req.method != "GET")
        || (req.url == "/tutor/mutate" && req.method != "POST")
    {
        return agent_method_not_allowed();
    }
    route(state, req)
}

fn readiness(state: &AppState) -> Reply {
    source_readiness(&state.workspace.book, &state.workspace.book_dir)
}

pub(crate) fn source_readiness(book: &read_tools::Book, book_dir: &std::path::Path) -> Reply {
    let source_id = &book.base.book_id;
    let preparing = |reason: &str, status: &str| Reply { status: 200, body: json!({
        "status": status, "source_id": source_id, "teaching_map_revision": null,
        "limitations": [], "reason": reason,
    }).to_string() };
    let receipt_path = book_dir.join("teaching_readiness.json");
    if !receipt_path.exists() { return preparing("正式教学素材尚未发布", "preparing"); }
    let result = (|| -> Result<serde_json::Value, String> {
        let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(&receipt_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if receipt["source_id"].as_str() != Some(source_id.as_str())
            || receipt["source_revision"].as_str() != Some(book.source_fingerprint()) {
            return Err("来源已更新，教学素材需要重新发布".into());
        }
        if receipt["version"] != "teaching_readiness.v1" || receipt["status"] != "ready"
            || ["source", "structure", "objects", "cognitive_materials"].iter().any(|key| receipt["coverage"][key] != "complete")
            || receipt["coverage"]["source_review"] != "passed" {
            return Err("教学素材覆盖或来源审阅尚未完成".into());
        }
        let revision = receipt["teaching_map_revision"].as_str().filter(|r| !r.is_empty()).ok_or("教学版本缺失")?;
        let expected_path = format!("teaching/versions/{revision}/map.json");
        if revision.contains(['/', '\\']) || receipt["map_path"].as_str() != Some(expected_path.as_str()) { return Err("教学版本引用无效".into()); }
        let map: serde_json::Value = serde_json::from_slice(&std::fs::read(book_dir.join(expected_path)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if map["source_revision"] != receipt["source_revision"] || map["source_id"] != receipt["source_id"] {
            return Err("教学版本与来源不一致".into());
        }
        Ok(json!({ "status": "ready", "source_id": source_id, "teaching_map_revision": revision,
            "limitations": receipt["limitations"], "reason": "教学材料已就绪，可以开始学习" }))
    })();
    match result {
        Ok(view) => Reply { status: 200, body: view.to_string() },
        Err(reason) => preparing(&reason, "stale"),
    }
}
