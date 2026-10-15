use super::*;
use crate::tool_result::{project_tool_result, ToolContinuation, ToolResultDraft};
use serde_json::{json, Value};

fn batch_book(pieces: &[String]) -> Book {
    let mut source = String::new();
    let mut leaves = Vec::new();
    for (index, text) in pieces.iter().enumerate() {
        if index > 0 {
            source.push_str("\r\n\r\n");
        }
        let start = source.encode_utf16().count();
        source.push_str(text);
        leaves.push(LidNode {
            lid: format!("1.{}", index + 1),
            path: vec![1, index as u32 + 1],
            kind: NodeKind::Paragraph,
            span: Span {
                start,
                end: source.encode_utf16().count(),
            },
            children: vec![],
        });
    }
    let mut nodes = vec![LidNode {
        lid: "1".into(),
        path: vec![1],
        kind: NodeKind::Chapter,
        span: Span {
            start: 0,
            end: source.encode_utf16().count(),
        },
        children: leaves.iter().map(|node| node.lid.clone()).collect(),
    }];
    nodes.extend(leaves);
    Book::new(
        ReadOnlyBase {
            book_id: "text-batch-test".into(),
            lid_nodes: nodes,
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        &source,
    )
}

fn batch_input() -> Value {
    json!({"ranges": [{"lid":"1.1"}, {"lid":"1.2"}, {"lid":"1.3"}]})
}

fn batch_project(book: &Book, input: &Value, budget: usize) -> (String, ToolResultDraft) {
    let arguments = input.to_string();
    let registry = resident_tool_registry();
    let registration = registry.registration("book.text_batch").unwrap();
    registration.validate_arguments(&arguments).unwrap();
    let raw = crate::book_text_batch::read(&arguments, book);
    let draft = project_tool_result(
        "book.text_batch",
        &arguments,
        &raw,
        crate::tool_registry::ToolOutputPolicy {
            result_policy: crate::tool_registry::ToolResultPolicy::EvidenceProjection,
            max_model_body_bytes: budget,
        },
        budget,
        book,
    );
    (raw, draft)
}

#[test]
fn book_text_batch_noncontiguous_reads_and_small_budget_pages_preserve_exact_evidence() {
    let pieces = [
        "甲😀\"\\needle\n".repeat(400),
        "乙needle小段。".into(),
        "甲😀\"\\needle\n末段。".into(),
    ];
    let book = batch_book(&pieces);
    let input = batch_input();
    let (_, complete) = batch_project(&book, &input, 100_000);
    assert!(complete.continuation.is_none());
    for (index, item) in complete.model_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(item["result"]["text"], pieces[index]);
    }

    let mut input = input;
    let mut restored = vec![String::new(); pieces.len()];
    let mut page_count = 0;
    let mut ledger = TurnEvidenceLedger::default();
    for _ in 0..100 {
        let (raw, draft) = batch_project(&book, &input, 1_000);
        assert!(draft.model_body_json().len() <= 1_000);
        let receipt = tool_receipt(
            "book.text_batch",
            &draft.evidence_arguments,
            &draft.model_body_json(),
            &raw,
            &book,
        );
        let mut expected_intervals = Vec::new();
        for item in draft.model_body["items"].as_array().unwrap() {
            let Some(text) = item["result"]["text"].as_str() else {
                continue;
            };
            let lid = item["arguments"]["lid"].as_str().unwrap();
            let index = lid.strip_prefix("1.").unwrap().parse::<usize>().unwrap() - 1;
            let node = &book.base.lid_nodes[index + 1];
            let start = node.span.start + restored[index].encode_utf16().count();
            restored[index].push_str(text);
            assert!(
                pieces[index].starts_with(&restored[index]),
                "{lid}: source ordering changed"
            );
            expected_intervals.push((start, start + text.encode_utf16().count()));
        }
        assert!(
            !expected_intervals.is_empty(),
            "a continuation must make progress"
        );
        let mut actual_intervals: Vec<_> = receipt
            .accepted_evidence
            .iter()
            .flat_map(|range| evidence_intervals(&book, range).unwrap())
            .collect();
        actual_intervals.sort_unstable();
        expected_intervals.sort_unstable();
        assert_eq!(
            actual_intervals, expected_intervals,
            "receipts must exclude unread text and gaps"
        );
        observe_tool_evidence(
            &mut ledger,
            "book.text_batch",
            &draft.evidence_arguments,
            &draft.model_body_json(),
            &book,
        );
        page_count += 1;
        if page_count == 1 {
            assert_eq!(
                restored[1], pieces[1],
                "the long first item must not starve later ranges"
            );
            assert_eq!(restored[2], pieces[2]);
        }
        match draft.continuation {
            Some(ToolContinuation::NextCall {
                tool, arguments, ..
            }) => {
                assert_eq!(tool, "book.text_batch");
                input = arguments;
            }
            None => break,
            other => panic!("unexpected continuation: {other:?}"),
        }
    }
    assert!(page_count > 2);
    assert_eq!(restored, pieces);
    let expected: Vec<_> = book
        .base
        .lid_nodes
        .iter()
        .skip(1)
        .map(|node| (node.span.start, node.span.end))
        .collect();
    assert_eq!(ledger.observed_intervals(&book), expected);
}

