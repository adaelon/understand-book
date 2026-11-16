//! A deterministic, publication-local navigation view over admitted knowledge.
use crate::{Book, ToolError};
use base_schema::{EvidenceTarget, MusicSourceTarget, NodeKind, PublishedBookRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct KnowledgeReference {
    pub source_id: String,
    pub source_revision: String,
    pub publication_id: Option<String>,
    pub artifact: String,
    pub entry_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct KnowledgeLink {
    pub reference: KnowledgeReference,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KnowledgeSource {
    Text {
        lid: String,
    },
    Media {
        target: EvidenceTarget,
        start_ms: Option<f64>,
        end_ms: Option<f64>,
    },
    Music {
        target: MusicSourceTarget,
    },
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct KnowledgeRelation {
    pub kind: String,
    pub target_id: String,
    pub label: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct KnowledgeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub reference: KnowledgeReference,
    pub label: String,
    pub kind: String,
    pub summary: String,
    pub sources: Vec<KnowledgeSource>,
    pub relations: Vec<KnowledgeRelation>,
    pub children: Vec<String>,
}

pub struct Index {
    pub nodes: BTreeMap<String, KnowledgeNode>,
    references: BTreeMap<(String, String), Vec<String>>,
    source_scopes: BTreeMap<String, String>,
    object_placements: BTreeMap<(String, u64), Vec<String>>,
    pub modality: String,
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "KNOWLEDGE_INVALID".into(),
        category: "validation".into(),
        message: message.into(),
    }
}
fn read(path: &Path) -> Result<Value, ToolError> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| invalid(e.to_string()))?)
        .map_err(|e| invalid(e.to_string()))
}
fn short(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}
fn identity(artifact: &str, id: &str, occurrence: &str) -> String {
    json!([artifact, id, occurrence]).to_string()
}
pub fn media_start(target: &EvidenceTarget) -> Option<f64> {
    match target {
        EvidenceTarget::Visual { frames, .. } => {
            frames.iter().map(|f| f.actual_pts_ms).reduce(f64::min)
        }
        EvidenceTarget::Audio { location, .. } => Some(location.start_ms),
        EvidenceTarget::Joint { members } => members
            .iter()
            .filter_map(|m| media_start(&m.target))
            .reduce(f64::min),
        _ => None,
    }
}
fn media_lids(target: &EvidenceTarget, lids: &mut Vec<String>) {
    match target {
        EvidenceTarget::Text { text_range, .. } => lids.push(text_range.lid.clone()),
        EvidenceTarget::Joint { members } => {
            for member in members {
                media_lids(&member.target, lids);
            }
        }
        _ => {}
    }
}

