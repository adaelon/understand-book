//! Lazy, local lexical retrieval. Hits locate candidate passages; only a subsequent
//! source read supplies evidence. This index never changes canonical source or LIDs.

use super::{Book, BookTextCall, HeadingPathItem, TextOccurrenceRange, ToolError};
use base_schema::Span;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

const WINDOW_UTF16: usize = 1_200;
const OVERLAP_UTF16: usize = 160;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, schemars::JsonSchema)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct RankedSearchResult {
    pub version: String,
    pub query: String,
    /// Number of lexically matching windows in the selected scope, not a claim
    /// that all passages with related meaning have been found.
    pub total_candidates: usize,
    pub hits: Vec<RankedSearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, schemars::JsonSchema)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct RankedSearchHit {
    pub rank: usize,
    pub score: f64,
    pub start_lid: String,
    pub end_lid: String,
    pub source_range_utf16: Span,
    pub ranges: Vec<TextOccurrenceRange>,
    pub heading_path: Vec<HeadingPathItem>,
    pub excerpt: String,
    pub matched_terms: Vec<String>,
    pub next_calls: Vec<BookTextCall>,
}

struct Chunk {
    node_index: usize,
    span: Span,
    length: usize,
}

struct Posting {
    chunk: usize,
    frequency: usize,
}

pub(super) struct Index {
    chunks: Box<[Chunk]>,
    postings: HashMap<Box<str>, Box<[Posting]>>,
    average_length: f64,
}

pub(super) struct IndexError {
    code: String,
    category: String,
    message: String,
}

impl From<ToolError> for IndexError {
    fn from(error: ToolError) -> Self {
        Self {
            code: error.error_code,
            category: error.category,
            message: error.message,
        }
    }
}

impl IndexError {
    fn to_tool_error(&self) -> ToolError {
        ToolError {
            error_code: self.code.clone(),
            category: self.category.clone(),
            message: self.message.clone(),
        }
    }
}

/// Admission reserves the lazy index before its first query. The estimate uses
/// two postings per CJK character (32 bytes) plus dictionary/window overhead;
/// other scripts usually produce one word per several source units. It is a
/// practical residency estimate, not the worst case for arbitrary unique text.
/// It does not shrink after building or depend on query history.
pub(super) fn reserved_bytes(book: &Book) -> u64 {
    let text_allowance =
        char::decode_utf16(book.source_u16.iter().copied()).fold(0u64, |bytes, ch| {
            let allowance = match ch {
                Ok(ch) if is_cjk(ch) => 48,
                Ok(ch) => 12 * ch.len_utf16() as u64,
                Err(_) => 12,
            };
            bytes.saturating_add(allowance)
        });
    text_allowance.saturating_add((book.base.lid_nodes.len() as u64).saturating_mul(128))
}

