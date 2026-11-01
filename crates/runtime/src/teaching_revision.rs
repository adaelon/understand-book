use read_tools::ToolError;
use serde_json::{json, Value};
use std::ops::Range;

const EDITABLE_FIELDS: [(&str, &str); 2] = [
    ("delivery_text", "/move/delivery_text"),
    ("public_prompt", "/move/response_expectation/public_prompt"),
];

/// The latest schema-validated, unaccepted ordinary text selection in this run.
/// The caller validates the complete selection before retaining it and clears it
/// when the selection is accepted or its teaching design no longer applies.
#[derive(Clone, Debug, Default)]
pub(crate) struct TeachingRevisionDraft {
    version: u64,
    current: Option<RetainedDraft>,
}

#[derive(Clone, Debug)]
struct RetainedDraft {
    reference: String,
    select_request: Value,
}

struct ParagraphEdit<'a> {
    path: &'static str,
    range: Range<usize>,
    new_text: &'a str,
}

impl TeachingRevisionDraft {
    pub(crate) fn retain(&mut self, request: &Value) -> Option<&str> {
        let movement = &request["move"];
        if request["operation"] != "select"
            || !movement["delivery_text"].is_string()
            || ["presentation", "board_candidate_revision", "board"]
                .into_iter()
                .any(|field| !movement[field].is_null())
            || request["delivery_scope"]["kind"] == "board_part"
        {
            self.clear();
            return None;
        }
        if !self
            .current
            .as_ref()
            .is_some_and(|draft| draft.select_request == *request)
        {
            self.version += 1;
            self.current = Some(RetainedDraft {
                reference: format!("teaching-draft:{}", self.version),
                select_request: request.clone(),
            });
        }
        self.reference()
    }

    /// Prepare a complete select request without changing the retained draft.
    /// The caller validates and retains the result before its normal review.
    pub(crate) fn prepare_revision(&self, request: &Value) -> Result<Value, ToolError> {
        if request["operation"] != "revise" {
            return Err(invalid("文本修订须使用 operation=revise。"));
        }
        let current = self
            .current
            .as_ref()
            .ok_or_else(|| invalid("当前没有可修订的普通文本草稿；请提交完整 select。"))?;
        if request["draft_ref"].as_str() != Some(current.reference.as_str()) {
            return Err(invalid(
                "draft_ref 已失效或不匹配；请使用当前草稿的准确引用。",
            ));
        }
        let edits = request["edits"]
            .as_array()
            .ok_or_else(|| invalid("文本修订须提供 edits 数组；仅补读证据后重审可使用空数组。"))?;
        let mut replacements: Vec<ParagraphEdit<'_>> = Vec::with_capacity(edits.len());
        for (index, edit) in edits.iter().enumerate() {
            let paragraph = edit["paragraph"].as_u64();
            let Some(&(field, path)) = EDITABLE_FIELDS
                .iter()
                .find(|(field, _)| edit["field"].as_str() == Some(*field))
            else {
                return Err(edit_error(
                    index,
                    "unsupported",
                    paragraph,
                    0,
                    "修订仅支持 delivery_text 或既有的 public_prompt。",
                ));
            };
            let ranges = current
                .select_request
                .pointer(path)
                .and_then(Value::as_str)
                .map(paragraph_ranges)
                .unwrap_or_default();
            let available = ranges.len();
            let range = paragraph
                .and_then(|number| number.checked_sub(1))
                .and_then(|number| usize::try_from(number).ok())
                .and_then(|number| ranges.get(number))
                .cloned()
                .ok_or_else(|| {
                    edit_error(index, field, paragraph, available,
                    "段落编号不存在；请使用 editable_paragraphs 给出的编号，不能通过修订创建邀请。")
                })?;
            if replacements
                .iter()
                .any(|replacement| replacement.path == path && replacement.range == range)
            {
                return Err(edit_error(
                    index,
                    field,
                    paragraph,
                    available,
                    "同一段落不能在一批修订中重复指定。",
                ));
            }
            let new_text = edit["new_text"].as_str().ok_or_else(|| {
                edit_error(
                    index,
                    field,
                    paragraph,
                    available,
                    "new_text 须为字符串；删除本段正文时可使用空字符串。",
                )
            })?;
            replacements.push(ParagraphEdit {
                path,
                range,
                new_text,
            });
        }
        // All locations refer to the retained original. Reverse byte order keeps
        // later targets stable when a replacement adds or removes paragraphs.
        replacements.sort_unstable_by(|left, right| {
            left.path
                .cmp(right.path)
                .then_with(|| right.range.start.cmp(&left.range.start))
        });
        let changes = request.get("move_changes").map(|changes| changes.as_object()
            .ok_or_else(|| invalid("move_changes 须为字段修改对象。"))).transpose()?;
        if let Some(changes) = changes {
            for (field, value) in changes {
                if !matches!(field.as_str(), "participation" | "response_expectation" | "coach_adjustment") {
                    return Err(invalid(&format!("move_changes 不支持修改 {field}；来源或设计变化须提交完整 select。")));
                }
                if field == "participation" && value.is_null() {
                    return Err(invalid("participation 不能删除；请明确新的参与方式。"));
                }
            }
            if changes.contains_key("response_expectation") && replacements.iter().any(|edit| edit.path == "/move/response_expectation/public_prompt") {
                return Err(invalid("整项修改 response_expectation 时不要同时编辑 public_prompt 段落。"));
            }
        }
        let mut candidate = current.select_request.clone();
        for replacement in replacements {
            let Value::String(text) = candidate
                .pointer_mut(replacement.path)
                .expect("validated paragraph field")
            else {
                unreachable!("validated text field")
            };
            text.replace_range(replacement.range, replacement.new_text);
        }
        if let Some(changes) = changes {
            let movement = candidate["move"].as_object_mut().expect("retained select move");
            for (field, value) in changes {
                if value.is_null() { movement.remove(field); }
                else { movement.insert(field.clone(), value.clone()); }
            }
        }
        Ok(candidate)
    }

