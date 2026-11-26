use super::{semantic_items, CompactionDraft, SourcedCheckpointItem};
use crate::{Message, Role, ToolCall};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct SourceWords<'a> {
    role: &'static str,
    message_index: usize,
    text: &'a str,
}

/// Rebuilt from the validated source window, including for previously installed checkpoints.
#[derive(Default)]
pub(super) struct NoteHandoff<'a> {
    pub(super) writes: Vec<Value>,
    sources: BTreeMap<&'a str, Vec<SourceWords<'a>>>,
    note_tasks: BTreeSet<&'a str>,
    material_sources: BTreeMap<&'a str, &'a str>,
}

impl<'a> NoteHandoff<'a> {
    pub(super) fn from_covered_history(
        raw: &'a [Message],
        by_id: &BTreeMap<&str, usize>,
        semantic: &'a CompactionDraft,
    ) -> Self {
        let mut notes = Self::default();
        let mut covered = BTreeMap::new();
        let note_sources: BTreeSet<_> = semantic_items(semantic)
            .into_iter()
            .filter(|(_, item)| mentions_note(&item.text))
            .flat_map(|(_, item)| item.source_item_ids.iter().map(String::as_str))
            .collect();
        for entry in &semantic.source_coverage {
            let index = by_id[entry.source_item_id.as_str()];
            covered.insert(index, entry.source_item_id.as_str());
            let message = &raw[index];
            if message.role == Role::User || note_sources.contains(entry.source_item_id.as_str()) {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    // Raw tool bodies are never replayed as historical evidence.
                    _ => continue,
                };
                let sentences: Vec<_> = sentences(message.content.as_deref().unwrap_or(""))
                    .map(str::trim)
                    .filter(|line| mentions_note(line) && line.chars().take(321).count() <= 320)
                    .map(|text| SourceWords {
                        role,
                        message_index: index,
                        text,
                    })
                    .collect();
                if message.role == Role::User
                    && sentences.iter().any(|words| note_operation(words.text))
                {
                    notes.note_tasks.insert(&entry.source_item_id);
                }
                if !sentences.is_empty() {
                    notes.sources.insert(&entry.source_item_id, sentences);
                }
            }
        }
        let mut calls = BTreeMap::new();
        for (index, message) in raw.iter().enumerate() {
            match message.role {
                // A provider may reuse a call ID in the next user turn.
                Role::User => calls.clear(),
                Role::Assistant if covered.contains_key(&index) => {
                    for call in &message.tool_calls {
                        if matches!(
                            call.name.as_str(),
                            "reader.note"
                                | "workspace.notes"
                                | "book.text"
                                | "book.text_batch"
                                | "music.read"
                                | "music.example"
                        ) {
                            if is_write_call(call) {
                                notes.note_tasks.insert(covered[&index]);
                            }
                            calls.insert(call.id.as_str(), call);
                        }
                    }
                }
                Role::Tool if covered.contains_key(&index) => {
                    let call = message
                        .tool_call_id
                        .as_deref()
                        .and_then(|id| calls.remove(id));
                    if call.is_some_and(is_write_call) {
                        notes.note_tasks.insert(covered[&index]);
                    }
                    if let Some(call) = call {
                        if matches!(call.name.as_str(), "reader.note" | "workspace.notes") {
                            if let Some(receipt) = write_receipt(call, message) {
                                notes.writes.push(receipt);
                            }
                        } else if note_sources.contains(covered[&index]) {
                            let body = message.content.as_deref().unwrap_or("");
                            if mentions_note(body) {
                                notes.material_sources.insert(covered[&index], body);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        notes
    }

    pub(super) fn active(&self) -> bool {
        !self.note_tasks.is_empty() || !self.writes.is_empty()
    }

    pub(super) fn retain_reader_instructions(&self, words: &mut Vec<SourceWords<'a>>) {
        for (id, source) in &self.sources {
            if !self.note_tasks.contains(id) {
                continue;
            }
            let instructions = source
                .iter()
                .copied()
                .filter(|s| s.role == "user" && note_operation(s.text))
                .take(3)
                .collect::<Vec<_>>();
            if instructions.len() <= 2 {
                add_source_words(words, instructions);
            }
        }
        // Instructions from later reader turns may supersede earlier ones.
        words.sort_by_key(|source| source.message_index);
    }

    pub(super) fn project_item(
        &self,
        item: &SourcedCheckpointItem,
        book_discussion: bool,
        words: &mut Vec<SourceWords<'a>>,
        active: &mut bool,
    ) -> Option<Value> {
        if !mentions_note(&item.text) {
            return Some(json!({"text":item.text, "evidence_refs":item.evidence_refs}));
        }
        let has_note_task = item
            .source_item_ids
            .iter()
            .any(|id| self.note_tasks.contains(id.as_str()));
        let mut text = String::new();
        let mut replaced = false;
        for segment in
            sentences(&item.text).flat_map(|sentence| sentence.split_inclusive([',', '，']))
        {
            let clause = segment
                .trim()
                .trim_end_matches(|ch: char| ch.is_ascii_punctuation() || "，。；！？".contains(ch))
                .trim();
            // A canonically cited fact from a score or lecture text is about
            // the work. Preserve its explanation and citations. A cite attached
            // to an unrelated goal or a Reader action grants no such status.
            let source_discussion = !has_note_task
                && !item.evidence_refs.is_empty()
                && item
                    .source_item_ids
                    .iter()
                    .filter_map(|id| self.material_sources.get(id.as_str()))
                    .any(|source| book_discussion || source.contains(clause));
            if !mentions_note(segment)
                || source_discussion
                || (!has_note_task && !note_operation(segment))
            {
                text.push_str(segment);
                continue;
            }
            replaced = true;
            *active = true;
            let candidates = item
                .source_item_ids
                .iter()
                .filter_map(|id| self.sources.get(id.as_str()))
                .flatten();
            // Preserve the entire originating sentence, including a preceding
            // condition or negation. An isolated substring can reverse its meaning.
            let mut exact = candidates
                .clone()
                .copied()
                .filter(|source| source.text.contains(clause))
                .peekable();
            if exact.peek().is_some() {
                add_source_words(words, exact);
            } else {
                // Paraphrases have no independently verified action status. In an
                // unambiguous source, the original discussion is still useful.
                // Never select unrelated lines from a whole attached document.
                let mut reader = candidates
                    .clone()
                    .copied()
                    .filter(|source| source.role == "user")
                    .take(3)
                    .collect::<Vec<_>>();
                if reader.is_empty() {
                    reader.extend(candidates.copied().take(3));
                }
                if reader.len() <= 2 {
                    add_source_words(words, reader);
                }
            }
        }
        if replaced {
            text.truncate(
                text.trim_end_matches([' ', '\n', ',', '，', ';', '；'])
                    .len(),
            );
        }
        (!text.is_empty() || !item.evidence_refs.is_empty())
            .then(|| json!({"text":text, "evidence_refs":item.evidence_refs}))
    }
}

fn is_write_call(call: &ToolCall) -> bool {
    call.name == "reader.note"
        || (call.name == "workspace.notes"
            && serde_json::from_str::<Value>(&call.arguments)
                .ok()
                .is_some_and(|args| matches!(args["action"].as_str(), Some("create" | "update"))))
}

fn add_source_words<'a>(
    words: &mut Vec<SourceWords<'a>>,
    candidates: impl IntoIterator<Item = SourceWords<'a>>,
) {
    // Each original sentence appears once per checkpoint. Keep the handoff
    // bounded even when many items cite the same long reader attachment.
    for source in candidates {
        if words.len() >= 8 {
            break;
        }
        if !words.contains(&source) {
            words.push(source);
        }
    }
}

fn write_receipt(call: &ToolCall, message: &Message) -> Option<Value> {
    let args: Value = serde_json::from_str(&call.arguments).ok()?;
    let result: Value = serde_json::from_str(message.content.as_deref()?).ok()?;
    let note_id = result["note_id"].as_str().filter(|id| !id.is_empty())?;
    if result.get("error_code").is_some() {
        return None;
    }
    match call.name.as_str() {
        "reader.note" if result["ok"] == true => {
            let status = match result["status"].as_str()? {
                "CREATED" => "created",
                "EXISTING" => "already_existed",
                _ => return None,
            };
            // A failed viewport selection does not roll back a persisted note.
            Some(
                json!({"tool":call.name,"outcome":status,"note_id":note_id,"lid":args["lid"].as_str()?,
                "text_preview":args["text"].as_str()?.chars().take(96).collect::<String>()}),
            )
        }
        "workspace.notes" => {
            let outcome = match args["action"].as_str()? {
                "create" => "created",
                "update" if args["note_id"] == note_id => "updated",
                _ => return None,
            };
            let revision = result["revision"].as_u64().filter(|rev| *rev > 0)?;
            Some(
                json!({"tool":call.name,"outcome":outcome,"note_id":note_id,"revision":revision,
                "title":result["title"].as_str()?.chars().take(96).collect::<String>()}),
            )
        }
        _ => None,
    }
}

fn sentences(text: &str) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let ends = text
        .char_indices()
        .filter_map(|(i, ch)| {
            let end = i + ch.len_utf8();
            let boundary = matches!(ch, '。' | '；' | ';' | '!' | '！' | '?' | '？' | '\n')
                || (ch == '.' && text[end..].starts_with(char::is_whitespace));
            boundary.then_some(end)
        })
        .chain(std::iter::once(text.len()));
    ends.map(move |end| {
        let sentence = &text[start..end];
        start = end;
        sentence
    })
    .filter(|sentence| !sentence.is_empty())
}

// Identify the product's note object, not a natural-language assertion of
// permission, tense, negation, success or failure.
fn mentions_note(text: &str) -> bool {
    if text.contains("笔记") {
        return true;
    }
    let mut tokens = text
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
        .peekable();
    while let Some(word) = tokens.next() {
        if (word.eq_ignore_ascii_case("note") || word.eq_ignore_ascii_case("notes"))
            && !tokens
                .peek()
                .is_some_and(|next| next.eq_ignore_ascii_case("that"))
        {
            return true;
        }
    }
    false
}

// A note in a personal write operation has a different referent from musical
// pitches and lecture materials. Only identify the operation's subject here;
// its authorization, tense and outcome are never inferred from wording.
fn note_operation(text: &str) -> bool {
    text.contains("reader.note")
        || text.contains("workspace.notes")
        || ["保存", "写", "记下", "创建", "更新"]
            .iter()
            .any(|op| text.contains(op))
        || text
            .split(|ch: char| !ch.is_ascii_alphabetic())
            .any(|word| {
                let word = word.to_ascii_lowercase();
                ["sav", "writ", "wrot", "creat", "updat", "record"]
                    .iter()
                    .any(|stem| word.starts_with(stem))
            })
}

#[cfg(test)]
mod tests {
    use super::super::{checkpoint_consumption_projection, SourceCoverage, SourceDisposition};
    use super::*;

    fn projection(messages: &[Message], covered: usize, summary: &str) -> Value {
        let ids: Vec<_> = (0..covered).map(|i| format!("item-{i}")).collect();
        let by_id = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let semantic = CompactionDraft {
            active_goal: vec![SourcedCheckpointItem {
                item_id: "task".into(),
                text: summary.into(),
                source_item_ids: ids.clone(),
                evidence_refs: vec![],
            }],
            source_coverage: ids
                .iter()
                .map(|id| SourceCoverage {
                    source_item_id: id.clone(),
                    disposition: SourceDisposition::Compacted,
                    target_item_ids: vec!["task".into()],
                    reason: None,
                })
                .collect(),
            ..Default::default()
        };
        let notes = NoteHandoff::from_covered_history(messages, &by_id, &semantic);
        checkpoint_consumption_projection(&semantic, &[], &notes)
    }

    fn call(name: &str, arguments: Value, result: Value) -> [Message; 2] {
        [
            Message {
                role: Role::Assistant,
                content: None,
                tool_calls: vec![ToolCall {
                    id: "reused".into(),
                    name: name.into(),
                    arguments: arguments.to_string(),
                }],
                tool_call_id: None,
                provider_continuation: None,
            },
            Message {
                role: Role::Tool,
                content: Some(result.to_string()),
                tool_calls: vec![],
                tool_call_id: Some("reused".into()),
                provider_continuation: None,
            },
        ]
    }

    #[test]
    fn note_handoff_keeps_conditions_denials_and_book_discussion_in_original_words() {
        let original = "如果笔记已保存，就先打开它，再按 η=0.01 计算。";
        let negative = "新笔记尚未写入，下一步才轮到这件事。";
        let raw = vec![Message::user(original), Message::user(negative)];
        for claim in [
            "笔记已经保存",
            "笔记写好了",
            "我们给你保存笔记成功了",
            "I have already written your note",
        ] {
            let answer = projection(&raw, 2, &format!("保留 η 和小数；{claim}"));
            assert_eq!(
                answer["semantic"]["active_goal"][0]["text"],
                "保留 η 和小数"
            );
            assert_eq!(
                answer["note_source_words"],
                json!([
                {"role":"user","message_index":0,"text":original},
                {"role":"user","message_index":1,"text":negative}])
            );
            assert_eq!(answer["verified_note_writes"], json!([]));
        }
        let article = "文章的实验笔记对照了两种测量方法。";
        let raw = vec![
            Message::user("Compare the experiments."),
            Message {
                role: Role::Assistant,
                ..Message::user(article)
            },
        ];
        let discussion = projection(
            &raw,
            2,
            "研究笔记讨论两组测量。The author notes that both groups used η.",
        );
        assert_eq!(
            discussion["semantic"]["active_goal"][0]["text"],
            "研究笔记讨论两组测量。The author notes that both groups used η."
        );
        for explanation in [
            "The notes C and E form a major third.",
            "The lecture notes contrast two step sizes.",
        ] {
            assert_eq!(
                projection(&raw, 2, explanation)["semantic"]["active_goal"][0]["text"],
                explanation
            );
        }
    }

    #[test]
    fn note_handoff_binds_each_confirmation_to_the_tool_result_for_that_note() {
        let mut raw = vec![Message::user("保存旧方案的笔记。")];
        let args = json!({"lid":"1.1","text":"旧方案"});
        raw.extend(call(
            "reader.note",
            args.clone(),
            json!({"note_id":"note-A","ok":true,"status":"CREATED",
            "reader_effect":{"status":"not_applied"}}),
        ));
        raw.extend(call(
            "reader.note",
            args,
            json!({"note_id":"note-A","ok":true,"status":"EXISTING"}),
        ));
        raw.push(Message::user("新方案先讨论。"));
        raw.extend(call(
            "reader.note",
            json!({"lid":"1.2","text":"新方案"}),
            json!({"error_code":"INVALID_RANGE"}),
        ));
        raw.extend(call(
            "workspace.notes",
            json!({"action":"read","note_id":"other"}),
            json!({"note_id":"other","revision":3,"title":"已存在"}),
        ));
        raw.extend(call(
            "workspace.notes",
            json!({"action":"update","note_id":"note-B"}),
            json!({"note_id":"note-B","revision":2,"title":"复习"}),
        ));
        let cutoff = raw.len();
        raw.extend(call(
            "reader.note",
            json!({"lid":"1.2","text":"新方案"}),
            json!({"ok":true,"note_id":"later","status":"CREATED"}),
        ));
        let p = projection(&raw, cutoff, "新方案笔记写入成功");
        assert_eq!(
            p["verified_note_writes"],
            json!([
            {"tool":"reader.note","outcome":"created","note_id":"note-A","lid":"1.1","text_preview":"旧方案"},
            {"tool":"reader.note","outcome":"already_existed","note_id":"note-A","lid":"1.1","text_preview":"旧方案"},
            {"tool":"workspace.notes","outcome":"updated","note_id":"note-B","revision":2,"title":"复习"}])
        );
        assert!(!p.to_string().contains("新方案笔记写入成功"));
        assert!(!p.to_string().contains("later"));
    }

    #[test]
    fn note_handoff_does_not_import_an_unrelated_attached_notebook() {
        let attachment = (0..250)
            .map(|i| format!("笔记摘录 {i} 讲不同的实验。"))
            .collect::<String>();
        let p = projection(&[Message::user(&attachment)], 1, "笔记保存成功");
        assert_eq!(p["note_source_words"], json!([]));
        assert_eq!(p["verified_note_writes"], json!([]));
    }

    #[test]
    fn note_handoff_preserves_cited_score_and_lecture_note_facts_without_write_overhead() {
        use super::super::EvidenceRef;
        for (tool, source, summary) in [
            (
                "music.read",
                "The following notes form the harmony: C and E.",
                "The composer wrote the notes C and E a major third apart.",
            ),
            (
                "book.text",
                "These lecture notes were prepared for the class in June.",
                "The author wrote the lecture notes for a June class.",
            ),
        ] {
            let mut raw = vec![Message::user("Explain this passage.")];
            raw.extend(call(tool, json!({}), json!({"text":source})));
            let by_id = BTreeMap::from([("reader", 0), ("read", 1), ("source", 2)]);
            let evidence = EvidenceRef::LidRange {
                start_lid: "1.1".into(),
                end_lid: "1.1".into(),
            };
            let mut semantic = CompactionDraft::default();
            semantic.critical_facts.push(SourcedCheckpointItem {
                item_id: "fact".into(),
                text: summary.into(),
                source_item_ids: vec!["source".into()],
                evidence_refs: vec![evidence.clone()],
            });
            semantic.source_coverage = by_id
                .keys()
                .map(|id| SourceCoverage {
                    source_item_id: id.to_string(),
                    disposition: SourceDisposition::Compacted,
                    target_item_ids: vec!["fact".into()],
                    reason: None,
                })
                .collect();
            let notes = NoteHandoff::from_covered_history(&raw, &by_id, &semantic);
            let p = checkpoint_consumption_projection(&semantic, &[], &notes);
            assert_eq!(
                p["semantic"]["critical_facts"][0],
                json!({"text":summary,"evidence_refs":[evidence]})
            );
            assert!(p.get("verified_note_writes").is_none());
        }
    }
}
