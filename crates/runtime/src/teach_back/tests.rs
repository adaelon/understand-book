use super::*;
use base_schema::{LidNode, NodeKind, ReadOnlyBase, Span};

const EXPRESSION: &str = "容量大一倍，读得快一倍。";
const ANCHOR: &str = "1.4";

fn run(
    book: &Book,
    anchors: &[String],
    question: Option<&str>,
    turn: &str,
    expression: &str,
    progress: impl FnMut(TeachBackStage),
    sample: impl FnMut(TeachBackStage, &str, &Value) -> Result<String, ToolError>,
) -> Result<Option<TeachBackComparison>, ToolError> {
    match super::run(
        book, anchors, question, None, turn, expression, None, progress, sample,
    )? {
        TeachBackResult::Comparison(card) => Ok(Some(card)),
        TeachBackResult::OrdinaryChat => Ok(None),
        TeachBackResult::NeedsRevision => panic!("initial expression cannot be a dispute"),
    }
}

fn book() -> Book {
    let pieces = [
        "容量决定数据能否放下。",
        "旁路是减少读取量。",
        "读取量保持不变。",
        "🛶带宽翻倍，读取下界减半。",
        "计算时间保持不变。",
        "本例受读取限制。",
        "还要校准实测。",
    ];
    let mut text = String::new();
    let mut leaves = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let start = text.encode_utf16().count();
        text.push_str(piece);
        leaves.push(LidNode {
            lid: format!("1.{}", index + 1),
            path: vec![1, index as u32 + 1],
            kind: NodeKind::Paragraph,
            children: vec![],
            span: Span {
                start,
                end: text.encode_utf16().count(),
            },
        });
    }
    let section = LidNode {
        lid: "1".into(),
        path: vec![1],
        kind: NodeKind::Section,
        children: leaves.iter().map(|n| n.lid.clone()).collect(),
        span: Span {
            start: 0,
            end: text.encode_utf16().count(),
        },
    };
    leaves.insert(0, section);
    Book::new(
        ReadOnlyBase {
            book_id: "memory-bandwidth".into(),
            lid_nodes: leaves,
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        &text,
    )
}

fn student_explanation(premise: Option<&str>, conclusion: &str, quote: &str) -> Value {
    let graph = match premise {
        Some(reason) => json!({
            "nodes":[{"id":"sn1","label":reason},{"id":"sn2","label":conclusion}],
            "edges":[{"id":"s","from":"sn1","to":"sn2","relation_type":"causes","quotes":[quote]}]
        }),
        None => json!({"nodes":[{"id":"sn2","label":conclusion,"quotes":[quote]}],"edges":[]}),
    };
    json!({"kind":"explanation","student_graph":graph})
}

fn expression() -> Value {
    student_explanation(Some("容量变大"), "读取加快", EXPRESSION)
}

fn comparison(input: &Value) -> Value {
    let evidence = &input["evidence"];
    json!({"source":{"source_id":evidence["source_id"],"source_revision":evidence["source_revision"]},
        "book_graph":{
            "nodes":[{"id":"bn1","label":"带宽翻倍"},{"id":"bn2","label":"读取下界减半"}],
            "edges":[{"id":"b","from":"bn1","to":"bn2","condition":"读取量不变",
                "relation_type":"causes","evidence":[
                    {"lid":"1.3","quote":"读取量保持不变。"},{"lid":ANCHOR,"quote":"🛶带宽翻倍，读取下界减半。"}]}]
        },
        "rows":[{"row_id":"r","student_edge_ids":["s"],"book_edge_ids":["b"],"status":"divergent",
            "summary":"加容量与加带宽改变的量不同。","divergence_condition":"带宽不变，只加容量"}],
        "side_notes":[],"follow_up":{"row_id":"r","question":"只加容量会提高带宽吗？"}})
}

#[test]
fn course_task_conditions_only_reach_the_book_comparison() {
    let task =
        json!({"saved_exploration":{"target_ms":9}, "displayed_explanation":"课程给出的运算条件"});
    let card = super::run(
        &book(),
        &[ANCHOR.into()],
        Some("这项改动能达到课程目标吗？"),
        Some(&task),
        "course-turn",
        EXPRESSION,
        None,
        |_| {},
        |stage, _, input| {
            Ok(match stage {
                TeachBackStage::OrganizeExpression => {
                    assert_eq!(input.as_object().unwrap().len(), 3);
                    assert!(!input.to_string().contains("课程给出的运算条件"));
                    expression()
                }
                TeachBackStage::ComparePassages => {
                    assert_eq!(input["visible_course_task"], task);
                    comparison(input)
                }
            }
            .to_string())
        },
    )
    .unwrap();
    assert!(matches!(card, TeachBackResult::Comparison(_)));
}

