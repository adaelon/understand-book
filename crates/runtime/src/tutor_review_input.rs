//! A lossless, page-local projection for the independent teaching reviewer.
//!
//! Preview reports repeat the same whole-page text after each action and in
//! each viewport. Keep the first exact text and point later copies to it; the
//! original packet remains available to the caller for evidence validation.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const MIN_REUSED_TEXT_CHARS: usize = 128;

const EVIDENCE_READING_GUIDE: &str = "author_claims 是作者说明，不是页面实际效果。按 author_preview_observations 中的环境与 observations 操作顺序阅读实际证据，结合 step、action、scroll、viewport、layout、scene 和截图说明确认位置及状态。dom.text 与 reading.text 是对应状态或 selector 的实际文字，不代表整段同时出现在屏幕；dom.visible_text 才描述该视口的可见文字。dom.semantic_text 仅作 DOM 语义证据，不作为逐字交付引文。same_text_as 是本页内直接指向完全相同原文的 JSON Pointer，读取该处原文即可，证据性质仍由当前字段决定；复用文字不复用视口、操作或可见范围，也不说明学习者看过这些内容。";

/// Reduce repeated preview text without summarizing it or discarding states.
/// References are confined to one saved presentation (including its revision).
pub fn project(mut packet: Value) -> Value {
    let Some(pages) = packet
        .get_mut("proposed_presentations")
        .and_then(Value::as_array_mut)
    else {
        return packet;
    };

    for (page_index, page) in pages.iter_mut().enumerate() {
        let Some(page) = page.as_object_mut() else {
            continue;
        };
        let mut claims = Map::new();
        for field in ["readable_content", "initial_state", "state_contract"] {
            if let Some(value) = page.remove(field) {
                claims.insert(field.to_owned(), value);
            }
        }
        if !claims.is_empty() {
            page.insert("author_claims".to_owned(), Value::Object(claims));
        }

        let Some(previews) = page
            .get_mut("author_preview_observations")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        if previews.is_empty() {
            continue;
        }

        // Direct string comparison is sufficient for the small number of
        // distinct page states. Never reuse author claims or another page.
        let mut seen_text = BTreeMap::<String, String>::new();
        for (preview_index, preview) in previews.iter_mut().enumerate() {
            let prefix = format!(
                "#/proposed_presentations/{page_index}/author_preview_observations/{preview_index}"
            );
            if let Some(observations) = preview
                .get_mut("observations")
                .and_then(Value::as_array_mut)
            {
                for (observation_index, observation) in observations.iter_mut().enumerate() {
                    if let Some(dom) = observation.get_mut("dom").and_then(Value::as_object_mut) {
                        for field in ["text", "semantic_text", "visible_text"] {
                            if let Some(text) = dom.get_mut(field) {
                                reuse_exact_text(
                                    text,
                                    format!(
                                        "{prefix}/observations/{observation_index}/dom/{field}"
                                    ),
                                    &mut seen_text,
                                );
                            }
                        }
                    }
                }
            }
            if let Some(text) = preview
                .get_mut("reading")
                .and_then(|reading| reading.get_mut("text"))
            {
                reuse_exact_text(text, format!("{prefix}/reading/text"), &mut seen_text);
            }
        }
        page.insert(
            "evidence_reading_guide".to_owned(),
            json!(EVIDENCE_READING_GUIDE),
        );
    }
    packet
}

