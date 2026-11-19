//! Shared knowledge navigation with a private, evidence-derived overlay.
use crate::*;
use read_tools::knowledge::{Index, KnowledgeNode, KnowledgeReference, KnowledgeSource};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: &str) -> ToolError {
    memory::teaching::invalid(message)
}
fn node_view(index: &Index, node: &KnowledgeNode) -> Value {
    let mut value = json!(node);
    value["child_count"] = json!(node.children.len());
    value["preview"] = json!(node
        .children
        .iter()
        .filter_map(|id| index.nodes.get(id))
        .take(2)
        .map(|n| &n.label)
        .collect::<Vec<_>>());
    value.as_object_mut().unwrap().remove("children");
    value
}

fn current_locations(index: &Index, input: &Value) -> Vec<String> {
    let mut current = if let Some(lid) = input["current_lid"].as_str() {
        index
            .path(&format!("text:{lid}"))
            .iter()
            .map(|n| n.id.clone())
            .collect::<Vec<_>>()
    } else if let Some(ms) = input["position_ms"].as_f64() {
        index
            .nodes
            .values()
            .filter(|n| {
                n.sources.iter().any(|s| match s {
                    KnowledgeSource::Music { target } => {
                        target.audio.start_ms <= ms && ms < target.audio.end_ms
                    }
                    KnowledgeSource::Media {
                        start_ms: Some(start),
                        end_ms,
                        ..
                    } => *start <= ms && ms < end_ms.unwrap_or(*start + 1000.),
                    _ => false,
                })
            })
            .flat_map(|n| {
                index
                    .path(&n.id)
                    .iter()
                    .map(|n| n.id.clone())
                    .collect::<Vec<_>>()
            })
            .collect()
    } else {
        vec![]
    };
    current.sort();
    current.dedup();
    current.sort_by_key(|id| {
        (
            index.path(id).len(),
            index.nodes.get(id).is_some_and(|n| {
                matches!(
                    n.reference.artifact.as_str(),
                    "music_event" | "video_discourse" | "pass1"
                )
            }),
        )
    });
    current
}

fn summary_placements(
    index: &Index,
    item: &memory::understanding::UnderstandingSummary,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for link in &item.knowledge_links {
        for node in index.placements(&link.reference) {
            ids.insert(node.id.clone());
        }
    }
    if ids.is_empty() {
        if let Some(target) = &item.target {
            for object in &target.object_refs {
                for node in index.object_placements(&object.object_id, object.object_revision) {
                    ids.insert(node.id.clone());
                }
            }
            for music in target.music_bindings.iter().flatten() {
                for id in &music.target.event_ids {
                    let reference = KnowledgeReference {
                        source_id: item.source_id.clone(),
                        source_revision: item.source_revision.clone(),
                        publication_id: Some(music.target.publication.publication_id.clone()),
                        artifact: "music_event".into(),
                        entry_id: id.clone(),
                    };
                    for node in index.placements(&reference) {
                        ids.insert(node.id.clone());
                    }
                }
            }
            if ids.is_empty() {
                for range in &target.source_bindings {
                    let a = index.path(&format!("text:{}", range.start_lid));
                    let b = index.path(&format!("text:{}", range.end_lid));
                    if let Some(common) = a
                        .iter()
                        .zip(&b)
                        .take_while(|(a, b)| a.id == b.id)
                        .last()
                        .map(|(a, _)| a.id.clone())
                    {
                        ids.insert(common);
                    }
                }
            }
            // Media and score reads without a finer placement stay at material scope.
            if ids.is_empty()
                && (target
                    .media_bindings
                    .as_ref()
                    .is_some_and(|s| !s.is_empty())
                    || target.music_bindings.as_ref().is_some_and(|s| !s.is_empty()))
            {
                ids.insert("root".into());
            }
        }
    }
    ids.into_iter().collect()
}

