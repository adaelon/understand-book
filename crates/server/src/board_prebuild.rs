//! Board entries projected from the same admitted artifacts that the Reader uses.
use memory::boards::BoardArtifactKind;
use read_tools::{Book, ToolError};
use serde_json::{json, Value};

pub(crate) struct Content {
    pub title: String,
    pub text: String,
    pub snapshot: Value,
    pub lids: Vec<String>,
}

fn short(text: &str) -> String { text.trim().chars().take(90).collect() }
fn heading(book: &Book, lid: &str) -> String {
    book.text(lid, None).ok().and_then(|text| text.lines().find(|line| !line.trim().is_empty())
        .map(|line| short(line.trim_start_matches('#').trim()))).filter(|text| !text.is_empty())
        .unwrap_or_else(|| format!("原文 {lid}"))
}
fn node_kind(kind: &base_schema::GraphNodeType) -> &'static str {
    match kind { base_schema::GraphNodeType::Entity => "实体", base_schema::GraphNodeType::Concept => "概念", base_schema::GraphNodeType::Claim => "论断" }
}
fn relation(kind: &str) -> &str {
    match kind {
        "part_of" => "组成部分", "depends_on" => "依赖", "builds_on" => "建立于", "causes" => "导致",
        "supports" => "支持", "contradicts" => "相互矛盾", "defines" => "定义", "explains" => "解释",
        "example_of" => "例子", "contrasts_with" => "对照", "elaborates" => "展开", "prerequisite" => "前提",
        "prepares" => "铺垫", "exemplifies" => "举例说明", "answers" => "回答", "summarizes" => "总结",
        "concedes" => "让步", "continues" => "承接", "contrasts" => "对照", "results_in" => "带来结果",
        "restates" => "重述", "rebuts" => "反驳", "refines" => "细化",
        _ => kind,
    }
}
fn discourse_label(value: &str) -> &str {
    match value {
        "meta" => "阅读引导", "argumentative" => "论证", "descriptive" => "描述", "procedural" => "步骤说明",
        "informative" | "exposition" => "知识说明", "explanation" => "解释", "comparison" => "比较",
        "description" => "描述", "contrast" => "对照", "example" => "举例", "warning" => "提醒",
        "procedure_step" => "操作步骤", "evidence_report" => "呈现证据", "result_interpretation" => "解释结果",
        "question" => "提出问题", "application" => "应用", "summary" | "recap" => "回顾总结",
        "classification" => "分类", "definition" => "定义", "limitation" => "适用限制", "transition" => "过渡",
        "effect" => "结果与影响", "method_description" => "方法说明", "experiment_setup" => "实验设置",
        "cause" => "解释原因", "related_work" => "相关工作", "answer" => "回答问题", "counterexample" => "反例",
        "hypothesis" => "提出假设", "research_question" => "研究问题", "chapter_setup" => "章节导入",
        "argument_support" => "支撑论点", "prerequisite" => "前置知识", "main_point" => "核心观点",
        "worked_example" => "例题推演", "case_analysis" => "案例分析", "bridge_to_next" => "衔接下文",
        "result_claim" => "提出结论", "problem_framing" => "界定问题", "concept_elaboration" => "展开概念",
        "limitation_acknowledgement" => "说明限制", "method_setup" => "引入方法", "experiment_report" => "实验报告",
        "objection" => "提出异议", "related_work_positioning" => "联系已有工作", "resolution" => "解决问题",
        "abstract_summary" => "摘要", "experiment_design" => "实验设计", _ => value,
    }
}
fn section(text: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) { text.push_str(&format!("\n\n**{label}**\n\n{value}")); }
}
fn finish(title: String, text: String, snapshot: Value, mut lids: Vec<String>) -> Content {
    let mut seen = std::collections::BTreeSet::new();
    lids.retain(|lid| seen.insert(lid.clone()));
    Content { title, text, snapshot, lids }
}

/// The visitor lets the API search all candidates while retaining only one page.
pub(crate) fn catalog(book: &Book, category: &str, add: &mut impl FnMut(Value)) {
    let mut entry = |artifact: BoardArtifactKind, id: String, title: String, summary: String| {
        add(json!({"kind":"prebuild","category":artifact,"title":title,"summary":summary,
            "source":{"kind":"artifact","artifact":artifact,"entry_id":id,"source_revision":book.source_fingerprint()}}));
    };
    // Whole-book orientation is useful immediately, before the thousands of local entries.
    if matches!(category, "all" | "book_structure") {
        if let Some(structure) = book.book_structure() {
            for unit in &structure.spine {
                entry(BoardArtifactKind::BookStructure, format!("spine:{}", unit.lid), heading(book, &unit.lid), unit.summary.text.clone());
            }
            for theme in &structure.throughlines {
                entry(BoardArtifactKind::BookStructure, format!("throughline:{}", theme.id), theme.name.clone(), theme.summary.text.clone());
            }
            for stop in &structure.key_stops {
                entry(BoardArtifactKind::BookStructure, format!("key_stop:{}", stop.id), stop.title.clone().unwrap_or_else(|| short(&stop.reason.text)), stop.reason.text.clone());
            }
        }
    }
    if matches!(category, "all" | "pass1") {
        for node in &book.base.graph_nodes {
            entry(BoardArtifactKind::Pass1, node.id.clone(), node.name.clone(), format!("{} · {}", node_kind(&node.node_type), node.source_lid.as_deref().unwrap_or_else(|| node.occurrences.first().map(String::as_str).unwrap_or_default())));
        }
    }
    if matches!(category, "all" | "discourse") {
        for item in book.discourse_items() {
            let summary = item.local_summary.as_deref().filter(|s| !s.trim().is_empty());
            entry(BoardArtifactKind::Discourse, item.lid.clone(), summary.map(short).unwrap_or_else(|| heading(book, &item.lid)),
                format!("{}\n{}", item.lid, summary.unwrap_or_default()));
        }
    }
}