#[test]
fn empty_draft_annotation_leaves_the_repair_for_a_quote_copied_from_the_wrong_lid() {
    let mut calls = 0;
    let card = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "turn",
        EXPRESSION,
        |_| {},
        |stage, _, packet| {
            calls += 1;
            let output = match stage {
                TeachBackStage::OrganizeExpression => {
                    let mut draft = expression();
                    draft["student_graph"]["edges"][0]["relation_type_note"] = Value::Null;
                    draft
                }
                TeachBackStage::ComparePassages if packet.get("validation_error").is_none() => {
                    let mut draft = comparison(packet);
                    draft["book_graph"]["edges"][0]["evidence"][0]["lid"] = json!(ANCHOR);
                    draft
                }
                TeachBackStage::ComparePassages => {
                    assert!(packet["validation_error"]
                        .as_str()
                        .unwrap()
                        .contains("under LID 1.3"));
                    comparison(&packet["input"])
                }
            };
            Ok(output.to_string())
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(calls, 3);
    assert_eq!(card.book_graph.edges[0].evidence[0].lid, "1.3");
}

#[test]
fn quote_repair_offers_exact_visible_wording_for_a_misquoted_number() {
    let aligned = "矩阵计算时间从约 0.1415 ms 变为 0.0707 ms。";
    let text = format!("🛶计算与读取。{aligned}");
    let range = Span {
        start: 10,
        end: 10 + text.encode_utf16().count(),
    };
    let page = TeachBackPassage {
        lid: ANCHOR.into(),
        source_range_utf16: range,
        text: text + "下一页。",
        next_start_utf16: None,
    };
    let quote = TeachBackBookQuote {
        lid: ANCHOR.into(),
        quote: aligned.replace("变为 ", "变为约 "),
    };
    let problem = verify_quotes(&[quote], &[page]).unwrap_err();
    assert!(problem.contains(aligned));
    assert!(!problem.contains("下一页"));
}

#[test]
fn single_comparison_repair_reports_quote_alignment_and_follow_up_errors_together() {
    let result = run(&book(), &[ANCHOR.into()], None, "t", EXPRESSION, |_| {}, |stage, _, packet| {
        if stage == TeachBackStage::OrganizeExpression { return Ok(expression().to_string()); }
        if let Some(diagnostic) = packet["validation_error"].as_str() {
            for message in ["UTF-16", "orphan", "Focus r: Agreement and divergence require both sides", "unresolved row"] {
                assert!(diagnostic.contains(message), "{diagnostic}");
            }
            return Ok(comparison(&packet["input"]).to_string());
        }
        let mut bad = comparison(packet);
        bad["book_graph"]["nodes"].as_array_mut().unwrap().push(json!({
            "id":"orphan","label":"计算时间不变","evidence":[{"lid":"1.5","quote":"计算时间保持不变。"}]
        }));
        bad["book_graph"]["edges"][0]["evidence"][0]["quote"] = json!("not the source text");
        bad["rows"][0]["book_edge_ids"] = json!([]);
        bad["rows"][0]["status"] = json!("consistent");
        Ok(bad.to_string())
    }).unwrap();
    assert!(result.is_some());
}

#[test]
fn help_ends_before_passages_or_second_progress_event() {
    let mut progress = Vec::new();
    let outcome = run(
        &book(),
        &[ANCHOR.into()],
        Some("如何换卡？"),
        "t",
        "带宽是什么？",
        |s| progress.push(s),
        |stage, _, input| {
            assert_eq!(stage, TeachBackStage::OrganizeExpression);
            assert_eq!(input.as_object().unwrap().len(), 3);
            assert!(!input.to_string().contains("读取下界减半"));
            Ok(json!({"kind":"help"}).to_string())
        },
    )
    .unwrap();
    assert!(outcome.is_none());
    assert_eq!(progress, [TeachBackStage::OrganizeExpression]);
}

#[test]
fn blind_student_is_frozen_then_aligned_to_actually_read_local_passages() {
    let mut calls = 0;
    let mut progress = Vec::new();
    let result = run(&book(), &[ANCHOR.into()], Some("如何换卡？"), "learner-turn", EXPRESSION,
        |s| progress.push(s), |stage, _, input| {
            calls += 1;
            let output = match stage {
                TeachBackStage::OrganizeExpression => {
                    assert!(!input.to_string().contains("读取下界减半"));
                    expression()
                }
                TeachBackStage::ComparePassages if calls == 2 => {
                    assert_eq!(input["frozen_student_graph"]["edges"][0]["quotes"][0],
                        json!({"turn_id":"learner-turn","text":EXPRESSION}));
                    assert!(input["evidence"]["nearby_lids"].as_array().unwrap().contains(&json!("1.1")));
                    assert!(!input["evidence"]["passages"].as_array().unwrap().iter().any(|p| p["lid"]=="1.1"));
                    json!({"read_lids":["1.1"]})
                }
                TeachBackStage::ComparePassages => {
                    assert_eq!(input["nearby_read_available"], false);
                    assert!(input["evidence"]["passages"].as_array().unwrap().iter().any(|p| p["lid"]=="1.1"));
                    let mut answer = comparison(input);
                    answer["side_notes"] = json!([{"label":"容量的作用","text":"容量关系到能否装下。","evidence":[
                        {"lid":"1.1","quote":"容量决定数据能否放下。"}]}]);
                    answer
                }
            };
            Ok(output.to_string())
        }).unwrap().unwrap();
    assert_eq!(
        progress,
        [
            TeachBackStage::OrganizeExpression,
            TeachBackStage::ComparePassages
        ]
    );
    assert_eq!(result.round_id, "tb:learner-turn:r1");
    assert_eq!(result.book_graph.edges[0].evidence.len(), 2);
    assert_eq!(result.side_notes.len(), 1);
}

#[test]
fn repair_of_book_output_cannot_edit_the_frozen_student_and_never_leaks_a_partial_result() {
    let mut calls = 0;
    let result = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "t",
        EXPRESSION,
        |_| {},
        |stage, _, input| {
            calls += 1;
            if stage == TeachBackStage::OrganizeExpression {
                return Ok(expression().to_string());
            }
            let mut answer = comparison(input.get("input").unwrap_or(input));
            if calls == 2 {
                answer["student_graph"] = json!({"nodes":[],"edges":[]});
            } else {
                assert!(input["validation_error"]
                    .as_str()
                    .unwrap()
                    .contains("student_graph"));
                assert_eq!(
                    input["input"]["frozen_student_graph"]["edges"][0]["quotes"][0]["text"],
                    EXPRESSION
                );
            }
            Ok(answer.to_string())
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.student_graph.nodes[1].label, "读取加快");
    assert_eq!(calls, 3);
}

#[test]
fn invalid_student_quotes_and_book_revision_share_a_single_repair_budget() {
    let mut calls = 0;
    let failure = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "t",
        EXPRESSION,
        |_| {},
        |stage, _, input| {
            calls += 1;
            let mut answer = match stage {
                TeachBackStage::OrganizeExpression => expression(),
                TeachBackStage::ComparePassages => comparison(input),
            };
            match calls {
                1 => answer["student_graph"]["edges"][0]["quotes"] = json!(["带宽翻倍"]),
                2 => assert!(input["validation_error"]
                    .as_str()
                    .unwrap()
                    .contains("exact quotations")),
                3 => answer["source"]["source_revision"] = json!("wrong-version"),
                _ => panic!("unexpected extra repair"),
            }
            Ok(answer.to_string())
        },
    )
    .unwrap_err();
    assert_eq!(calls, 3);
    assert_eq!(failure.error_code, "TEACH_BACK_INVALID");
    assert!(failure.message.contains("source_revision"));
}