pub(crate) fn view(
    user: &user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
) -> Result<Value, ToolError> {
    let index = workspace.book.knowledge_index(
        &workspace.book_dir,
        workspace.publication.as_ref().map(|p| &p.reference),
    )?;
    let current = current_locations(index, input);
    let current_scope = current
        .iter()
        .rev()
        .find(|id| {
            index.nodes.get(*id).is_some_and(|n| {
                matches!(
                    n.kind.as_str(),
                    "chapter" | "section" | "music_section" | "video_unit"
                )
            })
        })
        .or_else(|| current.last());
    let selected = if input["reference"].is_object() {
        let reference: KnowledgeReference = serde_json::from_value(input["reference"].clone())
            .map_err(|_| invalid("知识位置不可读"))?;
        Some(
            index
                .placements(&reference)
                .first()
                .ok_or_else(|| invalid("这条理解来自另一版材料，请返回原讨论查看"))?
                .id
                .clone(),
        )
    } else {
        None
    };
    let id = selected
        .as_deref()
        .or(input["node_id"].as_str())
        .or(current_scope.map(String::as_str))
        .unwrap_or("root");
    let id = if index.nodes.contains_key(id) {
        id
    } else {
        id.strip_prefix("text:")
            .and_then(|lid| index.scope_for_lid(lid))
            .unwrap_or(id)
    };
    let node = index
        .nodes
        .get(id)
        .ok_or_else(|| invalid("这处内容已更新，请重新打开知识树"))?;
    let after = input["after"].as_u64().unwrap_or(0) as usize;
    let store = user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?;
    let summaries = store.understanding_summaries(
        &workspace.book.base.book_id,
        workspace.book.source_fingerprint(),
    )?;
    let mut counts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut current_understanding: BTreeMap<String, Vec<memory::understanding::UnderstandingSummary>> = BTreeMap::new();
    let mut fruits = vec![];
    for item in summaries {
        let placements = summary_placements(index, &item);
        if placements
            .iter()
            .any(|id| index.path(id).iter().any(|parent| parent.id == node.id))
        {
            fruits.push(item.clone());
        }
        for placement in placements {
            for parent in index.path(&placement) {
                if item.later_evidence_ref.is_none() && item.correction.is_none() {
                    let entries=current_understanding.entry(parent.id.clone()).or_default();
                    if entries.len()<3 && !entries.iter().any(|entry|entry.evidence_ref==item.evidence_ref) { entries.push(item.clone()); }
                }
                counts
                    .entry(parent.id.clone())
                    .or_default()
                    .insert(item.evidence_ref.clone());
            }
        }
    }
    let decorate = |n: &KnowledgeNode| {
        let mut v = node_view(index, n);
        v["fruit_count"] = json!(counts.get(&n.id).map_or(0, BTreeSet::len));
        if input["overview"].as_bool()==Some(true) {
            v["understanding"] = json!(current_understanding.get(&n.id).into_iter().flatten().take(3).collect::<Vec<_>>());
        }
        v
    };
    let query = input["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let matches: Vec<_> = if query.is_empty() {
        node.children
            .iter()
            .filter_map(|id| index.nodes.get(id))
            .collect()
    } else {
        index
            .nodes
            .values()
            .filter(|n| {
                n.id != "root"
                    && (n.label.to_lowercase().contains(&query)
                        || n.summary.to_lowercase().contains(&query))
            })
            .collect()
    };
    let page: Vec<_> = matches
        .iter()
        .skip(after)
        .take(12)
        .map(|n| decorate(n))
        .collect();
    let introduction = index
        .introduction(id)
        .map(|n| json!({"text":n.summary,"is_opening":n.id != id}));
    let fruit_after = input["fruit_after"].as_u64().unwrap_or(0) as usize;
    let fruit_total = fruits.len();
    let fruits: Vec<_> = fruits.into_iter().skip(fruit_after).take(12).collect();
    let map_groups:Vec<_> = if input["overview"].as_bool()==Some(true) {
        matches.iter().skip(after).take(12).map(|group| {
            let mut pending=std::collections::VecDeque::from(group.children.clone());
            let mut nodes=Vec::new();
            if pending.is_empty() { nodes.push(decorate(group)); }
            while let Some(id)=pending.pop_front() {
                let Some(child)=index.nodes.get(&id) else {continue;};
                if child.children.is_empty() || !matches!(child.kind.as_str(),"chapter"|"section"|"navigation"|"concepts_group"|"claims_group"|"discourse_group"|"themes") {
                    nodes.push(decorate(child));
                    if nodes.len()==4 {break;}
                } else {pending.extend(child.children.clone());}
            }
            json!({"topic":decorate(group),"nodes":nodes})
        }).collect()
    } else {vec![]};
    Ok(
        json!({"node":decorate(node),"children":page,"map_groups":map_groups,"breadcrumbs":index.path(id).iter().map(|n|json!({"id":n.id,"label":n.label})).collect::<Vec<_>>(),
        "next":(after+12<matches.len()).then_some(after+12),"total":matches.len(),"fruits":fruits,"next_fruit":(fruit_after+12<fruit_total).then_some(fruit_after+12),"current_ids":current,"modality":index.modality,"current_scope":current_scope,"entry_point":index.entry_point(id).map(decorate),"introduction":introduction,
        "updates":store.understanding_jobs(&workspace.book.base.book_id,workspace.book.source_fingerprint())?}),
    )
}