    pub(crate) fn clear(&mut self) {
        self.current = None;
    }

    pub(crate) fn fragment_value(&self) -> Option<Value> {
        self.current.as_ref().map(|draft| {
            let mut preserved = draft.select_request.clone();
            let movement = preserved["move"]
                .as_object_mut()
                .expect("retained text selection");
            movement.remove("delivery_text");
            if let Some(expectation) = movement
                .get_mut("response_expectation")
                .and_then(Value::as_object_mut)
            {
                expectation.remove("public_prompt");
            }
            let mut editable_paragraphs = Vec::new();
            for (field, path) in EDITABLE_FIELDS {
                if let Some(text) = draft.select_request.pointer(path).and_then(Value::as_str) {
                    for (index, range) in paragraph_ranges(text).into_iter().enumerate() {
                        editable_paragraphs
                            .push(json!({"field":field,"paragraph":index + 1,"text":&text[range]}));
                    }
                }
            }
            json!({"draft_ref":draft.reference,"preserved_select_fields":preserved,
                "editable_paragraphs":editable_paragraphs})
        })
    }

    pub(crate) fn reference(&self) -> Option<&str> {
        self.current.as_ref().map(|draft| draft.reference.as_str())
    }
}

fn paragraph_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for paragraph in text.split("\n\n") {
        let end = start + paragraph.len();
        if !paragraph.trim().is_empty() {
            ranges.push(start..end);
        }
        start = end + 2;
    }
    ranges
}

fn edit_error(
    index: usize,
    field: &str,
    paragraph: Option<u64>,
    available: usize,
    reason: &str,
) -> ToolError {
    let paragraph = paragraph
        .map(|number| number.to_string())
        .unwrap_or_else(|| "invalid".into());
    invalid(&format!(
        "第 {} 项修订无效（field={}，paragraph={}，当前可用段落数={}）：{}",
        index + 1,
        field,
        paragraph,
        available,
        reason
    ))
}