#[test]
fn book_text_batch_zero_budget_preserves_requests_without_accepting_evidence() {
    let book = batch_book(&["甲😀".into(), "乙".into(), "丙".into()]);
    let input = batch_input();
    let (raw, draft) = batch_project(&book, &input, 0);
    assert!(draft.model_body.is_null());
    assert!(draft.truncated);
    let receipt = tool_receipt(
        "book.text_batch",
        &draft.evidence_arguments,
        &draft.model_body_json(),
        &raw,
        &book,
    );
    assert!(receipt.accepted_evidence.is_empty());
    match draft.continuation {
        Some(ToolContinuation::NextCall {
            tool, arguments, ..
        }) => {
            assert_eq!(tool, "book.text_batch");
            assert_eq!(arguments, input);
        }
        other => panic!("all ranges must remain executable: {other:?}"),
    }
}

#[test]
fn book_text_batch_authorizes_every_endpoint_only_after_the_locating_sampling() {
    let book = book_leaves(3);
    let locators = TurnLocatorLedger::from_turn(&book, "请读 1.1 和 1.3", &[], "1.1");
    authorize_book_text_batch(
        &json!({"ranges":[{"lid":"1.1"},{"lid":"1.3"}]}).to_string(),
        &locators,
    )
    .unwrap();
    for input in [
        json!({"ranges":[{"lid":"1.1"},{"lid":"1.2"}]}),
        json!({"ranges":[{"lid":"1.1","end_lid":"1.2"},{"lid":"1.3"}]}),
    ] {
        assert_eq!(
            authorize_book_text_batch(&input.to_string(), &locators)
                .unwrap_err()
                .error_code,
            "LID_PROVENANCE_REQUIRED"
        );
    }

    let book = guided_read_book();
    let arguments = json!({"ranges":[{"lid":"1.1"},{"lid":"1.3"}]}).to_string();
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            turn_calls(vec![
                call("locate", "book.structure", "{}"),
                call("too-early", "book.text_batch", &arguments),
            ]),
            turn_calls(vec![call("after-location", "book.text_batch", &arguments)]),
            turn_final("done"),
        ],
        vec![],
    );
    let mut messages = new_session();
    let outcome = run(
        &book,
        &mut MemoryStore::open(tmp("batch-locator")).unwrap(),
        &mut Reader::new(&book, DEFAULT_RADIUS),
        &adapter,
        &mut messages,
        "已知 LID 1.1 和 1.2，概述这本书的主线",
        "t0",
        OuterConfig::default(),
    )
    .unwrap();
    assert!(!outcome.incomplete, "{:?}", outcome.warning);
    assert!(tool_result(&messages, "too-early").contains("LID_PROVENANCE_REQUIRED"));
    let plans = adapter.seen_plans.borrow();
    let envelope: Value =
        serde_json::from_str(tool_result(&plans[2].input, "after-location")).unwrap();
    assert_ne!(envelope["status"], "error", "{envelope}");
    assert_eq!(envelope["model_body"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        envelope["receipt"]["accepted_evidence"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ComparisonMode {
    SingleSequential,
    SingleCallsOneSampling,
    Batch,
}

impl ComparisonMode {
    fn label(self) -> &'static str {
        match self {
            Self::SingleSequential => "single_sequential",
            Self::SingleCallsOneSampling => "single_calls_one_sampling",
            Self::Batch => "batch",
        }
    }

    fn read_steps(self) -> usize {
        if self == Self::SingleSequential {
            3
        } else {
            1
        }
    }
}

struct BatchComparisonAdapter {
    mode: ComparisonMode,
    pieces: Vec<String>,
    plans: RefCell<Vec<AgentRequestPlan>>,
}

impl ModelAdapter for BatchComparisonAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        panic!("batch comparison uses deterministic reads only")
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let step = self.plans.borrow().len();
        self.plans.borrow_mut().push(plan.clone());
        let read_steps = self.mode.read_steps();
        if step < read_steps {
            return Ok(match self.mode {
                ComparisonMode::Batch => turn_calls(vec![call(
                    "read-batch",
                    "book.text_batch",
                    &batch_input().to_string(),
                )]),
                ComparisonMode::SingleSequential => turn_calls(vec![call(
                    &format!("read-{step}"),
                    "book.text",
                    &json!({"lid":format!("1.{}", step + 1)}).to_string(),
                )]),
                ComparisonMode::SingleCallsOneSampling => turn_calls(
                    (0..3)
                        .map(|index| {
                            call(
                                &format!("read-{index}"),
                                "book.text",
                                &json!({"lid":format!("1.{}", index + 1)}).to_string(),
                            )
                        })
                        .collect(),
                ),
            });
        }
        if step == read_steps {
            if self.mode == ComparisonMode::Batch {
                let envelope: Value =
                    serde_json::from_str(tool_result(&plan.input, "read-batch")).unwrap();
                assert_eq!(
                    envelope["receipt"]["accepted_evidence"]
                        .as_array()
                        .unwrap()
                        .len(),
                    3
                );
                for (index, item) in envelope["model_body"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    assert_eq!(item["result"]["text"], self.pieces[index]);
                }
            } else {
                for (index, text) in self.pieces.iter().enumerate() {
                    let envelope: Value =
                        serde_json::from_str(tool_result(&plan.input, &format!("read-{index}")))
                            .unwrap();
                    assert_eq!(envelope["model_body"]["text"], *text);
                    assert_eq!(
                        envelope["receipt"]["accepted_evidence"]
                            .as_array()
                            .unwrap()
                            .len(),
                        1
                    );
                }
            }
            return Ok(turn_calls(
                self.pieces
                    .iter()
                    .enumerate()
                    .map(|(index, quote)| {
                        call(
                            &format!("present-{index}"),
                            "source.present",
                            &quote_args(quote),
                        )
                    })
                    .collect(),
            ));
        }
        assert_eq!(step, read_steps + 1);
        let mut answer = "Read all three passages.".to_string();
        for index in 0..3 {
            let envelope: Value =
                serde_json::from_str(tool_result(&plan.input, &format!("present-{index}")))
                    .unwrap();
            assert_ne!(envelope["status"], "error", "{envelope}");
            let reference = envelope["model_body"]["source_ref_id"].as_str().unwrap();
            answer.push_str(&format!("[[source:{reference}]]"));
        }
        Ok(turn_final(&answer))
    }
}