#[test]
fn quotes_from_unread_lids_and_foreign_relation_or_followup_ids_fail() {
    let evidence = book().teach_back_evidence(&[ANCHOR.into()]).unwrap();
    let students = freeze(
        serde_json::from_value(expression()).unwrap(),
        "t",
        EXPRESSION,
    )
    .unwrap()
    .unwrap();
    let base = comparison(&json!({"evidence":evidence}));
    for (pointer, wrong) in [
        (
            "/book_graph/edges/0/evidence/0",
            json!({"lid":"1.1","quote":"容量决定数据能否放下。"}),
        ),
        (
            "/book_graph/edges/0/evidence/1/quote",
            json!("带宽保持不变。"),
        ),
        ("/rows/0/book_edge_ids", json!(["s"])),
        ("/rows/0/student_edge_ids", json!(["another-turn"])),
        ("/follow_up/row_id", json!("stale-row")),
    ] {
        let mut value = base.clone();
        *value.pointer_mut(pointer).unwrap() = wrong;
        let answer: ComparisonDraft = serde_json::from_value(value).unwrap();
        assert!(
            answer.verify(&evidence, &students, false).is_err(),
            "{pointer}"
        );
    }
}

#[test]
#[ignore = "requires the local published performance-hints workspace"]
fn enhanced_book_round_aligns_two_student_clauses_with_three_read_passages() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.understand-book/performance-hints");
    let book = Book::load(path.to_str().unwrap()).unwrap();
    let learner = "First estimate the memory cost. Then add the cost of mispredicted branches.";
    let result = run(&book, &["1.6.14.5".into()],
        Some("How does the author estimate the cost of sorting one billion numbers?"),
        "enhanced-turn", learner, |_| {}, |stage, _, input| {
            let output = match stage {
                TeachBackStage::OrganizeExpression => {
                    assert!(!input.to_string().contains("82.5"));
                    json!({"kind":"explanation","student_graph":{
                        "nodes":[{"id":"sn1","label":"Estimate memory cost"},{"id":"sn2","label":"Add branch costs"},{"id":"sn3","label":"Combined estimate"}],
                        "edges":[{"id":"s1","from":"sn1","to":"sn3","relation_type":"supports","quotes":["First estimate the memory cost."]},
                                 {"id":"s2","from":"sn2","to":"sn3","relation_type":"supports","quotes":["Then add the cost of mispredicted branches."]}]}})
                }
                TeachBackStage::ComparePassages => {
                    let evidence = &input["evidence"];
                    for field in ["edges","discourse","structure"] {
                        assert!(!evidence[field].as_array().unwrap().is_empty(), "{field}");
                    }
                    json!({"source":{"source_id":evidence["source_id"],"source_revision":evidence["source_revision"]},
                        "book_graph":{"nodes":[{"id":"bn1","label":"Add memory and branch costs"},{"id":"bn2","label":"Estimate the total time"}],
                            "edges":[{"id":"b","from":"bn1","to":"bn2","relation_type":"concludes","condition":"Using the example assumptions",
                            "evidence":[{"lid":"1.6.14.3","quote":"Memory bandwidth"},{"lid":"1.6.14.4","quote":"Branch mispredictions"},
                            {"lid":"1.6.14.5","quote":"Adding up the previous numbers"}]}]},
                        "rows":[{"row_id":"main","book_edge_ids":["b"],"student_edge_ids":["s1","s2"],
                            "status":"consistent","summary":"Both estimates include the two costs."}],"side_notes":[]})
                }
            };
            Ok(output.to_string())
        }).unwrap().unwrap();
    assert_eq!(result.source.source_revision, book.source_fingerprint());
    assert_eq!(result.rows[0].student_edge_ids.len(), 2);
    assert_eq!(result.book_graph.edges[0].evidence.len(), 3);
    assert!(result.follow_up.is_none());
}