pub(crate) fn resolve(book: &Book, artifact: BoardArtifactKind, id: &str) -> Result<Content, ToolError> {
    let missing = || ToolError { error_code: "BOARD_REQUEST_INVALID".into(), category: "validation".into(), message: "这项预构建内容已不可用，请重新选择。".into() };
    match artifact {
        BoardArtifactKind::Pass1 => {
            let node = book.base.graph_nodes.iter().find(|node| node.id == id).ok_or_else(missing)?;
            let edges: Vec<_> = book.base.graph_edges.iter().filter(|edge| edge.source == id || edge.target == id).collect();
            let mut text = format!("{}\n\n类型：{}", node.name, node_kind(&node.node_type));
            let mut lids = node.occurrences.clone();
            lids.extend(node.source_lid.clone());
            if !edges.is_empty() {
                text.push_str("\n\n### 关系线索");
                for edge in &edges {
                    let source = book.base.graph_nodes.iter().find(|node| node.id == edge.source);
                    let target = book.base.graph_nodes.iter().find(|node| node.id == edge.target);
                    text.push_str(&format!("\n\n- {} {} {}（{}）", source.map(|n| n.name.as_str()).unwrap_or(&edge.source),
                        if edge.direction == base_schema::Direction::Undirected { "↔" } else { "→" },
                        target.map(|n| n.name.as_str()).unwrap_or(&edge.target), relation(&edge.edge_type)));
                }
            }
            Ok(finish(node.name.clone(), text, json!({"node":node,"relations":edges}), lids))
        }
        BoardArtifactKind::Discourse => {
            let item = book.discourse_item(id).ok_or_else(missing)?;
            let title = item.local_summary.as_deref().filter(|s| !s.trim().is_empty()).map(short).unwrap_or_else(|| heading(book, id));
            let mut text = item.local_summary.clone().unwrap_or_else(|| title.clone());
            section(&mut text, "段落模式", Some(discourse_label(&item.mode)));
            section(&mut text, "局部作用", item.local_function.as_deref().map(discourse_label));
            section(&mut text, "行文推进", item.rhetorical_move.as_deref().map(discourse_label));
            let mut lids = vec![item.lid.clone()];
            if !item.relations.is_empty() {
                text.push_str("\n\n### 篇章关系");
                for link in &item.relations {
                    text.push_str(&format!("\n\n- {}：{}（{}）", relation(&link.relation_type), heading(book, &link.target_lid), link.target_lid));
                    lids.push(link.target_lid.clone());
                    lids.extend(link.evidence_lids.clone());
                }
            }
            Ok(finish(title, text, json!(item), lids))
        }
        BoardArtifactKind::BookStructure => {
            let structure = book.book_structure().ok_or_else(missing)?;
            if let Some(lid) = id.strip_prefix("spine:") {
                let unit = structure.spine.iter().find(|unit| unit.lid == lid).ok_or_else(missing)?;
                let mut text = unit.summary.text.clone();
                let mut lids = vec![unit.lid.clone()];
                lids.extend(unit.summary.evidence_lids.clone());
                if !unit.depends_on.is_empty() {
                    text.push_str("\n\n### 前置章节");
                    for lid in &unit.depends_on { text.push_str(&format!("\n\n- {}（{lid}）", heading(book, lid))); lids.push(lid.clone()); }
                }
                let stops: Vec<_> = structure.key_stops.iter().filter(|stop| unit.key_stop_ids.contains(&stop.id)).collect();
                append_stops(&mut text, &mut lids, &stops);
                Ok(finish(heading(book, lid), text, json!({"header":structure.header,"unit":unit,"key_stops":stops}), lids))
            } else if let Some(id) = id.strip_prefix("throughline:") {
                let theme = structure.throughlines.iter().find(|theme| theme.id == id).ok_or_else(missing)?;
                let mut text = theme.summary.text.clone();
                let mut lids = theme.lids.clone();
                lids.extend(theme.summary.evidence_lids.clone());
                text.push_str("\n\n### 贯穿章节");
                for lid in &theme.lids { text.push_str(&format!("\n\n- {}（{lid}）", heading(book, lid))); }
                let stops: Vec<_> = structure.key_stops.iter().filter(|stop| theme.key_stop_ids.contains(&stop.id)).collect();
                append_stops(&mut text, &mut lids, &stops);
                Ok(finish(theme.name.clone(), text, json!({"header":structure.header,"throughline":theme,"key_stops":stops}), lids))
            } else if let Some(id) = id.strip_prefix("key_stop:") {
                let stop = structure.key_stops.iter().find(|stop| stop.id == id).ok_or_else(missing)?;
                let mut lids = vec![stop.lid.clone()]; lids.extend(stop.reason.evidence_lids.clone());
                Ok(finish(stop.title.clone().unwrap_or_else(|| short(&stop.reason.text)), stop.reason.text.clone(), json!({"header":structure.header,"key_stop":stop}), lids))
            } else { Err(missing()) }
        }
    }
}

fn append_stops(text: &mut String, lids: &mut Vec<String>, stops: &[&read_tools::BookStructureKeyStop]) {
    if stops.is_empty() { return; }
    text.push_str("\n\n### 阅读重点");
    for stop in stops {
        if let Some(title) = &stop.title { text.push_str(&format!("\n\n#### {title}")); }
        text.push_str(&format!("\n\n{}", stop.reason.text));
        lids.push(stop.lid.clone()); lids.extend(stop.reason.evidence_lids.clone());
    }
}