fn reuse_exact_text(value: &mut Value, path: String, seen: &mut BTreeMap<String, String>) {
    let Some(text) = value.as_str() else {
        return;
    };
    // Short labels and changing numeric values remain inline. A reference must
    // replace a meaningfully larger copy, not make a small report harder to read.
    if text.chars().take(MIN_REUSED_TEXT_CHARS).count() < MIN_REUSED_TEXT_CHARS {
        return;
    }
    if let Some(first_path) = seen.get(text) {
        *value = json!({"same_text_as": first_path});
    } else {
        seen.insert(text.to_owned(), path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn restore_text(value: &Value, packet: &Value) -> Value {
        match value {
            Value::Object(object) if object.len() == 1 && object.contains_key("same_text_as") => {
                let pointer = object["same_text_as"]
                    .as_str()
                    .unwrap()
                    .strip_prefix('#')
                    .unwrap();
                let original = packet
                    .pointer(pointer)
                    .expect("every reference must resolve");
                assert!(
                    original.is_string(),
                    "references must point directly to text"
                );
                original.clone()
            }
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), restore_text(value, packet)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| restore_text(value, packet))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    #[test]
    fn review_projection_reconstructs_all_text_and_preserves_observed_states() {
        let text = "公开全文，包括下面需要滚动才能看到的推导。".repeat(10);
        let changed_text = format!("{text}当前数值已从 60 秒变成 30 秒。");
        let semantic = format!("{text}折叠区的语义内容。");
        let previews = json!([
            {"candidate_id":"candidate-1", "environment_name":"desktop",
                "environment":{"width":960,"height":720,"input":"mouse"},
                "observations":[
                    {"step":0,"action":null,"scroll":{"x":0,"y":0,"max_y":2400},
                        "viewport":{"width":960,"height":720},"scene":{"id":"start"},
                        "layout":{"elements":[{"x":12,"y":730,"width":400,"height":100}]},
                        "dom":{"text":text,"semantic_text":semantic,"visible_text":"开头", "controls":[{"value":1}]}},
                    {"step":1,"action":{"type":"scroll","y":720},"scroll":{"x":0,"y":720,"max_y":2400},
                        "dom":{"text":text,"semantic_text":semantic,"visible_text":"推导", "controls":[{"value":1}]}},
                    {"step":2,"action":{"type":"input","value":3},"scroll":{"x":0,"y":720,"max_y":2400},
                        "dom":{"text":changed_text,"semantic_text":semantic,"visible_text":"30 秒", "controls":[{"value":3}]}}
                ],"reading":{"selector":"main","action_step":2,"text":changed_text,"page_state":{"s":3}}},
            {"environment_name":"mobile","environment":{"width":390,"height":844,"input":"touch"},
                "observations":[{"step":0,"action":null,"scroll":{"x":0,"y":0,"max_y":4800},
                    "dom":{"text":text,"semantic_text":semantic,"visible_text":"移动端开头"}}],
                "reading":{"selector":"#details","text":"选择器只读到这里"}}
        ]);
        let original = json!({
            "original_request":"请保留完整推导", "frozen_input":{"quote":"我不懂这里"},
            "learner_grounding":{"expressions":[{"text":"原话不能换成摘要"}]},
            "current_source_evidence":[{"content":"来源原文"}],
            "current_coach":{"intent":"解释移位相减"},
            "proposed_presentations":[{"reference":{"presentation_id":"p","revision":2},
                "author_preview_observations":previews}]
        });
        let projected = project(original.clone());
        let page = &projected["proposed_presentations"][0];
        assert_eq!(
            restore_text(&page["author_preview_observations"], &projected),
            previews
        );
        assert!(
            page["author_preview_observations"][0]["observations"][0]["dom"]["text"].is_string()
        );
        assert!(
            page["author_preview_observations"][0]["observations"][1]["dom"]["text"]
                ["same_text_as"]
                .is_string()
        );
        assert_eq!(
            page["author_preview_observations"][0]["observations"][2]["dom"]["text"],
            changed_text
        );
        for key in [
            "original_request",
            "frozen_input",
            "learner_grounding",
            "current_source_evidence",
            "current_coach",
        ] {
            assert_eq!(projected[key], original[key]);
        }
        assert_eq!(
            original["proposed_presentations"][0]["author_preview_observations"],
            previews
        );
    }

    #[test]
    fn review_projection_does_not_reuse_text_across_page_revisions() {
        let text = "同一句话也不能让两个版本的观察混在一起。".repeat(10);
        let original = json!({"proposed_presentations":[
            {"reference":{"presentation_id":"p","revision":1},"author_preview_observations":[
                {"observations":[{"dom":{"text":text}},{"dom":{"text":text}}]}]},
            {"reference":{"presentation_id":"p","revision":2},"author_preview_observations":[
                {"observations":[{"dom":{"text":text}},{"dom":{"text":text}}]}]}
        ]});
        let projected = project(original.clone());
        for index in 0..2 {
            let previews =
                &projected["proposed_presentations"][index]["author_preview_observations"];
            assert_eq!(previews[0]["observations"][0]["dom"]["text"], text);
            assert_eq!(previews[0]["observations"][1]["dom"]["text"]["same_text_as"],
                format!("#/proposed_presentations/{index}/author_preview_observations/0/observations/0/dom/text"));
            assert_eq!(
                restore_text(previews, &projected),
                original["proposed_presentations"][index]["author_preview_observations"]
            );
        }
    }

    #[test]
    fn review_projection_separates_author_claims_without_using_them_as_evidence() {
        let text = "作者声称有效不能代替实际观察。".repeat(12);
        let original = json!({"proposed_presentations":[{
            "reference":{"presentation_id":"p","revision":1},
            "readable_content":text,"initial_state":{"s":3},"state_contract":{"claim":"可操作"},
            "author_preview_observations":[{"reading":{"selector":"main","text":text}}]
        }]});
        let projected = project(original.clone());
        let page = &projected["proposed_presentations"][0];
        for field in ["readable_content", "initial_state", "state_contract"] {
            assert!(page.get(field).is_none());
            assert_eq!(
                page["author_claims"][field],
                original["proposed_presentations"][0][field]
            );
        }
        assert_eq!(
            page["author_preview_observations"][0]["reading"]["text"],
            text
        );
        assert_eq!(project(projected.clone()), projected);
    }

    #[test]
    fn review_projection_leaves_packets_without_pages_unchanged() {
        for packet in [
            json!({"original_request":"只回答文字","proposed_delivery":{"text":"回答"}}),
            json!({"proposed_presentations":[],"learner_grounding":{"expressions":["原话"]}}),
            json!({"proposed_presentations":null}),
        ] {
            assert_eq!(project(packet.clone()), packet);
        }
    }
}