fn long_anchor(text: &str) -> Book {
    let node = LidNode {
        lid: "1".into(),
        path: vec![1],
        children: vec![],
        kind: NodeKind::Paragraph,
        span: Span {
            start: 0,
            end: text.encode_utf16().count(),
        },
    };
    Book::new(
        ReadOnlyBase {
            book_id: "long".into(),
            lid_nodes: vec![node],
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        text,
    )
}

#[test]
fn frozen_propositions_locate_the_late_anchor_evidence_before_comparison() {
    let words = "自愿交易应受保护";
    let quoted = "双方自愿的交易应受保护。";
    let book = long_anchor(&format!("{}{quoted}", "文".repeat(12000)));
    let mut calls = 0;
    let result = run(&book, &["1".into()], None, "turn", words, |_| {}, |stage, _, input| {
        calls += 1;
        let answer = match stage {
            TeachBackStage::OrganizeExpression => {
                assert!(!input.to_string().contains(quoted));
                student_explanation(None, words, words)
            }
            TeachBackStage::ComparePassages => {
                let evidence = &input["evidence"];
                let pages = evidence["passages"].as_array().unwrap();
                assert!(pages.iter().any(|p| p["text"].as_str().unwrap().contains(quoted)));
                json!({
                    "source":{"source_id":evidence["source_id"],"source_revision":evidence["source_revision"]},
                    "book_graph":{"nodes":[{"id":"choice","label":"双方自愿交易"},{"id":"protection","label":"交易应受保护"}],
                        "edges":[{"id":"b","from":"choice","to":"protection","relation_type":"concludes","evidence":[{"lid":"1","quote":quoted}]}]},
                    "rows":[{"row_id":"r","book_edge_ids":["b"],"student_node_ids":["sn2"],"summary":"结论一致。","status":"consistent"}],
                    "side_notes":[]
                })
            }
        };
        Ok(answer.to_string())
    }).unwrap().unwrap();
    assert_eq!(calls, 2);
    assert_eq!(result.book_graph.edges[0].evidence[0].quote, quoted);
}

#[test]
fn supplemental_read_verifies_a_quote_crossing_the_end_of_the_first_page() {
    let text = format!("{}甲🛶乙", "文".repeat(6399));
    let book = long_anchor(&text);
    let result = run(&book, &["1".into()], Some("哪个字在船前？"), "t", "甲在船前。", |_| {},
        |stage, _, input| {
            let output = match stage {
                TeachBackStage::OrganizeExpression => student_explanation(None,"甲在船前","甲在船前。"),
                TeachBackStage::ComparePassages if input["nearby_read_available"] == true => {
                    let page = &input["evidence"]["passages"][0];
                    assert_eq!(page["next_start_utf16"],6400);
                    assert!(!page["text"].as_str().unwrap().contains("🛶"));
                    json!({"read_lids":["1"]})
                }
                TeachBackStage::ComparePassages => {
                    let evidence = &input["evidence"];
                    assert!(evidence["passages"][0]["text"].as_str().unwrap().ends_with("甲🛶乙"));
                    json!({"source":{"source_id":evidence["source_id"],"source_revision":evidence["source_revision"]},
                        "book_graph":{"nodes":[{"id":"bn1","label":"甲后出现船"},{"id":"bn2","label":"甲在船前"}],
                            "edges":[{"id":"b","from":"bn1","to":"bn2","relation_type":"concludes","evidence":[{"lid":"1","quote":"甲🛶乙"}]}]},
                        "rows":[{"row_id":"r","book_edge_ids":["b"],"student_node_ids":["sn2"],
                            "status":"consistent","summary":"位置一致。"}],"side_notes":[]})
                }
            };
            Ok(output.to_string())
        }).unwrap().unwrap();
    assert_eq!(result.book_graph.edges[0].evidence[0].quote, "甲🛶乙");
}

fn original_card() -> TeachBackComparison {
    run(
        &book(),
        &[ANCHOR.into()],
        Some("如何换卡？"),
        "first",
        EXPRESSION,
        |_| {},
        |stage, _, input| {
            Ok(match stage {
                TeachBackStage::OrganizeExpression => expression(),
                TeachBackStage::ComparePassages => comparison(input),
            }
            .to_string())
        },
    )
    .unwrap()
    .unwrap()
}

#[test]
fn dispute_rereads_the_original_words_and_rejects_evidence_from_the_new_request() {
    let previous = original_card();
    let message = "你将我说的‘容量’误读成了带宽。";
    let mut calls = 0;
    let result = super::run(
        &book(),
        &[ANCHOR.into()],
        Some("如何换卡？"),
        None,
        "dispute",
        message,
        Some(TeachBackContinuation {
            kind: TeachBackReplyKind::DisputeInterpretation,
            previous: &previous,
            row_id: "r",
            first_expression: EXPRESSION,
            first_turn_id: "first",
        }),
        |_| {},
        |stage, _, packet| {
            calls += 1;
            let input = packet.get("input").unwrap_or(packet);
            let output = match stage {
                TeachBackStage::OrganizeExpression => {
                    assert_eq!(input["expression"], EXPRESSION);
                    assert_eq!(input["correction_request"], message);
                    assert_eq!(input["disputed_quotes"], json!([EXPRESSION]));
                    assert!(input.get("selected_prior_row").is_none());
                    assert!(!input.to_string().contains("读取量不变"));
                    let mut proposal = expression();
                    if calls == 1 {
                        proposal["student_graph"]["edges"][0]["quotes"] = json!([message]);
                    } else {
                        assert!(packet["validation_error"]
                            .as_str()
                            .unwrap()
                            .contains("exact quotations"));
                    }
                    proposal
                }
                TeachBackStage::ComparePassages => {
                    assert_eq!(
                        input["frozen_student_graph"]["edges"][0]["quotes"][0],
                        json!({"turn_id":"first","text":EXPRESSION})
                    );
                    assert_eq!(input["first_expression"], EXPRESSION);
                    assert_eq!(input["selected_prior_row"]["row"]["row_id"], "r");
                    comparison(input)
                }
            };
            Ok(output.to_string())
        },
    )
    .unwrap();
    let TeachBackResult::Comparison(card) = result else {
        panic!("expected reinterpreted card")
    };
    assert_eq!(card.round_id, "tb:dispute:r1");
    assert_eq!(card.student_graph.edges[0].quotes[0].turn_id, "first");
}

#[test]
fn dispute_with_new_reason_can_be_declined_at_either_stage() {
    let previous = original_card();
    for needs_check_against_passages in [false, true] {
        let mut stages = Vec::new();
        let result = super::run(
            &book(),
            &[ANCHOR.into()],
            None,
            None,
            "dispute",
            "其实还要考虑计算下界。",
            Some(TeachBackContinuation {
                kind: TeachBackReplyKind::DisputeInterpretation,
                previous: &previous,
                row_id: "r",
                first_expression: EXPRESSION,
                first_turn_id: "first",
            }),
            |s| stages.push(s),
            |stage, _, _| {
                Ok(if needs_check_against_passages && stage == TeachBackStage::OrganizeExpression {
                expression()
            } else { json!({"kind":"needs_revision"}) }.to_string())
            },
        )
        .unwrap();
        assert!(matches!(result, TeachBackResult::NeedsRevision));
        assert_eq!(
            stages.contains(&TeachBackStage::ComparePassages),
            needs_check_against_passages
        );
    }
}

#[test]
fn new_expression_after_feedback_can_clarify_without_rewriting_prior_quotes() {
    let previous = original_card();
    let message = "带宽加倍才会缩短读取时间下界。";
    for kind in [
        TeachBackReplyKind::ReviseUnderstanding,
        TeachBackReplyKind::FollowUp,
    ] {
        let result = super::run(&book(), &[ANCHOR.into()], None, None, "new", message,
            Some(TeachBackContinuation { kind, previous: &previous, row_id: "r",
                first_expression: EXPRESSION, first_turn_id: "first" }), |_| {},
            |stage, _, input| Ok(match stage {
                TeachBackStage::OrganizeExpression => {
                    assert_eq!(input.as_object().unwrap().len(), 3);
                    assert_eq!(input["expression"], message);
                    student_explanation(Some("带宽加倍"),"读取时间下界缩短",message)
                }
                TeachBackStage::ComparePassages => {
                    assert_eq!(input["selected_prior_row"]["follow_up"]["question"], "只加容量会提高带宽吗？");
                    let mut answer = comparison(input);
                    answer["rows"][0] = json!({"row_id":"new","book_edge_ids":["b"],"student_edge_ids":["s"],
                        "status":"clarified","summary":"你把影响读取时间下界的量改为了带宽。"});
                    answer["follow_up"] = Value::Null;
                    answer
                }
            }.to_string())).unwrap();
        let TeachBackResult::Comparison(card) = result else {
            panic!("expected clarified card")
        };
        assert_eq!(card.rows[0].status, TeachBackStatus::Clarified);
        assert_eq!(card.student_graph.edges[0].quotes[0].turn_id, "new");
        assert_eq!(previous.student_graph.edges[0].quotes[0].text, EXPRESSION);
    }
}

#[test]
fn reply_that_does_not_address_the_active_question_resumes_chat_without_a_card() {
    let previous = original_card();
    let message = "另一个系统按月计费。";
    let result = super::run(
        &book(),
        &[ANCHOR.into()],
        None,
        None,
        "new",
        message,
        Some(TeachBackContinuation {
            kind: TeachBackReplyKind::FollowUp,
            previous: &previous,
            row_id: "r",
            first_expression: EXPRESSION,
            first_turn_id: "first",
        }),
        |_| {},
        |stage, _, _| {
            Ok(match stage {
                TeachBackStage::OrganizeExpression => student_explanation(None, message, message),
                TeachBackStage::ComparePassages => json!({"kind":"ordinary_chat"}),
            }
            .to_string())
        },
    )
    .unwrap();
    assert!(matches!(result, TeachBackResult::OrdinaryChat));
}

#[test]
fn accepts_analogy_bare_conclusion_and_claim_without_book_basis_as_distinct_rows() {
    for (question, words, premise, status, known_basis) in [
        (
            "在读取量不变时，什么改变会让数据搬得更快？",
            "车道加宽才会把同样多的货更快搬完。",
            Some("车道加宽"),
            "alternative_valid",
            true,
        ),
        (
            "读取量不变而带宽翻倍，时间下界怎么变？",
            "读取时间下界减半。",
            None,
            "consistent",
            true,
        ),
        (
            "能从这段读出什么关于这张卡的结论？",
            "这张卡的实测温度会是四十度。",
            None,
            "insufficient_book_basis",
            false,
        ),
    ] {
        let result = run(&book(), &[ANCHOR.into()], Some(question), "t", words, |_| {}, |stage,_,input| {
            Ok(match stage {
                TeachBackStage::OrganizeExpression => student_explanation(premise,words,words),
                TeachBackStage::ComparePassages => {
                    let mut answer = comparison(input);
                    answer["rows"][0] = json!({"row_id":"r","book_edge_ids": if known_basis {vec!["b"]} else {vec![]},
                        "student_edge_ids":if premise.is_some() {vec!["s"]} else {vec![]},
                        "student_node_ids":if premise.is_none() {vec!["sn2"]} else {vec![]}, "status":status,"summary":words});
                    answer["follow_up"] = Value::Null;
                    if !known_basis { answer["book_graph"]=json!({"nodes":[],"edges":[]}); }
                    answer
                }
            }.to_string())
        }).unwrap().unwrap();
        assert_eq!(
            result.student_graph.edges.len(),
            usize::from(premise.is_some())
        );
        if let Some(reason) = premise {
            assert_eq!(result.student_graph.nodes[0].label, reason);
        } else {
            assert_eq!(result.student_graph.nodes[0].quotes[0].text, words);
        }
        assert_eq!(result.book_graph.nodes.is_empty(), !known_basis);
        assert_eq!(serde_json::to_value(result.rows[0].status).unwrap(), status);
    }
}

#[test]
fn connected_two_to_two_focus_requires_every_edge_exactly_once() {
    let evidence = book().teach_back_evidence(&[ANCHOR.into()]).unwrap();
    let words = "容量翻倍，读得更快，所以总耗时会减半。";
    let mut expression = student_explanation(Some("容量翻倍"), "读得更快", "容量翻倍，读得更快");
    let graph = &mut expression["student_graph"];
    graph["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sn3","label":"总耗时减半"}));
    graph["edges"].as_array_mut().unwrap().push(json!({
        "id":"s2","from":"sn2","to":"sn3","relation_type":"concludes","quotes":["所以总耗时会减半"]
    }));
    let student = freeze(serde_json::from_value(expression).unwrap(), "t", words)
        .unwrap()
        .unwrap();

    let mut answer = comparison(&json!({"evidence":evidence}));
    answer["book_graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"bn3","label":"仍取决于计算时间"}));
    answer["book_graph"]["edges"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"b2","from":"bn2","to":"bn3","relation_type":"requires",
            "evidence":[{"lid":"1.5","quote":"计算时间保持不变。"}]
        }));
    answer["rows"][0]["book_edge_ids"] = json!(["b", "b2"]);
    answer["rows"][0]["student_edge_ids"] = json!(["s", "s2"]);
    let verify = |answer: &Value| {
        serde_json::from_value::<ComparisonDraft>(answer.clone())
            .unwrap()
            .verify(&evidence, &student, false)
    };
    verify(&answer).unwrap();

    let mut missing = answer.clone();
    missing["rows"][0]["student_edge_ids"] = json!(["s"]);
    assert!(verify(&missing).unwrap_err().contains("s2"));

    let mut duplicate = answer.clone();
    let mut row = duplicate["rows"][0].clone();
    row["row_id"] = json!("r2");
    duplicate["rows"].as_array_mut().unwrap().push(row);
    assert!(verify(&duplicate).is_err());

    let mut disconnected = answer.clone();
    disconnected["book_graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"bn4","label":"计算量不变"}));
    disconnected["book_graph"]["edges"][1]["from"] = json!("bn4");
    assert!(verify(&disconnected).is_err());

    // The saved ACP repair assigned one edge twice and listed its endpoint
    // as a standalone claim, while leaving another edge without a focus.
    let mut repaired = answer;
    repaired["rows"][0]["book_edge_ids"] = json!(["b"]);
    repaired["rows"].as_array_mut().unwrap().push(json!({
        "row_id":"r2", "book_node_ids":["bn1"], "book_edge_ids":["b"],
        "student_edge_ids":["s"], "summary":"读取时间的条件不同。", "status":"divergent",
        "divergence_condition":"带宽不变时"
    }));
    let error = verify(&repaired).unwrap_err();
    for message in [
        "bn1 is not an isolated book node",
        "r2 repeats book ID b",
        "r2 repeats student ID s",
        "Unaligned book IDs: [\"b2\"]",
    ] {
        assert!(error.contains(message), "{error}");
    }
    assert!(!error.contains("isolated node IDs [\"bn1\"]"), "{error}");
}

