//! Public failure messages are selected by the host. Raw diagnostics stay in
//! the durable turn and observability records, including for historical turns.
use crate::{AgentTurnError, ToolError};
use serde_json::Value;

fn message(code: &str, category: &str) -> String {
    if let Some(stop) = runtime::model_spend::SpendStop::from_code(code) {
        return stop.tool_error().message;
    }
    match code {
        "AGENT_RUN_CANCELLED" => "本次回答已停止。",
        "INTERRUPTED" | "AGENT_RUN_NOT_FOUND" => "这次回答已中断，请重新提交。",
        "TURN_UNSAVED" => "结果保存失败，请重试保存。",
        _ if category == "storage" => "结果保存失败，请重试保存。",
        _ if matches!(category, "provider" | "protocol" | "capacity") => "服务暂时不可用，请稍后重试。",
        _ => "这次回答未能完成，请重试。",
    }.into()
}

pub(crate) fn turn(error: &AgentTurnError) -> AgentTurnError {
    AgentTurnError { error_code: error.error_code.clone(), category: error.category.clone(),
        message: message(&error.error_code, &error.category) }
}

pub(crate) fn tool(error: &ToolError) -> ToolError {
    ToolError { error_code: error.error_code.clone(), category: error.category.clone(),
        message: message(&error.error_code, &error.category) }
}

pub(crate) fn value(mut error: Value) -> Value {
    if let Some(fields) = error.as_object_mut() {
        let code = fields.get("error_code").and_then(Value::as_str).unwrap_or_default();
        let category = fields.get("category").and_then(Value::as_str).unwrap_or_default();
        let public = message(code, category);
        fields.insert("message".into(), Value::String(public));
        if let Some(cause) = fields.get_mut("execution_error").filter(|cause| cause.is_object()) {
            *cause = value(cause.take());
        }
    }
    error
}

pub(crate) fn view(mut view: Value) -> Value {
    for field in ["error", "board_delivery_error"] {
        if let Some(error) = view.get_mut(field).filter(|error| error.is_object()) {
            *error = value(error.take());
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_and_legacy_errors_keep_diagnostics_only_in_the_original() {
        for code in ["TOOL_ARGUMENTS_INVALID", "FUTURE_INTERNAL_ERROR"] {
            let original = AgentTurnError { error_code: code.into(), category: "validation".into(),
                message: "$.move.delivery_text is required; private diagnostic".into() };
            assert_eq!(turn(&original).message, "这次回答未能完成，请重试。");
            assert!(original.message.contains("$.move"));
            let unsaved = value(json!({"error_code":"TURN_UNSAVED","category":"storage",
                "message":"private path", "execution_error":original}));
            assert_eq!(unsaved["message"], "结果保存失败，请重试保存。");
            assert_eq!(unsaved["execution_error"]["message"], "这次回答未能完成，请重试。");
        }
    }

    #[test]
    fn history_projection_keeps_both_terminal_diagnostics_in_storage() {
        for code in ["TOOL_ARGUMENTS_INVALID", "FUTURE_INTERNAL_ERROR"] {
            let raw = json!({"error_code":code, "category":"validation", "message":"$.move is required"});
            let stored: crate::AgentChatTurn = serde_json::from_value(json!({"user":"Explain matrix norms", "status":"failed",
                "error":raw, "board_delivery_error":raw})).unwrap();
            let public = crate::scoped_turn_view(None, &stored);
            assert_eq!(public.error.unwrap().message, "这次回答未能完成，请重试。");
            assert_eq!(public.board_delivery_error.unwrap().message, "这次回答未能完成，请重试。");
            let independent_history = crate::authorization::turn_json(&stored);
            assert_eq!(independent_history["error"]["message"], "这次回答未能完成，请重试。");
            assert_eq!(independent_history["board_delivery_error"]["message"], "这次回答未能完成，请重试。");
            assert_eq!(stored.error.unwrap().message, "$.move is required");
            assert_eq!(stored.board_delivery_error.unwrap().message, "$.move is required");
        }
    }

    #[test]
    fn live_finish_and_restored_snapshot_project_the_same_failure() {
        use crate::agent_stream::{RunDescriptor, RunStream};
        let descriptor = RunDescriptor { book_id: "book".into(), session_id: "chat".into(), turn_id: "turn".into() };
        let stream = RunStream::new(descriptor);
        let raw = json!({"error_code":"TOOL_ARGUMENTS_INVALID", "category":"validation", "message":"$.move.delivery_text is required"});
        stream.finish(Some(json!({"status":"failed","error":raw})), None);
        let live = stream.snapshot();
        assert_eq!(live.final_view.as_ref().unwrap()["error"]["message"], "这次回答未能完成，请重试。");
        let mut historical = live;
        historical.final_view.as_mut().unwrap()["error"] = raw.clone();
        historical.error = Some(raw);
        let restored = RunStream::from_snapshot(historical).snapshot();
        assert_eq!(restored.error.unwrap()["message"], "这次回答未能完成，请重试。");
        assert_eq!(restored.final_view.unwrap()["error"]["message"], "这次回答未能完成，请重试。");
    }
}