#[test]
fn book_text_batch_scripted_run_compares_sequential_single_sampling_and_batch_reads() {
    let pieces = vec![
        "第一段：甲😀。".into(),
        "第二段：乙条件。".into(),
        "第三段：丙结论。".into(),
    ];
    let book = batch_book(&pieces);
    let mut samples = Vec::new();
    for mode in [
        ComparisonMode::SingleSequential,
        ComparisonMode::SingleCallsOneSampling,
        ComparisonMode::Batch,
    ] {
        let adapter = BatchComparisonAdapter {
            mode,
            pieces: pieces.clone(),
            plans: RefCell::new(vec![]),
        };
        let outcome = run(
            &book,
            &mut MemoryStore::open(tmp(&format!("batch-comparison-{}", mode.label()))).unwrap(),
            &mut Reader::new(&book, DEFAULT_RADIUS),
            &adapter,
            &mut new_session(),
            "阅读 LID 1.1、1.2、1.3，并提供原文来源。",
            "t0",
            OuterConfig::default(),
        )
        .unwrap();
        assert!(!outcome.incomplete, "{:?}", outcome.warning);
        assert_eq!(outcome.source_bindings.len(), 3);
        for (index, binding) in outcome.source_bindings.iter().enumerate() {
            assert_eq!(
                book.resolve_source(&binding.evidence_range, SOURCE_PRESENTATION_LOCALE, None)
                    .unwrap()
                    .highlighted_quote,
                pieces[index]
            );
        }
        let plans = adapter.plans.borrow();
        assert_eq!(plans.len(), mode.read_steps() + 2);
        let request_bytes: usize = plans
            .iter()
            .map(|plan| {
                crate::native_chat_request_projection("test", plan)
                    .0
                    .to_string()
                    .len()
            })
            .sum();
        samples.push(json!({"mode":mode.label(),
            "measurement":"scripted deterministic request projection; not provider token usage",
            "samplings":plans.len(), "native_request_bytes":request_bytes,
            "source_count":outcome.source_bindings.len()}));
    }
    if let Ok(path) = std::env::var("BOOK_TEXT_BATCH_COMPARISON") {
        std::fs::write(path, serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    }
}

#[test]
fn book_text_batch_cannot_bypass_experimental_unique_source_budget() {
    let book = batch_book(&["甲".repeat(6_000), "乙".repeat(6_000), "丙".into()]);
    let input = batch_input().to_string();
    let mut budget = crate::experiment::BodyBudget::default();
    let admitted: Value = serde_json::from_str(&budget.admit(
        "book.text_batch",
        &input,
        crate::book_text_batch::read(&input, &book),
        &book,
    ))
    .unwrap();
    assert_eq!(
        admitted["items"][0]["result"]["text"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        6_000
    );
    assert_eq!(
        admitted["items"][1]["result"]["text"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        6_000
    );
    assert_eq!(
        admitted["items"][2]["result"]["error_code"],
        "EXPERIMENT_BODY_BUDGET"
    );
    let receipt = tool_receipt(
        "book.text_batch",
        &input,
        &admitted.to_string(),
        &admitted.to_string(),
        &book,
    );
    assert_eq!(receipt.accepted_evidence.len(), 2);
    assert!(receipt
        .accepted_evidence
        .iter()
        .all(|range| range.start_lid != "1.3"));
    let retry = json!({"ranges":[{"lid":"1.1"},{"lid":"1.3"}]}).to_string();
    let admitted: Value = serde_json::from_str(&budget.admit(
        "book.text_batch",
        &retry,
        crate::book_text_batch::read(&retry, &book),
        &book,
    ))
    .unwrap();
    assert!(
        admitted["items"][0]["result"]["text"].is_string(),
        "already charged intervals remain readable"
    );
    assert_eq!(
        admitted["items"][1]["result"]["error_code"],
        "EXPERIMENT_BODY_BUDGET"
    );
}

#[test]
fn book_text_batch_experimental_budget_charges_delivered_pages_and_cancels_rejected_continuations()
{
    let book = batch_book(&["甲".repeat(6_000), "乙".repeat(6_000), "丙".repeat(6_000)]);
    let mut input = batch_input();
    let mut budget = crate::experiment::BodyBudget::default();
    let mut ledger = TurnEvidenceLedger::default();
    let mut rejected = false;
    for page in 0..20 {
        let (raw, mut draft) = batch_project(&book, &input, 16 * 1024);
        let before = draft.model_body.clone();
        budget.admit_projection("book.text_batch", &mut draft, &book);
        let mut delivered_intervals = Vec::new();
        for (old, item) in before["items"]
            .as_array()
            .unwrap()
            .iter()
            .zip(draft.model_body["items"].as_array().unwrap())
        {
            if item["result"]["error_code"] == "EXPERIMENT_BODY_BUDGET" {
                rejected = true;
                assert!(item["result"].get("text").is_none());
                assert_eq!(draft.status, crate::tool_result::ToolResultStatus::Partial);
                if let Some(ToolContinuation::NextCall { arguments, .. }) = &draft.continuation {
                    let mut forbidden = old["arguments"].clone();
                    forbidden["start_utf16"] = old["result"]["next_start_utf16"].clone();
                    assert!(
                        !arguments["ranges"].as_array().unwrap().contains(&forbidden),
                        "a rejected page must not advance past text the model never read"
                    );
                }
            } else if let Some(text) = item["result"]["text"].as_str() {
                let start = item["result"]["source_range"]["start"]
                    .as_u64()
                    .or_else(|| item["arguments"]["start_utf16"].as_u64())
                    .unwrap_or_else(|| {
                        book.base
                            .lid_nodes
                            .iter()
                            .find(|node| node.lid == item["arguments"]["lid"].as_str().unwrap())
                            .unwrap()
                            .span
                            .start as u64
                    }) as usize;
                delivered_intervals.push((start, start + text.encode_utf16().count()));
            }
        }
        if page == 0 {
            assert!(
                !rejected,
                "three bounded first pages fit; their full node spans do not count"
            );
            assert_eq!(delivered_intervals.len(), 3);
        }
        let receipt = tool_receipt(
            "book.text_batch",
            &draft.evidence_arguments,
            &draft.model_body_json(),
            &raw,
            &book,
        );
        let mut receipt_intervals: Vec<_> = receipt
            .accepted_evidence
            .iter()
            .flat_map(|range| evidence_intervals(&book, range).unwrap())
            .collect();
        receipt_intervals.sort_unstable();
        delivered_intervals.sort_unstable();
        assert_eq!(receipt_intervals, delivered_intervals);
        observe_tool_evidence(
            &mut ledger,
            "book.text_batch",
            &draft.evidence_arguments,
            &draft.model_body_json(),
            &book,
        );
        let used: usize = ledger
            .observed_intervals(&book)
            .iter()
            .map(|(start, end)| end - start)
            .sum();
        assert!(used <= 12_000);
        match draft.continuation {
            Some(ToolContinuation::NextCall { arguments, .. }) => input = arguments,
            None => break,
            other => panic!("unexpected continuation: {other:?}"),
        }
    }
    assert!(
        rejected,
        "reading all 18,000 source units must reach the 12,000-unit ceiling"
    );
    assert!(
        ledger
            .observed_intervals(&book)
            .iter()
            .map(|(start, end)| end - start)
            .sum::<usize>()
            > 6_000
    );
}

#[test]
fn book_text_batch_experimental_rejection_keeps_other_offsets_of_the_same_lid() {
    let book = batch_book(&["甲".repeat(18_000)]);
    let mut budget = crate::experiment::BodyBudget::default();
    let admitted = budget.admit(
        "book.text",
        r#"{"lid":"1.1"}"#,
        json!({"lid":"1.1", "text":"甲".repeat(12_000),
            "source_range":{"start":0,"end":12_000}})
        .to_string(),
        &book,
    );
    assert!(serde_json::from_str::<Value>(&admitted).unwrap()["text"].is_string());

    let input = json!({"ranges":[{"lid":"1.1","start_utf16":0},
        {"lid":"1.1","start_utf16":12_000}]});
    let (raw, mut draft) = batch_project(&book, &input, 1_000);
    let mut preserved = draft.model_body["items"][0]["arguments"].clone();
    preserved["start_utf16"] = draft.model_body["items"][0]["result"]["next_start_utf16"].clone();
    budget.admit_projection("book.text_batch", &mut draft, &book);
    assert!(draft.model_body["items"][0]["result"]["text"].is_string());
    assert_eq!(
        draft.model_body["items"][1]["result"]["error_code"],
        "EXPERIMENT_BODY_BUDGET"
    );
    match &draft.continuation {
        Some(ToolContinuation::NextCall { arguments, .. }) => {
            assert_eq!(arguments, &json!({"ranges":[preserved]}))
        }
        other => panic!("already admitted same-LID range must remain readable: {other:?}"),
    }
    let receipt = tool_receipt(
        "book.text_batch",
        &draft.evidence_arguments,
        &draft.model_body_json(),
        &raw,
        &book,
    );
    assert_eq!(receipt.accepted_evidence.len(), 1);
    assert!(receipt
        .accepted_evidence
        .iter()
        .flat_map(|range| evidence_intervals(&book, range).unwrap())
        .all(|(_, end)| end <= 12_000));

    let arguments = json!({"lid":"1.1","start_utf16":12_000}).to_string();
    let input: book_tool_contracts::TextInput = serde_json::from_str(&arguments).unwrap();
    let mut single = project_tool_result(
        "book.text",
        &arguments,
        &book.read_text(&input).unwrap().to_string(),
        crate::tool_registry::ToolOutputPolicy {
            result_policy: crate::tool_registry::ToolResultPolicy::EvidenceProjection,
            max_model_body_bytes: 1_000,
        },
        1_000,
        &book,
    );
    assert!(single.continuation.is_some());
    budget.admit_projection("book.text", &mut single, &book);
    assert_eq!(single.status, crate::tool_result::ToolResultStatus::Error);
    assert!(single.truncated);
    assert!(single.continuation.is_none());
    assert_eq!(single.model_body["error_code"], "EXPERIMENT_BODY_BUDGET");
}