#[test]
fn verbatim_isolated_label_supplies_its_quote_without_using_the_private_repair() {
    let words = "容量决定装不装得下。装得下以后，容量翻倍，读取也会快一倍。";
    let mut draft = student_explanation(
        Some("容量翻倍"),
        "读取也会快一倍",
        "容量翻倍，读取也会快一倍",
    );
    draft["student_graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"fit", "label":"容量决定装不装得下"
        }));
    let frozen = freeze(serde_json::from_value(draft).unwrap(), "first", words)
        .unwrap()
        .unwrap();
    let claim = frozen.nodes.iter().find(|node| node.id == "fit").unwrap();
    assert_eq!(
        claim.quotes,
        [TeachBackStudentQuote {
            turn_id: "first".into(),
            text: "容量决定装不装得下".into()
        }]
    );
}

#[test]
fn student_links_must_be_directed_and_isolated_claims_must_be_quoted() {
    let mut self_loop = expression();
    self_loop["student_graph"]["edges"][0]["from"] = json!("sn2");
    let mut unknown_target = expression();
    unknown_target["student_graph"]["edges"][0]["to"] = json!("missing");
    let mut bare_node = student_explanation(None, "读取加快", EXPRESSION);
    bare_node["student_graph"]["nodes"][0]["quotes"] = json!([]);
    let mut invented = expression();
    invented["student_graph"]["edges"][0]["quotes"] = json!(["带宽翻倍"]);
    let mut cycle = expression();
    cycle["student_graph"]["edges"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"s2","from":"sn2","to":"sn1","relation_type":"causes","quotes":[EXPRESSION]
        }));
    for invalid in [self_loop, unknown_target, bare_node, invented, cycle] {
        assert!(freeze(serde_json::from_value(invalid).unwrap(), "t", EXPRESSION).is_err());
    }
}