impl Book {
    /// Rank lexical passage candidates using BM25. English words and CJK
    /// character bigrams support natural questions without a model or embedding
    /// service. This does not assert semantic completeness or source observation.
    pub fn search_ranked(
        &self,
        query: &str,
        within_lid: Option<&str>,
        limit: usize,
    ) -> Result<RankedSearchResult, ToolError> {
        if query.trim().is_empty() {
            return Err(super::search_error(
                "SEARCH_QUERY_EMPTY",
                "search query must contain non-whitespace text",
            ));
        }
        if query.chars().count() > 4096 {
            return Err(super::search_error(
                "SEARCH_QUERY_TOO_LONG",
                "search query must contain at most 4096 Unicode scalar values",
            ));
        }
        if !(1..=50).contains(&limit) {
            return Err(super::search_error(
                "BOOK_TOOL_INPUT_INVALID",
                "limit must be between 1 and 50",
            ));
        }
        let scope = within_lid
            .map(|lid| {
                self.node(lid).map_err(|_| {
                    super::search_error(
                        "SEARCH_SCOPE_INVALID",
                        format!("within_lid does not exist: {lid}"),
                    )
                })
            })
            .transpose()?;
        let mut terms: Vec<_> = term_frequencies(query).into_keys().collect();
        terms.sort();
        let mut result = RankedSearchResult {
            version: "search_ranked.v1".into(),
            query: query.into(),
            total_candidates: 0,
            hits: Vec::new(),
        };
        if terms.is_empty() {
            return Ok(result);
        }
        let index = self
            .ranked_search_index
            .get_or_init(|| Index::build(self).map_err(Into::into))
            .as_ref()
            .map_err(IndexError::to_tool_error)?;
        // Only posting lists for actual query terms are visited on warm queries.
        let mut scores: HashMap<usize, f64> = HashMap::new();
        for term in &terms {
            let Some(postings) = index.postings.get(term.as_str()) else {
                continue;
            };
            let idf = (1.0
                + (index.chunks.len() as f64 - postings.len() as f64 + 0.5)
                    / (postings.len() as f64 + 0.5))
                .ln();
            // Single CJK characters are useful fallback matches but must not
            // overwhelm specific bigrams with incidental shared characters.
            let weight = if term.chars().count() == 1 && term.chars().next().is_some_and(is_cjk) {
                0.15
            } else {
                1.0
            };
            for posting in postings {
                let chunk = &index.chunks[posting.chunk];
                let node = &self.base.lid_nodes[chunk.node_index];
                if scope.is_some_and(|scope| !super::path_is_prefix(&scope.path, &node.path)) {
                    continue;
                }
                let tf = posting.frequency as f64;
                let length_norm = 1.2 * (0.25 + 0.75 * chunk.length as f64 / index.average_length);
                *scores.entry(posting.chunk).or_default() +=
                    weight * idf * tf * 2.2 / (tf + length_norm);
            }
        }
        result.total_candidates = scores.len();
        let mut candidates: Vec<_> = scores.into_iter().collect();
        candidates.sort_unstable_by(|(left, left_score), (right, right_score)| {
            right_score.total_cmp(left_score).then_with(|| {
                index.chunks[*left]
                    .span
                    .start
                    .cmp(&index.chunks[*right].span.start)
            })
        });
        for (chunk_index, score) in candidates {
            let chunk = &index.chunks[chunk_index];
            let node = &self.base.lid_nodes[chunk.node_index];
            let excerpt = String::from_utf16(&self.source_u16[chunk.span.start..chunk.span.end])
                .map_err(|_| {
                    super::search_error(
                        "SEARCH_SOURCE_INVALID",
                        "retrieval window is not valid UTF-16",
                    )
                })?;
            let frequencies = term_frequencies(&excerpt);
            let matched_terms: Vec<_> = terms
                .iter()
                .filter(|term| frequencies.contains_key(*term))
                .cloned()
                .collect();
            // A boundary occurrence can be present in two adjacent windows. Drop
            // the duplicate only when all of this window's query occurrences
            // are already in their shared source interval. Separate occurrences
            // elsewhere in the same leaf remain discoverable.
            let duplicate = result.hits.iter().any(|kept| {
                if kept.start_lid != node.lid {
                    return false;
                }
                let start = kept.source_range_utf16.start.max(chunk.span.start);
                let end = kept.source_range_utf16.end.min(chunk.span.end);
                if start >= end {
                    return false;
                }
                let overlap =
                    term_frequencies(&String::from_utf16_lossy(&self.source_u16[start..end]));
                matched_terms
                    .iter()
                    .all(|term| overlap.get(term) == frequencies.get(term))
            });
            if duplicate {
                continue;
            }
            let mut next_call = BookTextCall::new(&node.lid, None);
            // book.text continuation offsets are absolute source UTF-16 positions.
            // Explicit paging also prevents a large leaf from being read in full.
            next_call.arguments.start_utf16 = Some(chunk.span.start);
            result.hits.push(RankedSearchHit {
                rank: result.hits.len() + 1,
                score,
                start_lid: node.lid.clone(),
                end_lid: node.lid.clone(),
                source_range_utf16: chunk.span.clone(),
                ranges: vec![TextOccurrenceRange {
                    lid: node.lid.clone(),
                    start_utf16: chunk.span.start - node.span.start,
                    end_utf16: chunk.span.end - node.span.start,
                }],
                heading_path: self.source_heading_path_items(node)?,
                excerpt,
                matched_terms,
                next_calls: vec![next_call],
            });
            if result.hits.len() == limit {
                break;
            }
        }
        Ok(result)
    }
}

