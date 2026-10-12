//! Content-free differences between final provider requests within one run and purpose.
//! Message indices describe JSON structure, not the provider's tokenized KV prefix.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageChangeField {
    Added,
    Removed,
    Role,
    Content,
    Images,
    ReasoningContent,
    ToolArguments,
    ToolCallIds,
    ToolNames,
    ToolCalls,
    ToolCallId,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RequestMessageRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MessageChange {
    pub index: u32,
    pub role: RequestMessageRole,
    pub fields: Vec<MessageChangeField>,
    /// Compact JSON bytes of this message, not provider tokens.
    pub previous_bytes: u32,
    pub current_bytes: u32,
    pub unchanged_prefix_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestDiagnostics {
    pub request_index: u32,
    pub step_id: u32,
    pub previous_step_id: Option<u32>,
    pub message_count: u32,
    pub previous_message_count: Option<u32>,
    /// Number of identical leading messages; null for the first request of this purpose.
    pub unchanged_prefix_messages: Option<u32>,
    pub first_changed_message: Option<MessageChange>,
    /// True only if all previous messages remain identical and messages were appended.
    pub messages_append_only: Option<bool>,
    pub tool_count: u32,
    pub previous_tool_count: Option<u32>,
    pub tools_changed: Option<bool>,
    pub first_changed_tool: Option<u32>,
    /// Any top-level setting except messages and tools (including model, thinking, tool_choice).
    pub settings_changed: Option<bool>,
    pub image_count: u32,
    pub previous_image_count: Option<u32>,
    pub reasoning_message_count: u32,
    pub previous_reasoning_message_count: Option<u32>,
}

#[derive(Default)]
pub(crate) struct RequestTracker {
    next: u32,
    previous: HashMap<String, (u32, Arc<Value>)>,
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn images(message: &Value) -> Vec<&Value> {
    array(message, "content")
        .iter()
        .filter(|part| matches!(part["type"].as_str(), Some("image_url" | "input_image")))
        .collect()
}

fn image_count(messages: &[Value]) -> u32 {
    messages
        .iter()
        .map(|message| images(message).len() as u32)
        .sum()
}

fn reasoning_count(messages: &[Value]) -> u32 {
    messages
        .iter()
        .filter(|message| {
            message
                .get("reasoning_content")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.is_empty())
        })
        .count() as u32
}

fn other_fields_changed(before: &Value, after: &Value, excluded: &[&str]) -> bool {
    [before, after].iter().any(|value| {
        value.as_object().is_some_and(|object| {
            object
                .keys()
                .any(|key| !excluded.contains(&key.as_str()) && before.get(key) != after.get(key))
        })
    })
}

fn changed_message(index: usize, before: Option<&Value>, after: Option<&Value>) -> MessageChange {
    use MessageChangeField as Field;
    let message = after.or(before).expect("a changed position exists");
    let role = match message["role"].as_str() {
        Some("system") => RequestMessageRole::System,
        Some("developer") => RequestMessageRole::Developer,
        Some("user") => RequestMessageRole::User,
        Some("assistant") => RequestMessageRole::Assistant,
        Some("tool") => RequestMessageRole::Tool,
        _ => RequestMessageRole::Other,
    };
    let fields = match (before, after) {
        (None, _) => vec![Field::Added],
        (_, None) => vec![Field::Removed],
        (Some(before), Some(after)) => {
            let mut fields = Vec::new();
            for (key, field) in [
                ("role", Field::Role),
                ("content", Field::Content),
                ("reasoning_content", Field::ReasoningContent),
                ("tool_call_id", Field::ToolCallId),
            ] {
                if before.get(key) != after.get(key) {
                    fields.push(field);
                }
            }
            if images(before) != images(after) {
                fields.push(Field::Images);
            }
            if before.get("tool_calls") != after.get("tool_calls") {
                let old_calls = array(before, "tool_calls");
                let new_calls = array(after, "tool_calls");
                let start = fields.len();
                for (pointer, field) in [
                    ("/function/arguments", Field::ToolArguments),
                    ("/id", Field::ToolCallIds),
                    ("/function/name", Field::ToolNames),
                ] {
                    if old_calls
                        .iter()
                        .map(|v| v.pointer(pointer))
                        .ne(new_calls.iter().map(|v| v.pointer(pointer)))
                    {
                        fields.push(field);
                    }
                }
                if fields.len() == start || old_calls.len() != new_calls.len() {
                    fields.push(Field::ToolCalls);
                }
            }
            if other_fields_changed(
                before,
                after,
                &[
                    "role",
                    "content",
                    "reasoning_content",
                    "tool_call_id",
                    "tool_calls",
                ],
            ) {
                fields.push(Field::Other);
            }
            fields
        }
    };
    let old_bytes = before
        .map(|value| serde_json::to_vec(value).unwrap())
        .unwrap_or_default();
    let new_bytes = after
        .map(|value| serde_json::to_vec(value).unwrap())
        .unwrap_or_default();
    MessageChange {
        index: index as u32,
        role,
        fields,
        previous_bytes: old_bytes.len() as u32,
        current_bytes: new_bytes.len() as u32,
        unchanged_prefix_bytes: old_bytes
            .iter()
            .zip(&new_bytes)
            .take_while(|(a, b)| a == b)
            .count() as u32,
    }
}

impl RequestTracker {
    pub(crate) fn observe(
        &mut self,
        purpose: &str,
        step_id: u32,
        body: Arc<Value>,
    ) -> RequestDiagnostics {
        self.next += 1;
        let messages = array(&body, "messages");
        let tools = array(&body, "tools");
        let mut result = RequestDiagnostics {
            request_index: self.next,
            step_id,
            previous_step_id: None,
            message_count: messages.len() as u32,
            previous_message_count: None,
            unchanged_prefix_messages: None,
            first_changed_message: None,
            messages_append_only: None,
            tool_count: tools.len() as u32,
            previous_tool_count: None,
            tools_changed: None,
            first_changed_tool: None,
            settings_changed: None,
            image_count: image_count(messages),
            previous_image_count: None,
            reasoning_message_count: reasoning_count(messages),
            previous_reasoning_message_count: None,
        };
        if let Some((previous_step_id, previous)) = self.previous.get(purpose) {
            let old_messages = array(previous, "messages");
            let prefix = old_messages
                .iter()
                .zip(messages)
                .take_while(|(a, b)| a == b)
                .count();
            result.previous_step_id = Some(*previous_step_id);
            result.previous_message_count = Some(old_messages.len() as u32);
            result.unchanged_prefix_messages = Some(prefix as u32);
            result.messages_append_only =
                Some(prefix == old_messages.len() && messages.len() > old_messages.len());
            if prefix < old_messages.len().max(messages.len()) {
                result.first_changed_message = Some(changed_message(
                    prefix,
                    old_messages.get(prefix),
                    messages.get(prefix),
                ));
            }
            let old_tools = array(previous, "tools");
            result.previous_tool_count = Some(old_tools.len() as u32);
            result.tools_changed = Some(previous.get("tools") != body.get("tools"));
            let tool_prefix = old_tools
                .iter()
                .zip(tools)
                .take_while(|(a, b)| a == b)
                .count();
            if tool_prefix < old_tools.len().max(tools.len()) {
                result.first_changed_tool = Some(tool_prefix as u32);
            }
            result.settings_changed = Some(other_fields_changed(
                previous,
                &body,
                &["messages", "tools"],
            ));
            result.previous_image_count = Some(image_count(old_messages));
            result.previous_reasoning_message_count = Some(reasoning_count(old_messages));
        }
        self.previous.insert(purpose.to_owned(), (step_id, body));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body() -> Value {
        json!({"model":"fixture", "messages":[
            {"role":"system","content":"private instructions"},
            {"role":"assistant","content":"private answer","reasoning_content":"private reasoning",
                "tool_calls":[{"id":"call1","type":"function","function":{"name":"read","arguments":"private arguments"}}]},
            {"role":"tool","tool_call_id":"call1","content":[{"type":"image_url","image_url":{"url":"private image"}}]}
        ],"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}]})
    }

    #[test]
    fn tracks_append_rewrite_truncation_and_separates_compaction_baseline() {
        let mut tracker = RequestTracker::default();
        let first = body();
        assert_eq!(
            tracker
                .observe("outer", 1, Arc::new(first.clone()))
                .previous_step_id,
            None
        );
        tracker.observe("compaction", 2, Arc::new(json!({"messages":[]})));
        let mut appended = first.clone();
        appended["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"user","content":"next"}));
        let diag = tracker.observe("outer", 3, Arc::new(appended.clone()));
        assert_eq!(diag.request_index, 3);
        assert_eq!(diag.previous_step_id, Some(1));
        assert_eq!(diag.unchanged_prefix_messages, Some(3));
        assert_eq!(diag.messages_append_only, Some(true));
        assert_eq!(
            diag.first_changed_message.unwrap().fields,
            vec![MessageChangeField::Added]
        );
        appended["messages"][0]["content"] = json!("replaced instructions");
        let diag = tracker.observe("outer", 4, Arc::new(appended.clone()));
        assert_eq!(diag.unchanged_prefix_messages, Some(0));
        assert_eq!(diag.messages_append_only, Some(false));
        assert_eq!(
            diag.first_changed_message.unwrap().fields,
            vec![MessageChangeField::Content]
        );
        appended["messages"].as_array_mut().unwrap().truncate(1);
        let diag = tracker.observe("outer", 5, Arc::new(appended.clone()));
        assert_eq!(
            diag.first_changed_message.unwrap().fields,
            vec![MessageChangeField::Removed]
        );
        let unchanged = tracker.observe("outer", 6, Arc::new(appended));
        assert_eq!(unchanged.first_changed_message, None);
        assert_eq!(unchanged.tools_changed, Some(false));
        assert_eq!(unchanged.settings_changed, Some(false));
    }

    #[test]
    fn distinguishes_reasoning_tool_arguments_images_tools_and_settings_without_content() {
        for (pointer, replacement, expected) in [
            (
                "/messages/1/reasoning_content",
                json!("changed"),
                Some(MessageChangeField::ReasoningContent),
            ),
            (
                "/messages/1/tool_calls/0/function/arguments",
                json!("changed"),
                Some(MessageChangeField::ToolArguments),
            ),
            (
                "/messages/2/content/0/image_url/url",
                json!("changed"),
                Some(MessageChangeField::Images),
            ),
            ("/tools/0/function/parameters/type", json!("changed"), None),
            ("/model", json!("changed"), None),
        ] {
            let mut tracker = RequestTracker::default();
            let mut next = body();
            tracker.observe("outer", 1, Arc::new(next.clone()));
            *next.pointer_mut(pointer).unwrap() = replacement;
            let diag = tracker.observe("outer", 2, Arc::new(next));
            if let Some(expected) = expected {
                assert!(diag
                    .first_changed_message
                    .as_ref()
                    .unwrap()
                    .fields
                    .contains(&expected));
            } else if pointer.starts_with("/tools") {
                assert_eq!(diag.tools_changed, Some(true));
                assert_eq!(diag.first_changed_tool, Some(0));
            } else {
                assert_eq!(diag.settings_changed, Some(true));
            }
            let metadata = serde_json::to_string(&diag).unwrap();
            assert!(!metadata.contains("private"));
            assert!(!metadata.contains("\"changed\""));
        }
    }

    #[test]
    fn counts_removed_images_and_reasoning_and_resets_between_runs() {
        let mut tracker = RequestTracker::default();
        let mut next = body();
        tracker.observe("outer", 1, Arc::new(next.clone()));
        next["messages"][1]
            .as_object_mut()
            .unwrap()
            .remove("reasoning_content");
        next["messages"][2]["content"] = json!("receipt");
        let diag = tracker.observe("outer", 2, Arc::new(next.clone()));
        assert_eq!((diag.previous_image_count, diag.image_count), (Some(1), 0));
        assert_eq!(
            (
                diag.previous_reasoning_message_count,
                diag.reasoning_message_count
            ),
            (Some(1), 0)
        );
        assert!(diag
            .first_changed_message
            .unwrap()
            .fields
            .contains(&MessageChangeField::ReasoningContent));
        assert_eq!(
            RequestTracker::default()
                .observe("outer", 1, Arc::new(next))
                .previous_step_id,
            None
        );
    }
}