#[test]
fn an_unlinked_book_definition_is_repaired_before_the_reader_sees_it() {
    let mut repairs = 0;
    let result = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "t",
        EXPRESSION,
        |_| {},
        |stage, _, packet| {
            if stage == TeachBackStage::OrganizeExpression {
                return Ok(expression().to_string());
            }
            if packet.get("validation_error").is_some() {
                repairs += 1;
                return Ok(comparison(&packet["input"]).to_string());
            }
            let mut answer = comparison(packet);
            answer["book_graph"] = json!({"nodes":[{"id":"bn1","label":"带宽决定下界",
            "evidence":[{"lid":ANCHOR,"quote":"🛶带宽翻倍，读取下界减半。"}]}],"edges":[]});
            answer["rows"][0]["book_edge_ids"] = json!([]);
            answer["rows"][0]["book_node_ids"] = json!(["bn1"]);
            Ok(answer.to_string())
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(repairs, 1);
    assert_eq!(result.book_graph.edges.len(), 1);
}

#[test]
fn long_and_multiline_labels_are_repaired_in_the_private_stage_that_authored_them() {
    for stage_to_repair in [
        TeachBackStage::OrganizeExpression,
        TeachBackStage::ComparePassages,
    ] {
        for bad_label in ["命".repeat(23), "x".repeat(61), "带宽\n翻倍".into()] {
            let mut repairs = 0;
            let result = run(
                &book(),
                &[ANCHOR.into()],
                None,
                "t",
                EXPRESSION,
                |_| {},
                |stage, _, packet| {
                    let input = packet.get("input").unwrap_or(packet);
                    let mut answer = match stage {
                        TeachBackStage::OrganizeExpression => expression(),
                        TeachBackStage::ComparePassages => comparison(input),
                    };
                    if stage == stage_to_repair {
                        if packet.get("validation_error").is_some() {
                            repairs += 1;
                        } else {
                            let graph = if stage == TeachBackStage::OrganizeExpression {
                                "student_graph"
                            } else {
                                "book_graph"
                            };
                            answer[graph]["nodes"][0]["label"] = json!(bad_label);
                        }
                    }
                    Ok(answer.to_string())
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(repairs, 1);
            assert_eq!(result.student_graph.nodes[0].label, "容量变大");
            assert_eq!(result.book_graph.nodes[0].label, "带宽翻倍");
        }
    }
}

#[test]
fn isolated_book_claim_requires_evidence_from_the_read_interval() {
    let evidence = book().teach_back_evidence(&[ANCHOR.into()]).unwrap();
    let frozen = freeze(
        serde_json::from_value(expression()).unwrap(),
        "t",
        EXPRESSION,
    )
    .unwrap()
    .unwrap();
    let mut card = comparison(&json!({"evidence":evidence}));
    let quote = json!({"lid":"1.5","quote":"计算时间保持不变。"});
    card["book_graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"bn3","label":"计算时间不变","evidence":[quote]}));
    card["rows"].as_array_mut().unwrap().push(json!({"row_id":"r2","book_node_ids":["bn3"],"status":"omitted","summary":"没有提到计算时间。"}));
    let verify = |v: &Value| {
        serde_json::from_value::<ComparisonDraft>(v.clone())
            .unwrap()
            .verify(&evidence, &frozen, false)
    };
    verify(&card).unwrap();
    card["book_graph"]["nodes"][2]["evidence"][0]["lid"] = json!("1.1");
    assert!(verify(&card).is_err());
    card["book_graph"]["nodes"][2]["evidence"] = json!([]);
    assert!(verify(&card).is_err());
}

#[test]
fn historical_comparison_projects_a_single_hop_and_a_quoted_conclusion_without_losing_rows() {
    let quote = json!({"turn_id":"old","text":"读取加快。"});
    let legacy = json!({
        "round_id":"tb:old:r1","source":{"source_id":"book","source_revision":"v1"},
        "book_relations":[{"id":"b","premise":"带宽翻倍","conclusion":"读取下界减半",
            "condition":"读取量不变","relation_type":"causes","evidence":[{"lid":ANCHOR,"quote":"带宽翻倍，读取下界减半。"}]}],
        "student_relations":[{"id":"s","conclusion":"读取加快","relation_type":"concludes","quotes":[quote]}],
        "rows":[{"row_id":"r","book_relation_ids":["b"],"student_relation_ids":["s"],
            "status":"consistent","summary":"方向一致。"},
            {"row_id":"r2","book_relation_ids":["b"],"student_relation_ids":[],
            "status":"omitted","summary":"没有提及条件。"}],
        "side_notes":[],"follow_up":{"row_id":"r2","question":"这个结论需要什么条件？"}
    });
    let card: TeachBackComparison = serde_json::from_value(legacy).unwrap();
    let edge = &card.book_graph.edges[0];
    assert_eq!(
        (edge.id.as_str(), edge.from.as_str(), edge.to.as_str()),
        ("b", "b:from", "b:to")
    );
    assert_eq!(edge.condition.as_deref(), Some("读取量不变"));
    assert_eq!(edge.evidence[0].lid, ANCHOR);
    assert_eq!(card.book_graph.nodes.len(), 2);
    assert!(card.student_graph.edges.is_empty());
    assert_eq!(card.student_graph.nodes[0].id, "s:to");
    assert_eq!(card.student_graph.nodes[0].quotes[0].turn_id, "old");
    assert_eq!(card.rows[0].student_node_ids, ["s:to"]);
    assert_eq!(card.rows[0].book_edge_ids, ["b"]);
    assert_eq!(card.rows[1].book_edge_ids, ["b"]);
    assert_eq!(card.follow_up.as_ref().unwrap().row_id, "r2");
    let saved = serde_json::to_value(&card).unwrap();
    assert!(saved.get("book_relations").is_none());
    assert_eq!(
        serde_json::from_value::<TeachBackComparison>(saved).unwrap(),
        card
    );
}

#[test]
fn focus_targets_name_edges_and_only_isolated_student_nodes() {
    let words = "容量大一倍，读得快一倍。我还预计温度不变。";
    let result = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "turn",
        words,
        |_| {},
        |stage, _, packet| {
            let input = packet.get("input").unwrap_or(packet);
            let answer =
                match stage {
                    TeachBackStage::OrganizeExpression => {
                        assert!(input.get("focus_targets").is_none());
                        let mut draft = expression();
                        draft["student_graph"]["nodes"].as_array_mut().unwrap().push(json!({
                    "id":"temperature","label":"温度不变","quotes":["我还预计温度不变。"]
                }));
                        draft
                    }
                    TeachBackStage::ComparePassages => {
                        assert_eq!(
                            input["focus_targets"],
                            json!({"student_edge_ids":["s"],"student_node_ids":["temperature"]})
                        );
                        let mut answer = comparison(input);
                        answer["rows"].as_array_mut().unwrap().push(json!({
                            "row_id":"temperature_check","student_node_ids":["temperature"],
                            "status":"unverified_against_passage","summary":"本段没有测量温度。"
                        }));
                        answer
                    }
                };
            Ok(answer.to_string())
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.rows.len(), 2);
}

#[test]
fn a_private_repair_replaces_internal_ids_in_each_reader_facing_comparison_field() {
    for (field, leak) in [
        ("/rows/0/summary", "你在e1、e2-e4中改变了推理的条件。"),
        (
            "/rows/0/divergence_condition",
            "把带宽固定时，(s1) 与原文分歧。",
        ),
        ("/follow_up/question", "e1 指的是什么？"),
    ] {
        let mut repairs = 0;
        let card = run(
            &book(),
            &[ANCHOR.into()],
            None,
            "turn",
            EXPRESSION,
            |_| {},
            |stage, _, packet| {
                let input = packet.get("input").unwrap_or(packet);
                let answer = match stage {
                    TeachBackStage::OrganizeExpression => {
                        let mut student = expression();
                        student["student_graph"]["edges"][0]["id"] = json!("s1");
                        student
                    }
                    TeachBackStage::ComparePassages => {
                        let mut draft = comparison(input);
                        draft["book_graph"]["edges"][0]["id"] = json!("e1");
                        draft["rows"][0]["book_edge_ids"] = json!(["e1"]);
                        draft["rows"][0]["student_edge_ids"] = json!(["s1"]);
                        *draft.pointer_mut(field).unwrap() =
                            if packet.get("validation_error").is_some() {
                                repairs += 1;
                                // e10 is not a reference to edge e1.
                                json!("在 e10 型号上固定带宽，只加容量会怎样？")
                            } else {
                                json!(leak)
                            };
                        draft
                    }
                };
                Ok(answer.to_string())
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(repairs, 1, "{field}");
        assert_eq!(
            serde_json::to_value(card).unwrap().pointer(field).unwrap(),
            "在 e10 型号上固定带宽，只加容量会怎样？"
        );
    }
}

#[test]
fn a_term_in_the_readers_own_words_can_coincide_with_an_internal_id() {
    let words = "换成 e1 型号：容量大一倍，读得快一倍。";
    let card = run(
        &book(),
        &[ANCHOR.into()],
        None,
        "turn",
        words,
        |_| {},
        |stage, _, packet| {
            assert!(packet.get("validation_error").is_none());
            let output = match stage {
                TeachBackStage::OrganizeExpression => {
                    student_explanation(Some("容量变大"), "读取加快", words)
                }
                TeachBackStage::ComparePassages => {
                    let mut answer = comparison(packet);
                    answer["book_graph"]["edges"][0]["id"] = json!("e1");
                    answer["rows"][0]["book_edge_ids"] = json!(["e1"]);
                    answer["rows"][0]["summary"] = json!("你推测换成 e1 型号就能加快读取。");
                    answer
                }
            };
            Ok(output.to_string())
        },
    )
    .unwrap()
    .unwrap();
    assert!(card.rows[0].summary.contains("e1"));
}