pub(crate) fn retry(
    user: &user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    let mut store =
        user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?;
    let id = input["job_ref"]
        .as_str()
        .ok_or_else(|| invalid("缺少理解整理任务"))?;
    let job = store
        .coach_job(id)?
        .ok_or_else(|| invalid("理解整理任务不可用"))?;
    if job.input.binding.source_id != workspace.book.base.book_id
        || job.input.binding.source_revision != workspace.book.source_fingerprint()
    {
        return Err(invalid("请在原材料中重试理解整理"));
    }
    let result = store.retry_coach_job(id, now)?;
    Ok(json!({"status":result.status}))
}

/// Nominate bounded candidates from actual reads. Proximity does not accept a link.
pub(crate) fn candidates(
    book: &Book,
    dir: &Path,
    context: &Value,
) -> Result<Vec<Value>, ToolError> {
    let publication: Option<base_schema::PublishedBookRef> =
        serde_json::from_value(context.get("publication").cloned().unwrap_or(Value::Null))
            .map_err(|_| invalid("知识候选发布不可读"))?;
    let mut bindings: Vec<SourceBinding> =
        serde_json::from_value(context.get("source_bindings").cloned().unwrap_or(json!([])))
            .map_err(|_| invalid("知识候选来源不可读"))?;
    bindings.retain(|b| {
        b.book_id == book.base.book_id
            && b.source_revision.as_deref() == Some(book.source_fingerprint())
    });
    let mut ranges: Vec<EvidenceRange> = bindings
        .iter()
        .filter_map(|b| b.evidence_range.clone())
        .collect();
    ranges.extend(
        serde_json::from_value::<Vec<EvidenceRange>>(
            context
                .get("verified_read_ranges")
                .cloned()
                .unwrap_or(json!([])),
        )
        .map_err(|_| invalid("知识候选阅读范围不可读"))?,
    );
    if bindings.is_empty() && ranges.is_empty() {
        return Ok(vec![]);
    }
    let index = book.knowledge_index(dir, publication.as_ref())?;
    let spans: BTreeMap<_, _> = book
        .base
        .lid_nodes
        .iter()
        .map(|n| (n.lid.as_str(), &n.span))
        .collect();
    let mut seen = BTreeSet::new();
    let mut result = vec![];
    let mut nodes: Vec<_> = index
        .nodes
        .values()
        .filter(|n| {
            !matches!(
                n.reference.artifact.as_str(),
                "material" | "text" | "music_section" | "video_unit" | "navigation"
            )
        })
        .collect();
    nodes.sort_by_key(|n| {
        json!(n.reference) != context["teaching_context"]["knowledge_focus"]["reference"]
    });
    for node in nodes {
        let related = node.sources.iter().any(|source| match source {
            KnowledgeSource::Text { lid } => spans.get(lid.as_str()).is_some_and(|span| {
                ranges.iter().any(|r| {
                    spans
                        .get(r.start_lid.as_str())
                        .zip(spans.get(r.end_lid.as_str()))
                        .is_some_and(|(a, b)| span.start >= a.start && span.end <= b.end)
                })
            }),
            KnowledgeSource::Music { target } => {
                bindings.iter().filter_map(|b| b.music.as_ref()).any(|b| {
                    b.target.publication == target.publication
                        && b.target.audio.recording == target.audio.recording
                        && b.target.audio.start_ms < target.audio.end_ms
                        && target.audio.start_ms < b.target.audio.end_ms
                })
            }
            KnowledgeSource::Media {
                target, start_ms, ..
            } => bindings.iter().filter_map(|b| b.media.as_ref()).any(|b| {
                b.target == *target
                    || start_ms
                        .zip(read_tools::knowledge::media_start(&b.target))
                        .is_some_and(|(a, b)| (a - b).abs() < 1000.)
            }),
        });
        if related
            && seen.insert((
                node.reference.artifact.clone(),
                node.reference.entry_id.clone(),
            ))
        {
            let relations:Vec<_> = node.relations.iter().filter_map(|relation| {
                let target=index.nodes.get(&relation.target_id)?;
                Some(json!({"kind":relation.kind,"label":relation.label,"reference":target.reference,
                    "target_label":target.label,"summary":target.summary.chars().take(300).collect::<String>()}))
            }).collect();
            result.push(json!({"reference":node.reference,"label":node.label,"kind":node.kind,
                "summary":node.summary.chars().take(400).collect::<String>(),"relations":relations}));
            if result.len() == 32 {
                break;
            }
        }
    }
    Ok(result)
}