impl Index {
    fn build(book: &Book) -> Result<Self, ToolError> {
        let leaves = book.validated_search_leaves()?;
        let mut chunks = Vec::new();
        let mut postings: HashMap<String, Vec<Posting>> = HashMap::new();
        let mut total_length = 0usize;
        for leaf in leaves {
            let node_index = book.lid_idx[&leaf.lid];
            let mut start = leaf.span.start;
            while start < leaf.span.end {
                let mut end = start.saturating_add(WINDOW_UTF16).min(leaf.span.end);
                if !super::is_utf16_boundary(&book.source_u16, end) {
                    end -= 1;
                }
                let text = String::from_utf16(&book.source_u16[start..end]).map_err(|_| {
                    super::search_error(
                        "SEARCH_SOURCE_INVALID",
                        "canonical source is not valid UTF-16",
                    )
                })?;
                let frequencies = term_frequencies(&text);
                let length = frequencies.values().sum();
                if length > 0 {
                    let chunk = chunks.len();
                    for (term, frequency) in frequencies {
                        postings
                            .entry(term)
                            .or_default()
                            .push(Posting { chunk, frequency });
                    }
                    chunks.push(Chunk {
                        node_index,
                        span: Span { start, end },
                        length,
                    });
                    total_length += length;
                }
                if end == leaf.span.end {
                    break;
                }
                start = end - OVERLAP_UTF16;
                if !super::is_utf16_boundary(&book.source_u16, start) {
                    start += 1;
                }
            }
        }
        let average_length = total_length as f64 / chunks.len().max(1) as f64;
        Ok(Self {
            chunks: chunks.into_boxed_slice(),
            postings: postings
                .into_iter()
                .map(|(term, entries)| (term.into_boxed_str(), entries.into_boxed_slice()))
                .collect(),
            average_length,
        })
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF
        | 0x3040..=0x30FF | 0xAC00..=0xD7AF)
}

fn stop_word(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "can"
            | "do"
            | "does"
            | "for"
            | "from"
            | "how"
            | "i"
            | "in"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "was"
            | "what"
            | "when"
            | "where"
            | "which"
            | "why"
            | "will"
            | "with"
            | "you"
            | "your"
            | "的"
            | "了"
            | "是"
            | "在"
            | "和"
            | "与"
            | "呢"
            | "吗"
            | "我"
            | "你"
            | "他"
            | "她"
            | "它"
            | "这"
            | "那"
            | "有"
            | "个"
            | "么"
            | "什么"
            | "怎么"
            | "如何"
            | "为何"
            | "为什么"
            | "请问"
            | "一下"
            | "这个"
            | "那个"
    )
}