// Reserve the lazy view before publication admission, independent of which user
// opens it first. Media assets themselves are streamed and are not included.
pub(super) fn asset_bytes(dir: &Path) -> u64 {
    let mut files = vec![
        "music_source_manifest.json".to_owned(),
        "formal_objects.json".into(),
    ];
    if let Ok(catalog) = read(&dir.join("video_enrichments.json")) {
        files.extend(
            catalog["batches"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|b| b["files"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .filter(|f| f.ends_with("/visual_units.json"))
                .map(str::to_owned),
        );
    }
    files
        .iter()
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok())
        .map(|m| m.len())
        .sum()
}
pub(super) fn reserved_bytes(book: &Book) -> u64 {
    let mut bytes = (book.base.lid_nodes.len() as u64).saturating_mul(2048);
    for node in &book.base.graph_nodes {
        let size = serde_json::to_vec(node).map_or(0, |v| v.len() as u64);
        bytes = bytes.saturating_add(size.saturating_mul(8).saturating_add(1024).saturating_mul(
            (node.occurrences.len() + usize::from(node.source_lid.is_some())).max(1) as u64,
        ));
    }
    let sidecars = json!([
        book.base.graph_edges,
        book.discourse_index,
        book.video_discourse,
        book.book_structure
    ]);
    bytes
        .saturating_add(
            serde_json::to_vec(&sidecars)
                .map_or(0, |v| v.len() as u64)
                .saturating_mul(8),
        )
        .saturating_add(book.knowledge_asset_bytes.saturating_mul(4))
}

impl Index {
    fn add(&mut self, node: KnowledgeNode) {
        self.references
            .entry((
                node.reference.artifact.clone(),
                node.reference.entry_id.clone(),
            ))
            .or_default()
            .push(node.id.clone());
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn build(
        book: &Book,
        dir: &Path,
        publication: Option<&PublishedBookRef>,
    ) -> Result<Self, ToolError> {
        let reference = |artifact: &str, id: &str| KnowledgeReference {
            source_id: book.base.book_id.clone(),
            source_revision: book.source_fingerprint().into(),
            publication_id: publication.map(|p| p.publication_id.clone()),
            artifact: artifact.into(),
            entry_id: id.into(),
        };
        let make = |id: String,
                    parent: Option<String>,
                    artifact: &str,
                    entry: &str,
                    label: String,
                    kind: &str,
                    summary: String,
                    sources| KnowledgeNode {
            id,
            parent_id: parent,
            reference: reference(artifact, entry),
            label,
            kind: kind.into(),
            summary,
            sources,
            relations: vec![],
            children: vec![],
        };
        let mut tree = Self {
            nodes: BTreeMap::new(),
            references: BTreeMap::new(),
            source_scopes: BTreeMap::new(),
            object_placements: BTreeMap::new(),
            modality: "text".into(),
        };
        tree.add(make(
            "root".into(),
            None,
            "material",
            &book.base.book_id,
            "全部内容".into(),
            "material",
            String::new(),
            vec![],
        ));
        if dir.join("music_source_manifest.json").exists() {
            tree.modality = "music".into();
            let m = book.load_music_source(dir)?;
            tree.nodes.get_mut("root").unwrap().label = m.title.clone();
            let publication = publication
                .cloned()
                .ok_or_else(|| invalid("音乐知识树需要确切发布"))?;
            let target = |start,
                          end,
                          event_ids: Vec<String>,
                          occurrence_ids: Vec<String>,
                          evidence: &[base_schema::MusicEvidence]| {
                let mut score_ids = vec![];
                let mut provenance_ids = vec![];
                for e in evidence {
                    score_ids.extend(e.score_object_ids.clone());
                    provenance_ids.push(e.provenance_id.clone());
                }
                score_ids.sort();
                score_ids.dedup();
                provenance_ids.sort();
                provenance_ids.dedup();
                MusicSourceTarget {
                    publication: publication.clone(),
                    audio: base_schema::MusicAudioLocation {
                        recording: m.recording.source.clone(),
                        start_ms: start,
                        end_ms: end,
                    },
                    score: (!score_ids.is_empty()).then(|| base_schema::MusicScoreLocation {
                        score: m.score.source.clone(),
                        source_ids: score_ids,
                    }),
                    event_ids,
                    occurrence_ids,
                    provenance_ids,
                }
            };
            for s in &m.structure {
                let sources = s
                    .start_ms
                    .zip(s.end_ms)
                    .map(|(start, end)| {
                        vec![KnowledgeSource::Music {
                            target: target(start, end, vec![], vec![], &[]),
                        }]
                    })
                    .unwrap_or_default();
                let summary = if s.coverage == base_schema::MusicStructureCoverage::NavigationOnly {
                    format!("{}\n\n曲式说明目前仅提供结构导航。", s.description)
                } else {
                    s.description.clone()
                };
                tree.add(make(
                    format!("music_section:{}", s.section_id),
                    Some(
                        s.parent_id
                            .as_ref()
                            .map(|id| format!("music_section:{id}"))
                            .unwrap_or("root".into()),
                    ),
                    "music_section",
                    &s.section_id,
                    s.title.clone(),
                    "music_section",
                    summary,
                    sources,
                ));
            }
            for e in &m.events {
                let occurrences = m
                    .occurrences
                    .iter()
                    .filter(|o| {
                        e.start_ms < o.end_ms
                            && o.start_ms < e.end_ms
                            && (e.occurrence_ids.is_empty()
                                || e.occurrence_ids.contains(&o.occurrence_id))
                    })
                    .map(|o| o.occurrence_id.clone())
                    .collect::<Vec<_>>();
                let roles = e
                    .roles
                    .iter()
                    .map(|r| {
                        format!(
                            "{}：{}",
                            m.parts
                                .iter()
                                .find(|p| p.part_id == r.part_id)
                                .map(|p| p.name.as_str())
                                .unwrap_or(&r.part_id),
                            r.role
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("；");
                let parents: Vec<_> = m
                    .structure
                    .iter()
                    .filter(|s| s.event_ids.contains(&e.event_id))
                    .map(|s| format!("music_section:{}", s.section_id))
                    .collect();
                for parent in if parents.is_empty() {
                    vec!["root".into()]
                } else {
                    parents
                } {
                    let mut node = make(
                        identity("music_event", &e.event_id, &parent),
                        Some(parent),
                        "music_event",
                        &e.event_id,
                        e.title.clone(),
                        "music_event",
                        format!("{}\n\n{}\n\n{}", e.explanation, e.listen_for, roles)
                            .trim()
                            .into(),
                        vec![KnowledgeSource::Music {
                            target: target(
                                e.start_ms,
                                e.end_ms,
                                vec![e.event_id.clone()],
                                occurrences.clone(),
                                &e.evidence,
                            ),
                        }],
                    );
                    node.relations = e
                        .relations
                        .iter()
                        .map(|r| KnowledgeRelation {
                            available: false,
                            kind: serde_json::to_value(r.kind)
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .into(),
                            target_id: format!("music_event:{}", r.target_event_id),
                            label: r.description.clone(),
                        })
                        .collect();
                    tree.add(node);
                }
            }
        } else {
            let mut parents = BTreeMap::new();
            for n in &book.base.lid_nodes {
                for child in &n.children {
                    parents.insert(child.as_str(), n.lid.as_str());
                }
            }
            // LIDs locate evidence. Only chapter/section scopes are visible
            // navigation; paragraphs, formulas and images are not knowledge branches.
            let scopes: BTreeSet<_> = book
                .base
                .lid_nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Chapter | NodeKind::Section))
                .map(|n| n.lid.as_str())
                .collect();
            for n in &book.base.lid_nodes {
                let mut at = Some(n.lid.as_str());
                let mut scope = "root".to_owned();
                while let Some(lid) = at {
                    if scopes.contains(lid) {
                        scope = format!("text:{lid}");
                        break;
                    }
                    at = parents.get(lid).copied();
                }
                tree.source_scopes.insert(n.lid.clone(), scope);
            }
            for n in book
                .base
                .lid_nodes
                .iter()
                .filter(|n| scopes.contains(n.lid.as_str()))
            {
                let start = n.span.start as usize;
                let end = (start + 320)
                    .min(n.span.end as usize)
                    .min(book.source_u16.len());
                let text = String::from_utf16_lossy(&book.source_u16[start..end]);
                let heading = text
                    .lines()
                    .find(|s| !s.trim().is_empty())
                    .unwrap_or(&n.lid)
                    .trim_start_matches('#')
                    .trim();
                let summary = book
                    .book_structure()
                    .and_then(|s| s.spine.iter().find(|s| s.lid == n.lid))
                    .map(|s| s.summary.text.clone())
                    .unwrap_or_default();
                let parent = parents
                    .get(n.lid.as_str())
                    .and_then(|lid| tree.source_scopes.get(*lid))
                    .cloned()
                    .unwrap_or("root".into());
                tree.add(make(
                    format!("text:{}", n.lid),
                    Some(parent),
                    "text",
                    &n.lid,
                    short(heading, 80),
                    if n.kind == NodeKind::Chapter {
                        "chapter"
                    } else {
                        "section"
                    },
                    summary,
                    vec![KnowledgeSource::Text { lid: n.lid.clone() }],
                ));
            }
            if tree.nodes.len() == 1 {
                if let Some(first) = book.base.lid_nodes.first() {
                    tree.nodes
                        .get_mut("root")
                        .unwrap()
                        .sources
                        .push(KnowledgeSource::Text {
                            lid: first.lid.clone(),
                        });
                }
            }
            // Navigation can expose admitted partial coverage; it never registers a media read.
            let video = if dir.join("video_source_manifest.json").exists() {
                let manifest: base_schema::VideoSourceManifest =
                    serde_json::from_value(read(&dir.join("video_source_manifest.json"))?)
                        .map_err(|e| invalid(e.to_string()))?;
                manifest.validate().map_err(invalid)?;
                let map: base_schema::TranscriptMap =
                    serde_json::from_value(read(&dir.join(&manifest.transcript_map_path))?)
                        .map_err(|e| invalid(e.to_string()))?;
                book.validate_video_source(&manifest, &map)?;
                Some((manifest, map))
            } else {
                None
            };
            let mut units: Vec<(String, f64, f64)> = vec![];
            if video.is_some() {
                tree.modality = "video".into();
                if dir.join("video_enrichments.json").exists() {
                    let catalog = read(&dir.join("video_enrichments.json"))?;
                    for file in catalog["batches"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|b| b["files"].as_array().into_iter().flatten())
                        .filter_map(Value::as_str)
                        .filter(|f| f.ends_with("/visual_units.json"))
                    {
                        let contribution = read(&dir.join(file))?;
                        for u in contribution["units"].as_array().into_iter().flatten() {
                            let Some(id) = u["vu_id"].as_str() else {
                                continue;
                            };
                            let location: base_schema::MediaLocation =
                                serde_json::from_value(u["location"].clone())
                                    .map_err(|e| invalid(e.to_string()))?;
                            let frames = u["representative_frames"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|f| serde_json::from_value(f["frame"].clone()).ok())
                                .collect();
                            let pubref = publication
                                .cloned()
                                .ok_or_else(|| invalid("视频知识树需要确切发布"))?;
                            let target = EvidenceTarget::Visual {
                                publication: pubref,
                                frames,
                                regions: vec![],
                            };
                            let label = short(u["description"].as_str().unwrap_or("视频片段"), 80);
                            let key = format!("video_unit:{id}");
                            tree.add(make(
                                key.clone(),
                                Some("root".into()),
                                "video_unit",
                                id,
                                label,
                                "video_unit",
                                u["description"].as_str().unwrap_or_default().into(),
                                vec![KnowledgeSource::Media {
                                    target,
                                    start_ms: Some(location.start_ms),
                                    end_ms: Some(location.end_ms),
                                }],
                            ));
                            units.push((key, location.start_ms, location.end_ms));
                        }
                    }
                }
            }
            let media_sources =
                |evidence: &[base_schema::JointEvidenceMember]| -> Vec<KnowledgeSource> {
                    evidence
                        .iter()
                        .map(|m| {
                            let start = media_start(&m.target).or_else(|| {
                                let mut lids = vec![];
                                media_lids(&m.target, &mut lids);
                                video.as_ref().and_then(|(_, map)| {
                                    map.segments
                                        .iter()
                                        .filter(|s| lids.contains(&s.text_range.lid))
                                        .map(|s| s.location.start_ms)
                                        .reduce(f64::min)
                                })
                            });
                            KnowledgeSource::Media {
                                target: m.target.clone(),
                                start_ms: start,
                                end_ms: None,
                            }
                        })
                        .collect()
                };
            let names: BTreeMap<_, _> = book
                .base
                .graph_nodes
                .iter()
                .map(|n| (&n.id, &n.name))
                .collect();
            let mut edges: BTreeMap<_, Vec<_>> = BTreeMap::new();
            for e in &book.base.graph_edges {
                edges.entry(&e.source).or_default().push(e);
            }
            for n in &book.base.graph_nodes {
                let mut lids = n.occurrences.clone();
                lids.extend(n.source_lid.clone());
                lids.sort();
                lids.dedup();
                let mut sources: Vec<_> = lids
                    .iter()
                    .map(|lid| KnowledgeSource::Text { lid: lid.clone() })
                    .collect();
                sources.extend(media_sources(&n.evidence));
                let mut placements: Vec<_> = lids
                    .iter()
                    .filter_map(|lid| tree.source_scopes.get(lid).cloned())
                    .collect();
                for source in &sources {
                    if let KnowledgeSource::Media {
                        start_ms: Some(ms), ..
                    } = source
                    {
                        placements.extend(
                            units
                                .iter()
                                .filter(|(_, start, end)| start <= ms && ms < end)
                                .map(|(id, _, _)| id.clone()),
                        );
                    }
                }
                placements.sort();
                placements.dedup();
                if placements.is_empty() {
                    placements.push("root".into());
                }
                for parent in placements {
                    let local_sources: Vec<_> = sources
                        .iter()
                        .filter(|source| match source {
                            KnowledgeSource::Text { lid } => {
                                tree.source_scopes.get(lid) == Some(&parent)
                            }
                            KnowledgeSource::Media {
                                start_ms: Some(ms), ..
                            } => units
                                .iter()
                                .any(|(id, start, end)| id == &parent && start <= ms && ms < end),
                            _ => false,
                        })
                        .cloned()
                        .collect();
                    let mut node = make(
                        identity("pass1", &n.id, &parent),
                        Some(parent),
                        "pass1",
                        &n.id,
                        n.name.clone(),
                        match n.node_type {
                            base_schema::GraphNodeType::Entity => "entity",
                            base_schema::GraphNodeType::Concept => "concept",
                            base_schema::GraphNodeType::Claim => "claim",
                        },
                        String::new(),
                        if local_sources.is_empty() {
                            sources.clone()
                        } else {
                            local_sources
                        },
                    );
                    node.relations = edges
                        .get(&n.id)
                        .into_iter()
                        .flatten()
                        .map(|e| KnowledgeRelation {
                            available: false,
                            kind: e.edge_type.clone(),
                            target_id: format!("pass1:{}", e.target),
                            label: names
                                .get(&e.target)
                                .map(|name| (*name).clone())
                                .unwrap_or(e.target.clone()),
                        })
                        .collect();
                    tree.add(node);
                }
            }
            let discourse_labels: BTreeMap<_, _> = book
                .discourse_items()
                .iter()
                .filter_map(|d| d.local_summary.as_deref().map(|s| (&d.lid, s)))
                .collect();
            for d in book.discourse_items() {
                let Some(summary) = d.local_summary.as_deref().filter(|s| !s.trim().is_empty())
                else {
                    continue;
                };
                let scope = tree
                    .source_scopes
                    .get(&d.lid)
                    .cloned()
                    .unwrap_or("root".into());
                let parent = format!("explanations:{scope}");
                if !tree.nodes.contains_key(&parent) {
                    let sources = tree
                        .nodes
                        .get(&scope)
                        .map(|n| n.sources.clone())
                        .unwrap_or_default();
                    tree.add(make(
                        parent.clone(),
                        Some(scope),
                        "navigation",
                        &parent,
                        "解释、例子与论证".into(),
                        "discourse_group",
                        "这部分保留材料已有的解释、例子和论证过程；各项均可返回对应原文。".into(),
                        sources,
                    ));
                }
                let mut node = make(
                    identity("discourse", &d.lid, &parent),
                    Some(parent),
                    "discourse",
                    &d.lid,
                    short(summary, 90),
                    d.local_function.as_deref().unwrap_or("discourse"),
                    summary.into(),
                    vec![KnowledgeSource::Text { lid: d.lid.clone() }],
                );
                node.relations = d
                    .relations
                    .iter()
                    .map(|r| KnowledgeRelation {
                        available: false,
                        kind: r.relation_type.clone(),
                        target_id: format!("discourse:{}", r.target_lid),
                        label: discourse_labels
                            .get(&r.target_lid)
                            .copied()
                            .unwrap_or("相关原文")
                            .to_owned(),
                    })
                    .collect();
                tree.add(node);
            }
            if let Some(structure) = book.book_structure() {
                for stop in &structure.key_stops {
                    let parent = tree
                        .source_scopes
                        .get(&stop.lid)
                        .cloned()
                        .unwrap_or("root".into());
                    tree.add(make(
                        identity("book_structure", &format!("key_stop:{}", stop.id), &parent),
                        Some(if tree.nodes.contains_key(&parent) {
                            parent
                        } else {
                            "root".into()
                        }),
                        "book_structure",
                        &format!("key_stop:{}", stop.id),
                        stop.title
                            .clone()
                            .unwrap_or_else(|| short(&stop.reason.text, 80)),
                        "key_stop",
                        stop.reason.text.clone(),
                        vec![KnowledgeSource::Text {
                            lid: stop.lid.clone(),
                        }],
                    ));
                }
                if !structure.throughlines.is_empty() {
                    tree.add(make(
                        "themes".into(),
                        Some("root".into()),
                        "navigation",
                        "themes",
                        "贯穿内容的主题".into(),
                        "themes",
                        String::new(),
                        vec![],
                    ));
                    for theme in &structure.throughlines {
                        tree.add(make(
                            identity(
                                "book_structure",
                                &format!("throughline:{}", theme.id),
                                "themes",
                            ),
                            Some("themes".into()),
                            "book_structure",
                            &format!("throughline:{}", theme.id),
                            theme.name.clone(),
                            "theme",
                            theme.summary.text.clone(),
                            theme
                                .lids
                                .iter()
                                .map(|lid| KnowledgeSource::Text { lid: lid.clone() })
                                .collect(),
                        ));
                    }
                }
            }
            for d in &book.video_discourse {
                let value = serde_json::to_value(d).unwrap();
                let id = value["id"].as_str().unwrap();
                let parents: Vec<_> = value["vu_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|id| format!("video_unit:{id}"))
                    .filter(|id| tree.nodes.contains_key(id))
                    .collect();
                let evidence: Vec<base_schema::JointEvidenceMember> =
                    serde_json::from_value(value["evidence"].clone())
                        .map_err(|e| invalid(e.to_string()))?;
                for parent in if parents.is_empty() {
                    vec!["root".into()]
                } else {
                    parents
                } {
                    let mut node = make(
                        identity("video_discourse", id, &parent),
                        Some(parent),
                        "video_discourse",
                        id,
                        short(value["local_summary"].as_str().unwrap_or_default(), 90),
                        value["local_function"].as_str().unwrap_or("discourse"),
                        value["local_summary"].as_str().unwrap_or_default().into(),
                        media_sources(&evidence),
                    );
                    node.relations = value["relations"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|r| KnowledgeRelation {
                            available: false,
                            kind: r["type"].as_str().unwrap_or_default().into(),
                            target_id: format!(
                                "video_discourse:{}",
                                r["target_id"].as_str().unwrap_or_default()
                            ),
                            label: r["rationale"].as_str().unwrap_or_default().into(),
                        })
                        .collect();
                    tree.add(node);
                }
            }
        }
        // Resolve relation endpoints to real placements. Missing counterparts remain source labels.
        let relation_ids: BTreeMap<_, _> = tree
            .references
            .iter()
            .filter_map(|((kind, id), placements)| {
                placements
                    .first()
                    .map(|p| (format!("{kind}:{id}"), p.clone()))
            })
            .collect();
        for n in tree.nodes.values_mut() {
            for r in &mut n.relations {
                if let Some(id) = relation_ids.get(&r.target_id) {
                    r.target_id = id.clone();
                    r.available = true;
                }
            }
        }
        if dir.join("formal_objects.json").exists() {
            let objects = read(&dir.join("formal_objects.json"))?;
            if objects["source_id"] == book.base.book_id
                && objects["source_revision"] == book.source_fingerprint()
            {
                let mut latest = BTreeMap::<String, u64>::new();
                for object in objects["objects"].as_array().into_iter().flatten() {
                    if let Some(revision) = object["object_revision"].as_u64() {
                        let current = latest.entry(object["ref"].to_string()).or_default();
                        *current = (*current).max(revision);
                    }
                }
                for object in objects["objects"].as_array().into_iter().flatten() {
                    let Some(id) = object["ref"]["object_id"].as_str() else {
                        continue;
                    };
                    let Some(revision) = object["object_revision"].as_u64() else {
                        continue;
                    };
                    if latest.get(&object["ref"].to_string()) != Some(&revision) {
                        continue;
                    }
                    if !objects["active_refs"]
                        .as_array()
                        .is_some_and(|refs| refs.contains(&object["ref"]))
                    {
                        continue;
                    }
                    for (candidate, refs) in
                        objects["candidate_refs"].as_object().into_iter().flatten()
                    {
                        if refs
                            .as_array()
                            .is_some_and(|refs| refs.contains(&object["ref"]))
                        {
                            tree.object_placements
                                .entry((id.into(), revision))
                                .or_default()
                                .extend(
                                    tree.references
                                        .get(&("pass1".into(), candidate.clone()))
                                        .into_iter()
                                        .flatten()
                                        .cloned(),
                                );
                        }
                    }
                }
            }
        }
        // Busy sections expose a few navigable branches. Keep semantic identities
        // and source locations intact; these groups do not assert new knowledge.
        let mut semantic_children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for node in tree.nodes.values() {
            if node.reference.artifact == "pass1" || node.kind == "key_stop" {
                if let Some(parent) = &node.parent_id {
                    semantic_children
                        .entry(parent.clone())
                        .or_default()
                        .push(node.id.clone());
                }
            }
        }
        for (scope, ids) in semantic_children {
            if ids.len() <= 6 {
                continue;
            }
            for (kind, label, claims) in [
                ("claims_group", "要弄清的关系", true),
                ("concepts_group", "会用到的概念", false),
            ] {
                let members: Vec<_> = ids
                    .iter()
                    .filter(|id| {
                        matches!(tree.nodes[*id].kind.as_str(), "claim" | "key_stop") == claims
                    })
                    .cloned()
                    .collect();
                if members.is_empty() {
                    continue;
                }
                let id = format!("{kind}:{scope}");
                tree.add(make(
                    id.clone(),
                    Some(scope.clone()),
                    "navigation",
                    &id,
                    label.into(),
                    kind,
                    String::new(),
                    tree.nodes[&scope].sources.clone(),
                ));
                for member in members {
                    tree.nodes.get_mut(&member).unwrap().parent_id = Some(id.clone());
                }
            }
        }
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for n in tree.nodes.values() {
            if let Some(parent) = &n.parent_id {
                children
                    .entry(parent.clone())
                    .or_default()
                    .push(n.id.clone());
            }
        }
        let order = |node: &KnowledgeNode| -> f64 {
            node.sources
                .iter()
                .filter_map(|s| match s {
                    KnowledgeSource::Text { lid } => book
                        .lid_idx
                        .get(lid)
                        .map(|i| book.base.lid_nodes[*i].span.start as f64),
                    KnowledgeSource::Media { start_ms, .. } => *start_ms,
                    KnowledgeSource::Music { target } => Some(target.audio.start_ms),
                })
                .reduce(f64::min)
                .unwrap_or(0.)
        };
        for (parent, mut ids) in children {
            let priority = |n: &KnowledgeNode| match n.kind.as_str() {
                "chapter" | "section" => 0,
                "concepts_group" => 2,
                "discourse_group" => 3,
                _ => 1,
            };
            ids.sort_by(|a, b| {
                priority(&tree.nodes[a])
                    .cmp(&priority(&tree.nodes[b]))
                    .then(order(&tree.nodes[a]).total_cmp(&order(&tree.nodes[b])))
                    .then(a.cmp(b))
            });
            if let Some(p) = tree.nodes.get_mut(&parent) {
                p.children = ids;
            }
        }
        Ok(tree)
    }

    /// Existing overview, or the first local explanation in source order.
    pub fn introduction(&self, id: &str) -> Option<&KnowledgeNode> {
        let node = self.nodes.get(id)?;
        if !node.summary.trim().is_empty() {
            return Some(node);
        }
        let explanations = node
            .children
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .find(|n| n.kind == "discourse_group")?;
        explanations
            .children
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .find(|n| !n.summary.trim().is_empty())
    }

    /// A transparent starting point, not a computed importance ranking.
    pub fn entry_point(&self, id: &str) -> Option<&KnowledgeNode> {
        let node = self.nodes.get(id)?;
        let children: Vec<_> = node
            .children
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .collect();
        children
            .iter()
            .find(|n| n.kind == "key_stop")
            .copied()
            .or_else(|| {
                children
                    .iter()
                    .find(|n| n.kind == "claims_group")
                    .and_then(|n| self.entry_point(&n.id))
            })
            .or_else(|| children.iter().find(|n| n.kind == "claim").copied())
            .or_else(|| children.iter().find(|n| n.kind == "music_event").copied())
            .or_else(|| children.first().copied())
    }

    pub fn placements(&self, reference: &KnowledgeReference) -> Vec<&KnowledgeNode> {
        self.references
            .get(&(reference.artifact.clone(), reference.entry_id.clone()))
            .into_iter()
            .flatten()
            .filter_map(|id| self.nodes.get(id))
            .filter(|node| node.reference == *reference)
            .collect()
    }
    pub fn object_placements(&self, id: &str, revision: u64) -> Vec<&KnowledgeNode> {
        self.object_placements
            .get(&(id.into(), revision))
            .into_iter()
            .flatten()
            .filter_map(|id| self.nodes.get(id))
            .collect()
    }
    pub fn scope_for_lid(&self, lid: &str) -> Option<&str> {
        self.source_scopes.get(lid).map(String::as_str)
    }
    pub fn path(&self, id: &str) -> Vec<&KnowledgeNode> {
        let mut result = vec![];
        let mut next = self.nodes.get(id).or_else(|| {
            id.strip_prefix("text:")
                .and_then(|lid| self.scope_for_lid(lid))
                .and_then(|scope| self.nodes.get(scope))
        });
        let mut seen = BTreeSet::new();
        while let Some(n) = next {
            if !seen.insert(&n.id) {
                break;
            }
            result.push(n);
            next = n.parent_id.as_ref().and_then(|id| self.nodes.get(id));
        }
        result.reverse();
        result
    }
}

impl Book {
    pub fn knowledge_index(
        &self,
        dir: &Path,
        publication: Option<&PublishedBookRef>,
    ) -> Result<&Index, ToolError> {
        if self.knowledge_index.get().is_none() {
            let index = Index::build(self, dir, publication)?;
            let _ = self.knowledge_index.set(index);
        }
        Ok(self.knowledge_index.get().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn knowledge_busy_scope_keeps_all_references_and_an_existing_opening() {
        let source = "Section\nFirst passage\nLater passage";
        let mut graphs: Vec<_> = (0..6).map(|i| json!({"id":format!("claim:{i}"),"type":"claim","name":format!("论断 {i}"),"occurrences":[],"source_lid":if i==5 {"1.1"} else {"1.2"}})).collect();
        graphs.extend((0..2).map(|i| json!({"id":format!("concept:{i}"),"type":"concept","name":format!("概念 {i}"),"occurrences":["1.1","1.2"],"source_lid":null})));
        let base = serde_json::from_value(json!({"book_id":"busy","lid_nodes":[
            {"lid":"1","path":[1],"kind":"section","span":{"start":0,"end":source.len()},"children":["1.1","1.2"]},
            {"lid":"1.1","path":[1,1],"kind":"paragraph","span":{"start":8,"end":21},"children":[]},
            {"lid":"1.2","path":[1,2],"kind":"paragraph","span":{"start":22,"end":source.len()},"children":[]}],"graph_nodes":graphs,"graph_edges":[]})).unwrap();
        let discourse = serde_json::from_value(json!([
            {"lid":"1.2","mode":"technical","local_function":"example","local_summary":"后面的例子"},
            {"lid":"1.1","mode":"technical","local_function":"explanation","local_summary":"开篇解释的真实内容"}
        ])).unwrap();
        let book = Book::new(base, source).with_discourse_items(discourse);
        let dir = tempfile::tempdir().unwrap();
        let tree = book.knowledge_index(dir.path(), None).unwrap();
        let scope = &tree.nodes["text:1"];
        assert_eq!(scope.children.len(), 3);
        assert_eq!(
            tree.introduction(&scope.id).unwrap().summary,
            "开篇解释的真实内容"
        );
        let start = tree.entry_point(&scope.id).unwrap();
        assert_eq!(start.reference.entry_id, "claim:5"); // Source order, not the lexical ID.
        assert_eq!(
            tree.nodes[scope.children.first().unwrap()].kind,
            "claims_group"
        );
        for graph in &book.base.graph_nodes {
            let id = identity("pass1", &graph.id, &scope.id);
            let placed = &tree.nodes[&id];
            assert_eq!(tree.placements(&placed.reference).len(), 1);
            assert!(tree.path(&id).iter().any(|n| n.id == scope.id));
            if placed.kind == "concept" {
                assert_eq!(placed.sources.len(), 2);
            }
        }
        assert_eq!(tree.path("text:1.2").last().unwrap().id, scope.id);
        assert!(tree.entry_point(&start.id).is_none());
    }
    #[test]
    fn knowledge_keeps_distinct_claims_and_repeated_concept_locations() {
        let source = "Chapter\nFirst passage.\nSecond passage.";
        let base=serde_json::from_value(json!({"book_id":"tree-book","lid_nodes":[
            {"lid":"1","path":[1],"kind":"chapter","span":{"start":0,"end":source.len()},"children":["1.1","1.2"]},
            {"lid":"1.1","path":[1,1],"kind":"section","span":{"start":8,"end":22},"children":["1.1.1","1.1.2"]},
            {"lid":"1.1.1","path":[1,1,1],"kind":"paragraph","span":{"start":8,"end":16},"children":[]},
            {"lid":"1.1.2","path":[1,1,2],"kind":"image","span":{"start":16,"end":22},"children":[]},
            {"lid":"1.2","path":[1,2],"kind":"section","span":{"start":23,"end":source.len()},"children":["1.2.1"]},
            {"lid":"1.2.1","path":[1,2,1],"kind":"paragraph","span":{"start":23,"end":source.len()},"children":[]}],
            "graph_nodes":[{"id":"concept:time","type":"concept","name":"时间","occurrences":["1.1.1","1.1.2","1.2.1"],"source_lid":null},
                {"id":"claim:a","type":"claim","name":"总时间包含停车","occurrences":[],"source_lid":"1.1.1"},
                {"id":"claim:b","type":"claim","name":"总路程包含往返","occurrences":[],"source_lid":"1.1.1"}],
            "graph_edges":[{"source":"claim:a","target":"concept:time","type":"explains","direction":"directed","scope":"local","weight":1}]})).unwrap();
        let book = Book::new(base, source);
        let directory = tempfile::tempdir().unwrap();
        let object = json!({"source_id":"tree-book","object_id":"time-object"});
        std::fs::write(directory.path().join("formal_objects.json"),json!({"source_id":"tree-book","source_revision":book.source_fingerprint(),
            "objects":[{"ref":object,"object_revision":2}],"active_refs":[object],"candidate_refs":{"concept:time":[object]}}).to_string()).unwrap();
        let budget = book.resident_budget_bytes();
        let tree = book.knowledge_index(directory.path(), None).unwrap();
        assert_eq!(book.resident_budget_bytes(), budget);
        assert!(serde_json::to_vec(&tree.nodes).unwrap().len() as u64 <= reserved_bytes(&book));
        assert_eq!(tree.nodes["text:1.1"].children.len(), 3);
        assert!(!tree.nodes.contains_key("text:1.1.1"));
        assert!(!tree.nodes.contains_key("text:1.1.2"));
        assert_eq!(tree.scope_for_lid("1.1.2"), Some("text:1.1"));
        assert_eq!(tree.path("text:1.1.2").last().unwrap().id, "text:1.1");
        let a = tree
            .nodes
            .values()
            .find(|n| n.reference.entry_id == "claim:a")
            .unwrap();
        assert_eq!(a.parent_id.as_deref(), Some("text:1.1"));
        assert_eq!(a.relations[0].kind, "explains");
        assert!(a.relations[0].available);
        let concept = tree
            .nodes
            .values()
            .find(|n| n.reference.entry_id == "concept:time")
            .unwrap();
        assert_eq!(tree.placements(&concept.reference).len(), 2);
        assert_eq!(
            tree.placements(&concept.reference)
                .iter()
                .find(|n| n.parent_id.as_deref() == Some("text:1.1"))
                .unwrap()
                .sources
                .len(),
            2
        );
        assert_eq!(tree.object_placements("time-object", 2).len(), 2);
        assert!(tree.object_placements("time-object", 1).is_empty());
        let mut stale = concept.reference.clone();
        stale.source_revision = "different".into();
        assert!(tree.placements(&stale).is_empty());
        assert!(std::ptr::eq(
            tree,
            book.knowledge_index(directory.path(), None).unwrap()
        ));
    }
    #[test]
    fn knowledge_music_preserves_two_occurrences_and_score_sources() {
        let directory = tempfile::tempdir().unwrap();
        let fixture = include_str!("../../base-schema/tests/fixtures/music.json");
        let mut m: base_schema::MusicSourceManifest = serde_json::from_str(fixture).unwrap();
        m.events[0].start_ms = 100.;
        m.structure[0].coverage = base_schema::MusicStructureCoverage::NavigationOnly;
        let mut navigation = m.structure[0].clone();
        navigation.section_id = "navigation-opening".into();
        navigation.parent_id = Some("study".into());
        navigation.event_ids.clear();
        m.structure.push(navigation);
        std::fs::write(
            directory.path().join("music_source_manifest.json"),
            serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        let mut base = base_schema::sample_base();
        base.book_id = m.book_id.clone();
        let book = Book::new(base, &"source ".repeat(100));
        let publication = PublishedBookRef {
            book_id: m.book_id.clone(),
            publication_id: "published-study".into(),
        };
        let tree = book
            .knowledge_index(directory.path(), Some(&publication))
            .unwrap();
        assert_eq!(tree.modality, "music");
        assert_eq!(
            tree.nodes[&tree.nodes["music_section:study"].children[0]].kind,
            "music_section"
        );
        assert_eq!(
            tree.entry_point("music_section:study")
                .unwrap()
                .reference
                .artifact,
            "music_event"
        );
        let sources: Vec<_> = tree
            .nodes
            .values()
            .filter(|n| n.reference.artifact == "music_event")
            .flat_map(|n| &n.sources)
            .filter_map(|s| {
                if let KnowledgeSource::Music { target } = s {
                    Some(target)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(sources.len(), 2);
        assert_eq!(
            sources
                .iter()
                .find(|s| s.event_ids[0] == "opening")
                .unwrap()
                .occurrence_ids,
            vec!["first"]
        );
        assert_eq!(
            sources
                .iter()
                .find(|s| s.event_ids[0] == "return")
                .unwrap()
                .occurrence_ids,
            vec!["repeat"]
        );
        assert_ne!(sources[0].audio.start_ms, sources[1].audio.start_ms);
        assert!(sources
            .iter()
            .all(|s| s.publication == publication && s.event_ids.len() == 1));
        assert!(sources
            .iter()
            .find(|s| s.event_ids[0] == "opening")
            .unwrap()
            .score
            .is_some());
        assert!(
            sources
                .iter()
                .find(|s| s.event_ids[0] == "return")
                .unwrap()
                .score
                .is_none(),
            "audio-only event must not invent score evidence"
        );
        assert_eq!(tree.nodes["music_section:study"].children.len(), 3);
    }
    #[test]
    fn knowledge_video_keeps_joint_sources_and_visual_time_range() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../base-schema/fixtures/video-contract.json"
        ))
        .unwrap();
        let f = &fixture["cases"][0];
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, value: &Value| {
            let p = dir.path().join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, serde_json::to_vec(value).unwrap()).unwrap();
        };
        write("video_source_manifest.json", &f["manifest"]);
        write(
            f["manifest"]["transcript_map_path"].as_str().unwrap(),
            &f["map"],
        );
        write(
            f["manifest"]["source"]["imported_path"].as_str().unwrap(),
            &json!({"fixture":true}),
        );
        write(
            f["map"]["raw_result_path"].as_str().unwrap(),
            &json!({"fixture":true}),
        );
        let publication: PublishedBookRef =
            serde_json::from_value(f["questions"][0]["publication"].clone()).unwrap();
        let frame = f["questions"][0]["frame"].clone();
        let range = f["map"]["segments"][0]["location"].clone();
        write(
            "video_enrichments.json",
            &json!({"batches":[{"files":["video-enrichments/test/visual_units.json"]}]}),
        );
        write(
            "video-enrichments/test/visual_units.json",
            &json!({"units":[{"vu_id":"scene-a","location":range,"description":"讲者结合矩阵图解释输入维度","representative_frames":[{"frame":frame}]}]}),
        );
        let mut book = Book::new(
            serde_json::from_value(f["base"].clone()).unwrap(),
            f["source_text"].as_str().unwrap(),
        );
        let members = json!([
            {"target":{"kind":"text","publication":publication,"transcript":f["map"]["transcript"],"text_range":f["map"]["segments"][0]["text_range"]},"support_role":"讲者原话"},
            {"target":{"kind":"visual","publication":publication,"frames":[frame],"regions":[]},"support_role":"矩阵画面"}]);
        book.video_discourse=serde_json::from_value(json!([{"id":"explanation-a","mode":"informative","local_function":"explanation","rhetorical_move":null,
            "local_summary":"输入与输出维度的关系","vu_ids":["scene-a"],"evidence":members,"relations":[]}])).unwrap();
        let tree = book
            .knowledge_index(dir.path(), Some(&publication))
            .unwrap();
        let scene = &tree.nodes["video_unit:scene-a"];
        assert_eq!(scene.children.len(), 1);
        let item = &tree.nodes[&scene.children[0]];
        assert_eq!(item.reference.artifact, "video_discourse");
        assert_eq!(item.sources.len(), 2);
        assert!(
            matches!(&scene.sources[0],KnowledgeSource::Media{start_ms:Some(start),end_ms:Some(end),..} if *start==range["start_ms"].as_f64().unwrap() && *end==range["end_ms"].as_f64().unwrap())
        );
    }
}