pub(crate) fn validate_links(
    links: &[read_tools::knowledge::KnowledgeLink],
    candidates: &[Value],
    expression: &str,
) -> Result<(), ToolError> {
    for link in links {
        if link.quote.trim().is_empty() || !expression.contains(&link.quote) {
            return Err(invalid("知识关联必须引用这次用户表达中的原话"));
        }
        if !candidates
            .iter()
            .any(|c| c["reference"] == json!(link.reference))
        {
            let available: Vec<_> = candidates.iter().map(|c| &c["reference"]).collect();
            return Err(invalid(&format!("知识关联只能选择本轮已提供的来源条目。当前不匹配的 reference：{}。本轮允许的完整 references：{}。请按真实含义复制其中的完整引用，不从 lesson.source、earlier_expressions 或旧观察拼接引用；没有对应候选时将 knowledge_links 设为 []，保留原话及来源范围。",
                json!(link.reference), json!(available))));
        }
    }
    Ok(())
}

pub(crate) fn focus(
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
) -> Result<Value, ToolError> {
    let reference: KnowledgeReference = serde_json::from_value(input["reference"].clone())
        .map_err(|_| invalid("知识位置不可读"))?;
    let index = workspace.book.knowledge_index(
        &workspace.book_dir,
        workspace.publication.as_ref().map(|p| &p.reference),
    )?;
    let node = input["node_id"]
        .as_str()
        .and_then(|id| index.nodes.get(id))
        .filter(|node| node.reference == reference)
        .ok_or_else(|| invalid("原知识位置已更新，请重新选择"))?;
    Ok(
        json!({"reference":node.reference,"node_id":node.id,"label":node.label,"summary":node.summary,"sources":node.sources,
        "path":index.path(&node.id).iter().map(|n|&n.label).collect::<Vec<_>>(),
        "instruction":"This is the learner-selected discussion focus and source navigation, not a read receipt or evidence of understanding. Read the relevant source before teaching; begin from this focus and invite the learner's own interpretation."}),
    )
}

pub(crate) fn evidence(
    user: &user_runtime::UserRuntime,
    input: &Value,
) -> Result<Value, ToolError> {
    if input["note_id"].is_string() {
        return crate::note_thinking::evidence(user, input, false);
    }
    let id = input["evidence_ref"]
        .as_str()
        .ok_or_else(|| invalid("缺少理解依据"))?;
    let workspace = input["workspace_id"].as_str();
    let candidate = user.learning_store_for_workspace(workspace)?.evidence(id)?;
    teaching::retained_evidence_view(user, workspace, &candidate.source_id, id)
}
pub(crate) fn resume(user: &user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    if input["note_id"].is_string() {
        return crate::note_thinking::start(user, input);
    }
    let e = evidence(user, input)?;
    if e["superseded"] == true {
        return Err(invalid("这条判断已更新，请刷新后继续"));
    }
    let session = user
        .agent_history
        .sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == e["source_session_id"].as_str())
        .ok_or_else(|| invalid("原讨论暂时不可用"))?;
    let turn = session
        .turns
        .iter()
        .find(|t| Some(t.turn_id.as_str()) == e["source_turn_id"].as_str())
        .ok_or_else(|| invalid("原表达暂时不可用"))?;
    Ok(
        json!({"context":{"book_id":e["source_id"],"publication_id":turn.published_book_ref.as_ref().map(|p|&p.publication_id),
        "workspace_id":session.learning_workspace_id,"session_id":session.id,"turn_id":turn.turn_id,"label":e["label"]},
        "source_revision":e["source_revision"],"target":e["target"],"understanding_ref":{"workspace_id":input["workspace_id"],"evidence_ref":e["evidence_id"]}}),
    )
}