fn term_frequencies(text: &str) -> HashMap<String, usize> {
    let normalized: String = text.nfkc().case_fold().collect();
    let mut frequencies = HashMap::new();
    let mut word = String::new();
    let mut previous_cjk = None;
    let add = |term: String, frequencies: &mut HashMap<String, usize>| {
        if !term.is_empty() && !stop_word(&term) {
            *frequencies.entry(term).or_default() += 1;
        }
    };
    for ch in normalized.chars() {
        if is_cjk(ch) {
            add(std::mem::take(&mut word), &mut frequencies);
            add(ch.to_string(), &mut frequencies);
            if let Some(previous) = previous_cjk {
                add([previous, ch].iter().collect(), &mut frequencies);
            }
            previous_cjk = Some(ch);
        } else {
            previous_cjk = None;
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
            } else {
                add(std::mem::take(&mut word), &mut frequencies);
            }
        }
    }
    add(word, &mut frequencies);
    frequencies
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_schema::{LidNode, NodeKind, ReadOnlyBase};

    fn book(parts: &[&str]) -> Book {
        let mut start = 0;
        let nodes = parts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let end = start + text.encode_utf16().count();
                let node = LidNode {
                    lid: format!("1.{}", index + 1),
                    path: vec![1, (index + 1) as u32],
                    kind: NodeKind::Paragraph,
                    span: Span { start, end },
                    children: vec![],
                };
                start = end;
                node
            })
            .collect::<Vec<_>>();
        let root = LidNode {
            lid: "1".into(),
            path: vec![1],
            kind: NodeKind::Chapter,
            span: Span {
                start: 0,
                end: start,
            },
            children: nodes.iter().map(|node| node.lid.clone()).collect(),
        };
        Book::new(
            ReadOnlyBase {
                book_id: "ranked".into(),
                lid_nodes: [vec![root], nodes].concat(),
                graph_nodes: vec![],
                graph_edges: vec![],
            },
            &parts.concat(),
        )
    }

    #[test]
    fn ranked_chinese_natural_question_locates_relevant_passage() {
        let book = book(&[
            "数据集划分为训练集、验证集和测试集。\n",
            "正则化能够缓解过拟合，提高模型在未见数据上的泛化能力。\n",
            "缓存可以减少数据库读取次数。\n",
        ]);
        let result = book
            .search_ranked("为什么正则化能够缓解过拟合？", None, 5)
            .unwrap();
        assert_eq!(result.hits[0].start_lid, "1.2");
        assert!(result.hits[0].matched_terms.contains(&"拟合".to_owned()));
        assert!(result.hits[0].score > 0.0);
    }

    #[test]
    fn ranked_english_reorders_candidates_and_normalizes_case() {
        let book = book(&[
            "Database reads are discussed with storage latency and network communication.\n",
            "Caching reduces repeated database reads. Caching also reduces database latency.\n",
            "Photosynthesis converts light into chemical energy.\n",
        ]);
        let result = book
            .search_ranked("How does CACHING reduce DATABASE reads?", None, 2)
            .unwrap();
        assert_eq!(result.hits[0].start_lid, "1.2");
        assert_eq!(result.total_candidates, 2);
        assert_eq!(result.hits[1].start_lid, "1.1");
        assert!(result.hits[0].score > result.hits[1].score);
    }

    #[test]
    fn ranked_scope_filters_other_leaves_and_rejects_missing_scope() {
        let book = book(&[
            "Caching lowers latency.\n",
            "Caching speeds database reads.\n",
        ]);
        let result = book.search_ranked("caching", Some("1.1"), 5).unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].start_lid, "1.1");
        assert_eq!(
            book.search_ranked("caching", Some("1"), 5)
                .unwrap()
                .hits
                .len(),
            2
        );
        assert_eq!(
            book.search_ranked("caching", Some("missing"), 5)
                .unwrap_err()
                .error_code,
            "SEARCH_SCOPE_INVALID"
        );
    }

    #[test]
    fn ranked_long_leaf_middle_hit_has_readable_absolute_utf16_locator() {
        let prefix = "前置来源😀\n";
        let long = format!(
            "{} needle_unique 缓存🦀 {}",
            "背景😀 ".repeat(4_000),
            "尾声🦀 ".repeat(4_000)
        );
        let book = book(&[prefix, &long]);
        let result = book.search_ranked("needle_unique", None, 5).unwrap();
        assert!(!result.hits.is_empty());
        for hit in &result.hits {
            assert_eq!(hit.start_lid, "1.2");
            assert!(hit.source_range_utf16.start > prefix.encode_utf16().count() + 8_000);
            assert!(hit.excerpt.contains("needle_unique"));
            assert!(!hit.excerpt.contains('\u{fffd}'));
            assert_eq!(
                hit.source_range_utf16.start,
                prefix.encode_utf16().count() + hit.ranges[0].start_utf16
            );
            assert_eq!(
                book.text_in_source_range(&hit.source_range_utf16).unwrap(),
                hit.excerpt
            );
            let call = &hit.next_calls[0];
            assert_eq!(call.tool, "book.text");
            assert_eq!(
                call.arguments.start_utf16,
                Some(hit.source_range_utf16.start)
            );
            let read = book.read_text(&call.arguments).unwrap();
            assert!(read["text"].as_str().unwrap().contains("needle_unique"));
        }
    }

    #[test]
    fn ranked_overlap_deduplicates_one_occurrence_but_keeps_distant_occurrences() {
        let text = format!(
            "{}needle_unique{} needle_unique{}",
            "x ".repeat(540),
            " y".repeat(1_000),
            " z".repeat(1_000)
        );
        let book = book(&[&text]);
        let result = book.search_ranked("needle_unique", None, 10).unwrap();
        assert!(
            result.total_candidates > 2,
            "one occurrence must straddle a window overlap"
        );
        assert_eq!(result.hits.len(), 2);
        let first = &result.hits[0].source_range_utf16;
        let second = &result.hits[1].source_range_utf16;
        assert!(first.end <= second.start || second.end <= first.start);
    }

    /// Run explicitly to inspect cold-index cost versus warm retrieval without
    /// making machine-dependent latency an automated pass/fail criterion.
    #[test]
    #[ignore]
    fn ranked_retrieval_cost_sample() {
        let parts: Vec<_> = (0..4_000).map(|n| {
            format!("Section {n}: 缓存可以减少数据库读取次数，改善响应延迟。Caching reduces repeated database reads and storage latency. topic_{n}\n")
        }).collect();
        let parts: Vec<_> = parts.iter().map(String::as_str).collect();
        let book = book(&parts);
        let started = std::time::Instant::now();
        assert_eq!(
            book.search_ranked("topic_2100", None, 5).unwrap().hits[0].start_lid,
            "1.2101"
        );
        let cold = started.elapsed();
        let index = book
            .ranked_search_index
            .get()
            .unwrap()
            .as_ref()
            .ok()
            .unwrap();
        let allocated = index.chunks.len() * std::mem::size_of::<Chunk>()
            + index.postings.capacity() * (std::mem::size_of::<(Box<str>, Box<[Posting]>)>() + 1)
            + index
                .postings
                .iter()
                .map(|(term, entries)| term.len() + entries.len() * std::mem::size_of::<Posting>())
                .sum::<usize>();
        assert!((allocated as u64) <= reserved_bytes(&book));
        let mut samples = Vec::new();
        for _ in 0..20 {
            let started = std::time::Instant::now();
            assert_eq!(
                book.search_ranked("topic_2100", None, 5).unwrap().hits[0].start_lid,
                "1.2101"
            );
            samples.push(started.elapsed().as_micros());
        }
        samples.sort();
        let mut common_samples = Vec::new();
        for _ in 0..20 {
            let started = std::time::Instant::now();
            let result = book.search_ranked("缓存 database", None, 5).unwrap();
            assert_eq!(result.total_candidates, 4_000);
            assert_eq!(result.hits.len(), 5);
            common_samples.push(started.elapsed().as_micros());
        }
        common_samples.sort();
        eprintln!("ranked sample: leaves=4000 source_utf16={} cold_ms={} warm_us_min={} median={} max={} index_payload_bytes={} reserved_bytes={}",
            book.source_u16.len(), cold.as_millis(), samples[0], samples[10], samples[19], allocated, reserved_bytes(&book));
        eprintln!(
            "ranked common query: candidates=4000 warm_us_min={} median={} max={}",
            common_samples[0], common_samples[10], common_samples[19]
        );
    }

    #[test]
    fn ranked_empty_candidates_do_not_claim_semantic_absence() {
        let book = book(&["缓存可以减少数据库读取次数。\n"]);
        let result = book
            .search_ranked("How can repeated storage work be avoided?", None, 5)
            .unwrap();
        assert_eq!(result.total_candidates, 0);
        assert!(result.hits.is_empty());
        let value = serde_json::to_value(result).unwrap();
        assert!(value.get("exhaustive").is_none());
        assert!(value.get("not_found").is_none());
    }

    #[test]
    fn ranked_index_is_shared_across_arc_queries_and_budgeted_before_build() {
        let book = std::sync::Arc::new(book(&["Cache cache repeated reads.\n"]));
        let budget = book.resident_budget_bytes();
        assert!(book.ranked_search_index.get().is_none());
        let other = book.clone();
        other.search_ranked("cache", None, 5).unwrap();
        assert!(book.ranked_search_index.get().is_some());
        assert_eq!(book.resident_budget_bytes(), budget);
        assert_eq!(
            book.search_ranked("cache", None, 5).unwrap(),
            other.search_ranked("cache", None, 5).unwrap()
        );
    }

    #[test]
    fn ranked_input_limits_and_tokenless_query() {
        let book = book(&["some text"]);
        assert_eq!(
            book.search_ranked(" ", None, 5).unwrap_err().error_code,
            "SEARCH_QUERY_EMPTY"
        );
        assert_eq!(
            book.search_ranked("text", None, 0).unwrap_err().error_code,
            "BOOK_TOOL_INPUT_INVALID"
        );
        assert_eq!(
            book.search_ranked(&"a".repeat(4097), None, 5)
                .unwrap_err()
                .error_code,
            "SEARCH_QUERY_TOO_LONG"
        );
        assert!(book.search_ranked("?!", None, 5).unwrap().hits.is_empty());
        assert!(book.ranked_search_index.get().is_none());
    }
}