fn invalid(message: &str) -> ToolError {
    ToolError {
        error_code: "TEACHING_REVISION_INVALID".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection() -> Value {
        json!({
            "operation":"select", "finish":true, "delivery_move_refs":["earlier:1"],
            "move":{
                "move_id":"move:1", "design_ref":"design:1", "block_refs":["relation:1"],
                "object_ids":[], "capability":"explain", "kind":"explain", "prompt":"",
                "interaction_intent":"direct_explanation", "intent_origin":"current_request",
                "intent_quote":"请解释", "delivery_text":"第一段保留。\n\n第二段有错误。 [[source:one]]\n示例 A。\n\n  末段保留。  ",
                "target":{"learning_focus":"连接", "source_bindings":[{"start_lid":"1.1","end_lid":"1.2"}]},
                "responds_to":{"fact_refs":["expression:1"]},
                "participation":{"choice":"support","reason":"解释当前问题","not_yet_inferred":"理解程度"},
                "assessment":{"rule":{"kind":"open","items":[]},"private_text":"保持原样\n  包含空白"},
                "response_expectation":{"public_prompt":"请说明条件为什么必要？","block_refs":["relation:1"]}
            }
        })
    }

    fn revision(reference: &str, edits: Value) -> Value {
        json!({"operation":"revise","draft_ref":reference,"edits":edits})
    }

    #[test]
    fn allocation_revision_preserves_body_and_removes_only_the_named_invitation() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let mut request = revision(&reference, json!([]));
        request["move_changes"] = json!({"participation":{"choice":"conclude","reason":"已完成直接讲解"},
            "response_expectation":null});
        let mut expected = original.clone();
        expected["move"]["participation"] = request["move_changes"]["participation"].clone();
        expected["move"].as_object_mut().unwrap().remove("response_expectation");
        assert_eq!(draft.prepare_revision(&request).unwrap(), expected);
        assert_eq!(draft.prepare_revision(&revision(&reference, json!([]))).unwrap(), original,
            "preparation never changes the retained draft before full validation");
    }

    #[test]
    fn allocation_changes_reject_conflicting_invitation_edits_without_mutating_draft() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let mut request = revision(&reference, json!([{ "field":"public_prompt", "paragraph":1, "new_text":"Another question?" }]));
        request["move_changes"] = json!({"response_expectation":null});
        assert!(draft.prepare_revision(&request).unwrap_err().message.contains("同时"));
        request = revision(&reference, json!([]));
        request["move_changes"] = json!({"participation":null});
        assert!(draft.prepare_revision(&request).is_err());
        request["move_changes"] = json!({"target":{}});
        assert!(draft.prepare_revision(&request).is_err());
        assert_eq!(draft.prepare_revision(&revision(&reference, json!([]))).unwrap(), original);
    }

    #[test]
    fn paragraph_replacement_preserves_every_other_field_and_surrounding_byte() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let patched = draft.prepare_revision(&revision(&reference, json!([
            {"field":"delivery_text","paragraph":2,"new_text":"第二段已修正。 [[source:one]]\n示例 A。"}
        ]))).unwrap();
        let mut expected = original.clone();
        expected["move"]["delivery_text"] =
            json!("第一段保留。\n\n第二段已修正。 [[source:one]]\n示例 A。\n\n  末段保留。  ");
        assert_eq!(patched, expected);
        assert_eq!(
            draft
                .prepare_revision(&revision(&reference, json!([])))
                .unwrap(),
            original
        );
        assert_eq!(draft.reference(), Some(reference.as_str()));
        assert_eq!(draft.retain(&patched), Some("teaching-draft:2"));
        assert_eq!(draft.retain(&patched), Some("teaching-draft:2"));
    }

    #[test]
    fn all_targets_use_original_positions_when_replacements_split_or_delete_paragraphs() {
        let mut original = selection();
        original["move"]["delivery_text"] = json!("甲。\n\n乙。\n\n丙。\n\n丁。");
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let edits = json!([
            {"field":"delivery_text","paragraph":3,"new_text":""},
            {"field":"delivery_text","paragraph":1,"new_text":"新甲一。\n\n新甲二。"},
            {"field":"public_prompt","paragraph":1,"new_text":"新邀请？"},
            {"field":"delivery_text","paragraph":4,"new_text":"丁已改。"}
        ]);
        let prepared = draft
            .prepare_revision(&revision(&reference, edits.clone()))
            .unwrap();
        assert_eq!(
            prepared["move"]["delivery_text"],
            "新甲一。\n\n新甲二。\n\n乙。\n\n\n\n丁已改。"
        );
        assert_eq!(
            prepared["move"]["response_expectation"]["public_prompt"],
            "新邀请？"
        );
        assert_eq!(
            draft
                .prepare_revision(&revision(&reference, json!([])))
                .unwrap(),
            original
        );
        assert_eq!(draft.reference(), Some(reference.as_str()));
    }

    #[test]
    fn fragment_numbers_paragraphs_and_preserves_metadata_without_repeating_text() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        draft.retain(&original);
        let fragment = draft.fragment_value().unwrap();
        let mut metadata = original.clone();
        metadata["move"]
            .as_object_mut()
            .unwrap()
            .remove("delivery_text");
        metadata["move"]["response_expectation"]
            .as_object_mut()
            .unwrap()
            .remove("public_prompt");
        assert_eq!(
            fragment,
            json!({
                "draft_ref":"teaching-draft:1", "preserved_select_fields":metadata,
                "editable_paragraphs":[
                    {"field":"delivery_text","paragraph":1,"text":"第一段保留。"},
                    {"field":"delivery_text","paragraph":2,"text":"第二段有错误。 [[source:one]]\n示例 A。"},
                    {"field":"delivery_text","paragraph":3,"text":"  末段保留。  "},
                    {"field":"public_prompt","paragraph":1,"text":"请说明条件为什么必要？"}
                ]
            })
        );
    }

    #[test]
    fn empty_paragraphs_are_not_numbered_and_their_bytes_are_preserved() {
        let mut original = selection();
        original["move"]["delivery_text"] = json!("\n\n首段。\n\n  \n\n末段。\n\n");
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let paragraphs = draft.fragment_value().unwrap()["editable_paragraphs"].clone();
        assert_eq!(
            paragraphs[0],
            json!({"field":"delivery_text","paragraph":1,"text":"首段。"})
        );
        assert_eq!(
            paragraphs[1],
            json!({"field":"delivery_text","paragraph":2,"text":"末段。"})
        );
        assert_eq!(paragraphs[2]["field"], "public_prompt");
        let patched = draft
            .prepare_revision(&revision(
                &reference,
                json!([
                    {"field":"delivery_text","paragraph":2,"new_text":"更正。"}
                ]),
            ))
            .unwrap();
        assert_eq!(
            patched["move"]["delivery_text"],
            "\n\n首段。\n\n  \n\n更正。\n\n"
        );
    }

    #[test]
    fn stale_and_cleared_references_cannot_edit_a_later_draft() {
        let mut draft = TeachingRevisionDraft::default();
        let original = selection();
        assert_eq!(draft.retain(&original), Some("teaching-draft:1"));
        assert_eq!(draft.retain(&original), Some("teaching-draft:1"));
        let mut changed = original.clone();
        changed["move"]["delivery_text"] = json!("另一稿。");
        assert_eq!(draft.retain(&changed), Some("teaching-draft:2"));
        assert!(draft
            .prepare_revision(&revision("teaching-draft:1", json!([])))
            .is_err());
        draft.clear();
        assert!(draft.reference().is_none());
        assert!(draft.fragment_value().is_none());
        assert!(draft
            .prepare_revision(&revision("teaching-draft:2", json!([])))
            .is_err());
        assert_eq!(draft.retain(&original), Some("teaching-draft:3"));
        assert!(draft
            .prepare_revision(&revision("teaching-draft:2", json!([])))
            .is_err());
    }

    #[test]
    fn empty_edits_resubmit_the_exact_draft_after_evidence_reads() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let prepared = draft
            .prepare_revision(&revision(&reference, json!([])))
            .unwrap();
        assert_eq!(prepared, original);
        assert_eq!(draft.retain(&prepared), Some(reference.as_str()));
    }

    #[test]
    fn public_prompt_only_changes_an_existing_invitation_field() {
        let mut original = selection();
        original["move"]["response_expectation"]["public_prompt"] = json!("问题一？\n\n问题二？");
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        let edits = json!([{"field":"public_prompt","paragraph":2,"new_text":"修订的问题二？"}]);
        let patched = draft
            .prepare_revision(&revision(&reference, edits.clone()))
            .unwrap();
        let mut expected = original.clone();
        expected["move"]["response_expectation"]["public_prompt"] =
            json!("问题一？\n\n修订的问题二？");
        assert_eq!(patched, expected);
        for expectation in [Value::Null, json!({"block_refs":["relation:1"]})] {
            let mut no_prompt = original.clone();
            no_prompt["move"]["response_expectation"] = expectation;
            let reference = draft.retain(&no_prompt).unwrap().to_owned();
            let error = draft
                .prepare_revision(&revision(&reference, edits.clone()))
                .unwrap_err();
            assert!(error.message.contains("第 1 项"));
            assert!(error.message.contains("field=public_prompt"));
            assert!(error.message.contains("paragraph=2"));
            assert!(error.message.contains("当前可用段落数=0"));
            assert_eq!(
                draft
                    .prepare_revision(&revision(&reference, json!([])))
                    .unwrap(),
                no_prompt
            );
        }
    }

    #[test]
    fn duplicate_and_invalid_targets_identify_the_failing_edit_without_mutating_the_draft() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        let reference = draft.retain(&original).unwrap().to_owned();
        for edit in [
            json!({"field":"delivery_text","paragraph":1,"new_text":"不得回显"}),
            json!({"field":"delivery_text","paragraph":4,"new_text":"不得回显"}),
            json!({"field":"delivery_text","paragraph":0,"new_text":"不得回显"}),
        ] {
            let error = draft.prepare_revision(&revision(&reference, json!([
                {"field":"delivery_text","paragraph":1,"new_text":"第一项候选"}, edit.clone()
            ]))).unwrap_err();
            assert!(error.message.contains("第 2 项"));
            assert!(error.message.contains("field=delivery_text"));
            assert!(error
                .message
                .contains(&format!("paragraph={}", edit["paragraph"])));
            assert!(error.message.contains("当前可用段落数=3"));
            assert!(!error.message.contains("不得回显"));
            assert!(!error.message.contains("第一段保留"));
            assert_eq!(
                draft
                    .prepare_revision(&revision(&reference, json!([])))
                    .unwrap(),
                original
            );
        }
    }

    #[test]
    fn unsupported_candidates_clear_the_draft_but_null_optional_fields_do_not() {
        let original = selection();
        let mut draft = TeachingRevisionDraft::default();
        for field in ["presentation", "board_candidate_revision", "board"] {
            draft.retain(&original);
            let mut unsupported = original.clone();
            unsupported["move"][field] = json!({"value":"present"});
            assert!(draft.retain(&unsupported).is_none());
            assert!(draft.reference().is_none());
            unsupported["move"][field] = Value::Null;
            assert!(draft.retain(&unsupported).is_some());
        }
        for unsupported in [
            json!({"operation":"outside"}),
            json!({"operation":"select","move":{}}),
            {
                let mut part = original.clone();
                part["delivery_scope"] =
                    json!({"kind":"board_part","relation":"part","remaining_request":"rest"});
                part
            },
        ] {
            draft.retain(&original);
            assert!(draft.retain(&unsupported).is_none());
            assert!(draft.fragment_value().is_none());
        }
        let mut whole = original;
        whole["delivery_scope"] = json!({"kind":"whole"});
        assert!(draft.retain(&whole).is_some());
    }
}
