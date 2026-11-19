use super::*;
use runtime::presentation_preview::{PresentationPreviewPort, PreviewRequest};

pub const PAGE: &str = include_str!("../../tests/fixtures/presentation-repair.html");

pub fn preview_page(
    html: &str,
    actions: Vec<PreviewAction>,
    viewport: runtime::presentation_preview::PreviewViewport,
) -> runtime::presentation_preview::PreviewReport {
    let content: runtime::presentation::PresentationContent = serde_json::from_value(json!({
        "title":"Repair regression","entrypoint":"index.html","content_files":{"index.html":html},
        "readable_content":"Dependency and query pairs","source_bindings":[],"assumptions":[],
        "state_contract":{},"initial_state":{},
    }))
    .unwrap();
    crate::presentation_preview::BrowserPreview::discover()
        .unwrap()
        .preview(
            &PreviewRequest {
                candidate_id: "repair-regression".into(),
                html: crate::presentation_author::preview_document(&content),
                actions,
                width: None,
                viewport: Some(viewport),
                read_selector: Some("#result".into()),
            },
            &CancellationToken::default(),
        )
        .unwrap()
}

pub fn actions() -> Vec<PreviewAction> {
    vec![
        PreviewAction::Click {
            selector: "#preset".into(),
        },
        PreviewAction::Click {
            selector: "#causal".into(),
        },
    ]
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn repair_preview_checks_narrow_layout_ids_and_actual_restored_results() {
    let narrow = REQUIRED_PREVIEW_ENVIRONMENTS[0].1;
    for (name, html, expected) in [
        ("fixed",PAGE.to_owned(),None),
        ("short-slider",PAGE.replace("height:44px","height:36px"),Some("touch_target_small")),
        ("squeezed-slider",PAGE.replace("min-width:44px;width:100%","min-width:0;width:34px"),Some("touch_target_small")),
        ("grid-overflow",PAGE.replace("minmax(0,1fr))","24px)"),Some("geometry_overflow")),
        ("duplicate-control",PAGE.replace("id=\"pair-section\"","id=\"context\""),Some("duplicate_dom_id")),
        ("missing-restorer",PAGE.replace("window.presentation?.registerStateRestorer(saved=>{state={...saved.values.page};render()});",""),Some("state_restore_mismatch")),
        ("state-without-repaint",PAGE.replace("saved.values.page};render()","saved.values.page}"),Some("state_restore_mismatch")),
        ("wrong-control-after-restore",PAGE.replace("saved.values.page};render()","saved.values.page};render();$('context').value=0"),Some("state_restore_mismatch")),
    ] {
        let report=preview_page(&html,actions(),narrow);
        assert!(report.errors.is_empty(),"{name}: {:?}",report.errors);
        let issues:Vec<_>=report.observations.iter().flat_map(|observation|&observation.issues).collect();
        if let Some(expected)=expected { assert!(issues.iter().any(|issue|issue.kind==expected),"{name}: {issues:?}"); }
        else { assert!(issues.is_empty(),"{name}: {issues:?}"); }
    }
    for (name, viewport) in &REQUIRED_PREVIEW_ENVIRONMENTS[1..] {
        let report = preview_page(PAGE, actions(), *viewport);
        assert!(report.errors.is_empty(), "{name}: {:?}", report.errors);
        assert!(
            report
                .observations
                .iter()
                .all(|observation| observation.issues.is_empty()),
            "{name}: {:?}",
            report.observations
        );
    }
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn repair_width_only_receipt_cannot_hide_a_failed_narrow_preview() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope =
        app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let bad =
        format!("{PAGE}<style>@media(max-width:400px){{input{{height:36px!important}}}}</style>");
    let id = candidate(&mut port, &bad);
    let desktop = port
        .author_presentation(
            AuthorRequest::Preview {
                candidate_id: id.clone(),
                width: Some(960),
                viewport: None,
                actions: actions(),
                read_selector: Some("#result".into()),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(desktop.body["status"], "preview_environment_recorded");
    for (name, viewport) in &REQUIRED_PREVIEW_ENVIRONMENTS[..2] {
        let result = port
            .author_presentation(
                AuthorRequest::Preview {
                    candidate_id: id.clone(),
                    width: None,
                    viewport: Some(*viewport),
                    actions: actions(),
                    read_selector: Some("#result".into()),
                },
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        assert_eq!(
            result.body["status"],
            if *name == "narrow-content" {
                "preview_failed"
            } else {
                "preview_environment_recorded"
            }
        );
    }
    assert!(port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id: id },
            &[],
            &[],
            &cancellation
        )
        .is_err());
    let fixed = candidate(&mut port, PAGE);
    for (index, (_, viewport)) in REQUIRED_PREVIEW_ENVIRONMENTS.iter().enumerate() {
        let result = port
            .author_presentation(
                AuthorRequest::Preview {
                    candidate_id: fixed.clone(),
                    width: None,
                    viewport: Some(*viewport),
                    actions: actions(),
                    read_selector: Some("#result".into()),
                },
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        assert_eq!(
            result.body["status"],
            if index == 2 {
                "preview_ready_for_inspection"
            } else {
                "preview_environment_recorded"
            },
            "{}",
            result.body
        );
        assert!(result.body["reading"]["text"]
            .as_str()
            .unwrap()
            .contains("配对数 93"));
        assert_eq!(result.body["reading"]["controls"][0]["value"], "12");
    }
    assert!(port
        .author_presentation(
            AuthorRequest::Deliver {
                candidate_id: fixed
            },
            &[],
            &[],
            &cancellation
        )
        .unwrap()
        .delivered
        .is_some());
}

pub struct RepairTeachingAdapter {
    stage: std::cell::Cell<usize>,
    receipts: std::cell::RefCell<std::collections::BTreeMap<String, Value>>,
    reviews: std::cell::RefCell<Vec<Value>>,
    directory: PathBuf,
}

impl RepairTeachingAdapter {
    fn receipt(&self, id: &str) -> Value {
        self.receipts
            .borrow()
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}: {:?}", self.receipts.borrow().keys()))
            .clone()
    }
    fn selection(&self, id: &str, reference: Value, correct: bool) -> Value {
        json!({"operation":"select","move":{"move_id":id,"design_ref":self.receipt("design")["design_ref"],
            "block_refs":["path"],"object_ids":[],"target":repair_target(),"capability":"explanation","kind":"explain",
            "interaction_intent":"neutral","intent_origin":"neutral","prompt":"解释依赖路径和工作量的区别。",
            "delivery_text":if correct {"三层、四个已知输入允许跨层流水，沿图中依赖边计数，最长依赖链为 6 个单位；12 是全部节点的总工作量。页面用屏号标识步骤。"}
                else {"三层、四个已知输入的最长依赖链为 12 个单位。两笔账分别在第一笔和第二笔中展示。"},
            "presentation":reference,"participation":{"choice":"support","reason":"解释用户所问的路径与工作量。",
                "support":"用同一组节点和代价口径比较。","learner_inference":null}}})
    }
}

pub fn repair_target() -> Value {
    json!({"learning_focus":"依赖路径和总工作量","expected_performance":"区分路径上的等待和所有节点的工作量",
        "capability":"explanation","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]})
}

impl ModelAdapter for RepairTeachingAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        unreachable!()
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(guidance) = super::super::tutor_loop_tests::fixture_coach_guidance(&request) {
            return Ok(guidance);
        }
        if request.system.contains("Teaching explanation acceptance review.") {
            // This replay exercises page repair and persistence, not semantic
            // judgment. Keep auxiliary text review separate from the two
            // source/page verdicts whose findings drive the actual repair.
            return Ok(super::super::tutor_loop_tests::fixture_delivery_review(&request).unwrap());
        }
        assert!(request
            .system
            .contains("Teaching delivery acceptance review."),
            "unexpected structured request: {}", request.system.chars().take(160).collect::<String>());
        let packet: Value = serde_json::from_str(&request.user).unwrap();
        let round = self.reviews.borrow().len() + 1;
        let source = &packet["current_source_evidence"][0];
        assert_eq!(source["segments"].as_array().unwrap().len(), 3);
        assert!(source["segments"][0]["source_range"].is_object());
        let text = packet["proposed_delivery"]["text"].as_str().unwrap();
        let mut review = super::super::tutor_loop_tests::fixture_delivery_review(&request).unwrap();
        if round == 1 {
            assert!(text.contains("12 个单位"));
            review["decision"] = json!("revise");
            review["revision"] = json!("按图重算最长路径，并统一页面步骤名称。");
            review["findings"] = json!([
                {"rule_id":"R3","location":"依赖屏与正文","evidence":"图中三层四位置可流水，12 是总工作量。","required_change":"最长依赖链改为 6，计算式用 layers+tokens-1。"},
                {"rule_id":"R2","location":"页面标题","evidence":"步骤编号被写成概念账目。","required_change":"第一笔、第二笔改为屏 1、屏 2。"}
            ]);
        } else {
            assert_eq!(round, 2);
            assert!(text.contains("最长依赖链为 6"));
            assert_eq!(packet["review_context"]["round"], 2);
            assert!(packet["review_context"]["prior_reviews"]
                .to_string()
                .contains("最长依赖链改为 6"));
            assert!(
                packet["review_context"]["changes_since_previous_review"]
                    .as_array()
                    .unwrap()
                    .len()
                    > 0
            );
        }
        let valid = json!({"statement":"最长路径与全部工作量不同。","sources":[{
            "source_id":source["source_id"],"source_revision":source["source_revision"],"lid":"1.1.1","end_lid":"1.1.3",
            "quote":"最长路径是 $3+4-1=6$ 个单位"}]});
        let invalid = json!({"statement":"错误定位不能进入记忆。","sources":[{
            "source_id":source["source_id"],"source_revision":source["source_revision"],"lid":"1.1.2","quote":"三层、四个已知输入"}]});
        review["agent_work"] = json!({"understanding":if round==1 {vec![invalid,valid]} else {vec![valid]},"open_questions":[]});
        std::fs::write(
            self.directory.join(format!("review-{round}-input.json")),
            serde_json::to_vec_pretty(&packet).unwrap(),
        )
        .unwrap();
        std::fs::write(
            self.directory.join(format!("review-{round}-output.json")),
            serde_json::to_vec_pretty(&review).unwrap(),
        )
        .unwrap();
        self.reviews.borrow_mut().push(packet);
        Ok(review)
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        use base64::Engine as _;
        for message in &plan.input {
            if let (Some(id), Some(text)) = (&message.tool_call_id, &message.content) {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    self.receipts.borrow_mut().insert(
                        id.clone(),
                        value.get("model_body").unwrap_or(&value).clone(),
                    );
                }
            }
        }
        let sampling = self.stage.get();
        self.stage.set(sampling + 1);
        std::fs::write(
            self.directory.join("receipts.json"),
            serde_json::to_vec_pretty(&*self.receipts.borrow()).unwrap(),
        )
        .unwrap();
        if sampling == 0 {
            return Ok(AssistantTurn {
                provider_continuation: None,
                text: None,
                usage_total_tokens: None,
                tool_calls: vec![ToolCall {
                    id: "locate".into(),
                    name: "book.context".into(),
                    arguments: json!({"lid":"1.1"}).to_string(),
                }],
            });
        }
        let stage = sampling - 1;
        for (index, picture) in plan.preview_images.iter().enumerate() {
            std::fs::write(
                self.directory.join(format!("stage-{stage}-{index}.png")),
                base64::engine::general_purpose::STANDARD
                    .decode(&picture.png_base64)
                    .unwrap(),
            )
            .unwrap();
        }
        let call = |id: &str, name: &str, args: Value| ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args.to_string(),
        };
        let author = |id: &str, args: Value| call(id, "presentation.author", args);
        let previews = |id: &str, candidate: Value| {
            REQUIRED_PREVIEW_ENVIRONMENTS.iter().enumerate().map(|(index,(_,viewport))|
            author(&format!("{id}-{index}"),json!({"operation":"preview","candidate_id":candidate,"viewport":viewport,
                "read_selector":"#result","actions":[{"kind":"click","selector":"#preset"},{"kind":"click","selector":"#causal"}]}))).collect()
        };
        let calls = match stage {
            0 => vec![call("source", "book.text", json!({"lid":"1.1"}))],
            1 => {
                assert!(
                    self.receipt("source")["error_code"].is_null(),
                    "source read failed: {}",
                    self.receipt("source")
                );
                vec![call(
                    "design",
                    "tutor.step",
                    json!({"operation":"design","operation_id":"path-design","design":{
                "target":repair_target(),"organizing_model":"按实际依赖边区分最长路径和总工作量。",
                "representation":{"kind":"presentation","reason":"切换依赖模式，保留节点和单位代价。","observation":"节点总量不等于最长路径。"},
                "blocks":[{"block_id":"path","relation":"最长路径只累计依赖链上的节点。","purpose":"区分等待与工作总量。","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}],
                "focus_block_refs":["path"],"connections":[],"basis_refs":[],"open_question_refs":[],"revision_reason":"用可操作图示回答当前请求。"}}),
                )]
            }
            2 => {
                assert!(
                    self.receipt("design")["design_ref"].is_string(),
                    "design failed: {}",
                    self.receipt("design")
                );
                assert!(
                    plan.ordered_messages()
                        .iter()
                        .filter_map(|m| m.content.as_deref())
                        .any(|text| text.contains("registerStateRestorer")),
                    "state contract must arrive before write; design={} assets={:?}",
                    self.receipt("design"),
                    plan.instruction_assets
                );
                let bad = PAGE
                    .replace("const chain=layers+tokens-1;", "const chain=layers*tokens;")
                    .replace("屏 1", "第一笔")
                    .replace("屏 2", "第二笔");
                vec![author(
                    "write",
                    json!({"operation":"write","title":"依赖路径与查询配对","html":bad,
                    "readable_content":"依赖路径与查询配对。RNN 最长依赖链 12，因果 Transformer 为 3。",
                    "teaching_binding":{"design_ref":self.receipt("design")["design_ref"],"block_refs":["path"],"regions":[{
                        "region_id":"dependency-section","block_refs":["path"],"state_fields":["mode"],"observation":"改变依赖模式，比较路径。"}]},
                    "state_contract":{"s":"已有上下文 token 数，0..12","p":"新输入 token 数，固定 6","mode":"rnn 或 causal"},
                    "initial_state":{"s":0,"p":6,"mode":"rnn"}}),
                )]
            }
            3 => previews("before", self.receipt("write")["candidate_id"].clone()),
            4 => {
                for index in 0..3 {
                    assert!(self.receipt(&format!("before-{index}"))["errors"]
                        .as_array()
                        .unwrap()
                        .is_empty());
                }
                vec![
                    author(
                        "save-first",
                        json!({"operation":"deliver","candidate_id":self.receipt("write")["candidate_id"]}),
                    ),
                    call(
                        "early-select",
                        "tutor.step",
                        self.selection("early", Value::Null, false),
                    ),
                ]
            }
            5 => {
                assert_eq!(
                    self.receipt("early-select")["error_code"],
                    "TEACHING_PRESENTATION_REQUIRES_NEXT_SAMPLING"
                );
                vec![call(
                    "select-first",
                    "tutor.step",
                    self.selection(
                        "first",
                        self.receipt("save-first")["reference"].clone(),
                        false,
                    ),
                )]
            }
            6 => {
                let feedback = self.receipt("select-first");
                assert!(
                    feedback["message"]
                        .as_str()
                        .unwrap()
                        .contains("最长依赖链改为 6"),
                    "{feedback}"
                );
                assert!(
                    feedback["message"]
                        .as_str()
                        .unwrap()
                        .contains("第一笔、第二笔改为屏 1、屏 2"),
                    "{feedback}"
                );
                assert!(!feedback["message"]
                    .as_str()
                    .unwrap()
                    .contains("教师工作记忆的原文依据无效"));
                vec![author(
                    "read-first",
                    json!({"operation":"read","reference":self.receipt("save-first")["reference"],"file":"index.html"}),
                )]
            }
            7 => vec![author(
                "patch",
                json!({"operation":"patch","reference":self.receipt("save-first")["reference"],
                "readable_content":"依赖路径与查询配对。RNN 最长依赖链 6，因果 Transformer 为 3。步骤按屏号区分。",
                "edits":[{"old_text":"const chain=layers*tokens;","new_text":"const chain=layers+tokens-1;"},
                    {"old_text":"第一笔","new_text":"屏 1"},{"old_text":"第二笔","new_text":"屏 2"}]}),
            )],
            8 => {
                assert!(
                    self.receipt("patch")["candidate_id"].is_string(),
                    "patch failed: {}",
                    self.receipt("patch")
                );
                previews("after", self.receipt("patch")["candidate_id"].clone())
            }
            9 => {
                for index in 0..3 {
                    assert!(self.receipt(&format!("after-{index}"))["errors"]
                        .as_array()
                        .unwrap()
                        .is_empty());
                }
                vec![author(
                    "save-fixed",
                    json!({"operation":"deliver","candidate_id":self.receipt("patch")["candidate_id"]}),
                )]
            }
            10 => vec![call(
                "select-fixed",
                "tutor.step",
                self.selection(
                    "fixed",
                    self.receipt("save-fixed")["reference"].clone(),
                    true,
                ),
            )],
            11 => {
                assert!(
                    self.receipt("select-fixed")["error_code"].is_null(),
                    "{}",
                    self.receipt("select-fixed")
                );
                return Ok(AssistantTurn {
                    provider_continuation: None,
                    text: Some("完成修订。".into()),
                    tool_calls: vec![],
                    usage_total_tokens: None,
                });
            }
            _ => panic!(
                "unexpected model stage {stage}: {:?}",
                self.receipts.borrow()
            ),
        };
        Ok(AssistantTurn {
            provider_continuation: None,
            text: None,
            tool_calls: calls,
            usage_total_tokens: None,
        })
    }
}

#[test]
#[ignore = "requires installed Chromium/Edge; replays model decisions through real tools and persistence"]
fn repair_teaching_full_flow_returns_findings_and_delivers_only_revised_page() {
    use base_schema::{LidNode, NodeKind, Span};
    let parts=["三层、四个已知输入，每个节点耗时一单位，允许流水。每个 RNN 节点等待上一层同位置和同层前一位置。最长路径是 ","$3+4-1=6$"," 个单位；总工作量是 12 个单位。查询配对数是旧上下文长方形加新输入因果三角形。"];
    let source = parts.concat();
    let end = source.encode_utf16().count();
    let mut base = sample_base();
    base.book_id = "presentation-repair-regression".into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    base.lid_nodes = vec![
        LidNode {
            lid: "1".into(),
            path: vec![1],
            kind: NodeKind::Chapter,
            span: Span { start: 0, end },
            children: vec!["1.1".into()],
        },
        LidNode {
            lid: "1.1".into(),
            path: vec![1, 1],
            kind: NodeKind::Paragraph,
            span: Span { start: 0, end },
            children: vec!["1.1.1".into(), "1.1.2".into(), "1.1.3".into()],
        },
    ];
    let mut start = 0;
    for (index, part) in parts.iter().enumerate() {
        let end = start + part.encode_utf16().count();
        base.lid_nodes.push(LidNode {
            lid: format!("1.1.{}", index + 1),
            path: vec![1, 1, index as u32 + 1],
            kind: if index == 1 {
                NodeKind::Formula
            } else {
                NodeKind::Paragraph
            },
            span: Span { start, end },
            children: vec![],
        });
        start = end;
    }
    let (_root, mut state) =
        super::super::tutor_loop_tests::fixture_with_book(false, Book::new(base, &source), &source);
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/turn-failure-20261019/repair-integration");
    std::fs::create_dir_all(&directory).unwrap();
    let adapter = RepairTeachingAdapter {
        stage: Default::default(),
        receipts: Default::default(),
        reviews: Default::default(),
        directory: directory.clone(),
    };
    let run = prepare_agent_chat(
        &mut state,
        &json!({"message":"请用演示页解释依赖路径与总工作量，并保留操作后的状态。"}).to_string(),
        "repair-flow",
    )
    .unwrap_or_else(|error| panic!("{}", error.body));
    let turn = run.turn_ref.clone();
    let tutor_session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = agent_run::execute_prepared(
        &BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,
        run,
        Default::default(),
    );
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    assert_eq!(adapter.reviews.borrow().len(), 2);
    let store = state.user.learning_store().unwrap();
    let deliveries = store
        .teaching_deliveries(&turn.session_id, &turn.turn_id)
        .unwrap();
    assert_eq!(
        deliveries.len(),
        1,
        "only the accepted revision may reach the reader"
    );
    assert!(deliveries[0].payload["answer"]
        .as_str()
        .unwrap()
        .contains("最长依赖链为 6"));
    let events = store.teaching_events(&tutor_session, 0, 100).unwrap();
    let reviews: Vec<_> = events
        .iter()
        .filter(|(_, event)| event.kind == memory::teaching::TeachingFact::AgentWorkReviewed)
        .collect();
    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews[0].1.payload["review"]["decision"], "revise");
    assert_eq!(
        reviews[0].1.payload["agent_work_warnings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reviews[0].1.payload["review"]["agent_work"]["understanding"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(reviews[1].1.payload["review"]["decision"], "accept");
    let reference: runtime::presentation::PresentationRef =
        serde_json::from_value(adapter.receipt("save-fixed")["reference"].clone()).unwrap();
    let version = state
        .private_context()
        .read_presentation(&turn.session_id, &reference)
        .unwrap();
    assert!(version.content.content_files["index.html"].contains("const chain=layers+tokens-1;"));
    assert!(!version.content.content_files["index.html"].contains("第一笔"));
    std::fs::write(
        directory.join("final-page.html"),
        crate::presentation_author::preview_document(&version.content),
    )
    .unwrap();
    std::fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&json!({"http_status":result.reply.status,
        "model_boundary":"deterministic replay; real runtime, browser previews, review validation and persistent store",
        "review_count":2,"delivered_count":deliveries.len(),"final_reference":reference,"events":events,"receipts":*adapter.receipts.borrow()})).unwrap()).unwrap();
}
