// Fixed-transcript request-builder experiment, not an autonomous Tutor/RAG run.
// Source reads and citations execute against real Book fixtures. Tutor states
// and prior explanations are declared inputs; no presentation delivery is faked.
use super::*;
use serde_json::{json, Value};

const OVERFITTING_SOURCE: &str = "过拟合与泛化。训练集参与参数拟合，验证集用于选择模型和超参数，测试集应留到最终评估。模型把训练样本中特有的噪声也当成规律时，训练误差可能持续下降，而未见样本上的误差反而上升，这就是过拟合的一种典型表现。一次训练误差较低本身不足以判断模型更好，应比较同一验证集上的表现。例子：在相同的数据划分与误差指标下，模型甲的训练误差为 0.01、验证误差为 0.30，模型乙的训练误差为 0.08、验证误差为 0.12。若目标是对未见数据的预测，验证结果支持选择乙；乙没有把训练集拟合得最紧，但在这份验证集上的误差较小。验证误差不是对所有未来数据的保证，最终仍要在未参与调参的测试集上评估。欠拟合则是连训练数据中的主要关系也没有学好，常表现为训练与验证误差都较高。";
const REGULARIZATION_SOURCE: &str = "正则化与模型选择。L2 正则化把训练目标改为 J(w)=L_train(w)+lambda*sum_j(w_j^2)，其中 lambda>=0 控制对大权重的惩罚强度。对同一数据和损失定义，较大的 lambda 使大权重更昂贵，但并不保证验证误差单调下降。适当的惩罚可能减少模型对训练噪声的敏感程度，从而改善泛化；惩罚过强也可能使模型欠拟合。比较两个候选模型时，若 L_train 分别为 0.05、0.09，平方权重和分别为 10、1，则 lambda=0 时前者目标值 0.05 较小；lambda=0.01 时目标值分别为 0.15、0.10，后者目标值较小。这个算例说明惩罚改变优化中的偏好，不单独证明哪一个模型的验证误差更低。lambda 通常由验证集或交叉验证选择，测试集不用于反复挑选 lambda。增加正则化强度后训练误差可能上升；只要验证表现改善，这不等于模型变差。";

fn material(id: &str, text: &str) -> Book {
    let mut base = book_leaves(1).base;
    base.book_id = id.into();
    for node in &mut base.lid_nodes {
        node.span = Span { start: 0, end: text.encode_utf16().count() };
    }
    Book::new(base, text)
}

struct WorkspaceCacheFixture {
    current: Book,
    materials: Vec<(base_schema::PublishedBookRef, std::sync::Arc<Book>, &'static str)>,
    messages: Vec<Message>,
    fragments: ContextFragmentLedger,
    results: ActiveToolResultLedger,
    evidence: TurnEvidenceLedger,
    exposure: ToolExposureState,
    profile: ModelRuntimeProfile,
}

impl WorkspaceCacheFixture {
    fn new(active: bool) -> Self {
        let current = material("overfitting", OVERFITTING_SOURCE);
        let materials = vec![
            (base_schema::PublishedBookRef { book_id: "overfitting".into(), publication_id: "cache-fixture-v1".into() },
                std::sync::Arc::new(material("overfitting", OVERFITTING_SOURCE)), "过拟合与泛化"),
            (base_schema::PublishedBookRef { book_id: "regularization".into(), publication_id: "cache-fixture-v1".into() },
                std::sync::Arc::new(material("regularization", REGULARIZATION_SOURCE)), "正则化与模型选择"),
        ];
        let mut exposure = ToolExposureState::default();
        for name in ["library.list", "library.search", "library.read", "library.learning"] {
            exposure.activate(name);
        }
        if active {
            exposure.activate_tutor();
            exposure.activate("presentation.author");
        }
        let mut messages = new_session();
        messages.push(Message::system(format!(
            "Learning workspace: generalization. Authorized materials: {}. Both are authored instructional fixtures, each with one paragraph at LID 1.1. Publication identities remain distinct. Read material text before explaining; source references are registered only from actual reads.",
            json!(materials.iter().map(|(reference, _, title)| json!({"published_book_ref":reference,"title":title})).collect::<Vec<_>>())
        )));
        Self { current, materials, messages, fragments: ContextFragmentLedger::default(),
            results: ActiveToolResultLedger::default(), evidence: TurnEvidenceLedger::default(),
            exposure, profile: ModelRuntimeProfile::fallback("workspace-cache-controlled", ProviderToolProtocol::Native) }
    }

    fn append_result(&mut self, id: &str, name: &str, arguments: Value, raw: String, source: Option<&Book>) {
        let tc = call(id, name, &arguments.to_string());
        let registry = resident_tool_registry();
        let registration = registry.registration(name).unwrap();
        registration.validate_arguments(&tc.arguments).unwrap();
        let projection = self.results.project_result(name, &tc.arguments, &raw, registration.output_policy,
            source.unwrap_or(&self.current));
        let body = projection.model_body_json();
        observe_tool_evidence(&mut self.evidence, name, &projection.evidence_arguments, &body, &self.current);
        let receipt = tool_receipt(name, &projection.evidence_arguments, &body, &raw, &self.current);
        self.results.insert_call(&tc, projection.into_envelope(receipt));
        self.messages.push(Message { provider_continuation: None, role: Role::Assistant,
            content: None, tool_calls: vec![tc], tool_call_id: None });
        self.messages.push(Message { provider_continuation: None, role: Role::Tool,
            content: Some(raw), tool_calls: vec![], tool_call_id: Some(id.into()) });
    }

    fn read_sources(&mut self, sample: usize) -> Vec<String> {
        // Each sample represents a source-grounded follow-up. Re-read this turn
        // rather than making an old citation receipt stand in for observed text.
        self.evidence = TurnEvidenceLedger::default();
        let adapter = FakeAdapter::new(vec![], vec![]);
        let mut refs = Vec::new();
        for index in 0..self.materials.len() {
            let (reference, target, title) = &self.materials[index];
            let reference = reference.clone();
            let target = target.clone();
            let title = *title;
            self.evidence.references.insert(reference.clone(), ReferenceEvidence {
                source: crate::library::LibrarySource { reference: reference.clone(), book: target.clone(), title: title.into() },
                ledger: Box::default(),
            });
            let arguments = json!({"operation":"text","published_book_ref":reference,"lid":"1.1"});
            let (read, effect) = dispatch_resident_book_tool(BookToolId::Text, json!({"lid":"1.1"}), &target, &adapter);
            assert!(effect.is_none());
            let data: Value = serde_json::from_str(&read).unwrap();
            assert_eq!(data["text"].as_str(), Some(target.text("1.1", None).unwrap().as_str()));
            let raw = json!({"operation":"text","published_book_ref":reference,"data":data,
                "sources":[],"next_calls":[],"complete":true}).to_string();
            self.append_result(&format!("read-{sample}-{index}"), "library.read", arguments, raw, Some(&target));
            let quote = target.text("1.1", None).unwrap();
            let present_args = json!({"published_book_ref":reference,"quote":quote});
            let present = self.evidence.present(&self.current, &present_args.to_string()).unwrap();
            refs.push(present.source_ref_id.clone());
            self.append_result(&format!("cite-{sample}-{index}"), "source.present", present_args,
                serde_json::to_string(&present).unwrap(), None);
        }
        assert_eq!(self.evidence.bindings().len(), 2);
        assert!(self.evidence.bindings().iter().all(|binding| binding.publication_id.as_deref() == Some("cache-fixture-v1")));
        refs
    }

    fn sample(&mut self, tutor: Option<&Value>) -> AgentRequestPlan {
        let registry = resident_tool_registry().with_tutor_context(tutor);
        if let Some(tutor) = tutor {
            refresh_tutor_working_fragments(&mut self.fragments, tutor).unwrap();
            self.fragments.upsert(ContextFragment::new("agent.tutor", FragmentScope::Dynamic,
                Role::System, crate::tutor::control_instructions(tutor), FragmentSensitivity::Private)).unwrap();
        }
        let (_, plan) = build_sample_request(&self.messages, &mut self.fragments, &self.current,
            &mut self.results, None, COMPACTION_CONSUMPTION_WRAPPER, &registry, &self.profile,
            ToolPermissions::default(), &self.exposure, ArtifactExposureContext::no_overlay(),
            self.evidence.evidence_state(), &[], None, false, tutor, None, None, None).unwrap();
        for name in ["library.list", "library.search", "library.read", "library.learning", "source.present"] {
            assert!(plan.tools.iter().any(|tool| tool.name == name), "missing workspace tool {name}");
        }
        if tutor.is_some() {
            for name in ["tutor.step", "presentation.author"] {
                assert!(plan.tools.iter().any(|tool| tool.name == name), "missing active tool {name}");
            }
        }
        self.results.mark_projected_fresh_results_sampled();
        plan
    }

    fn answer(&mut self, answer: String) {
        self.messages.push(Message { provider_continuation: None, role: Role::Assistant,
            content: Some(answer), tool_calls: vec![], tool_call_id: None });
    }
}

fn tutor_state(step: usize, request: &str, book: &Book) -> Value {
    let representation = if step % 2 == 0 {
        json!({"kind":"text","reason":"先用文字完整回答当前关系和算例。"})
    } else {
        json!({"kind":"presentation","reason":"比较固定条件下训练与验证表现。",
            "observation":"Compare training and validation error; then compare loss plus penalty under fixed data."})
    };
    let source_bindings = json!([{"source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
        "start_lid":"1.1","end_lid":"1.1"}]);
    let design = json!({
        "target":{"learning_focus":"区分训练拟合、验证表现与正则化惩罚","expected_performance":"解释验证误差与训练目标为何不能混同",
            "capability":"relational_understanding","source_bindings":source_bindings,"object_refs":[]},
        "organizing_model":"把训练上的优化偏好和验证集上的检验结果分开，再连接正则化的作用。",
        "blocks":[{"block_id":"generalization","relation":"训练误差下降不保证验证误差下降；正则化改变优化偏好，验证集检验结果。",
            "purpose":"解释当前问题而不把助手讲解当成学习者掌握的证据。","source_bindings":source_bindings}],
        "focus_block_refs":["generalization"],"connections":[],"basis_refs":[],"open_question_refs":[],
        "revision_reason":request,"representation":representation});
    let typed: memory::teaching_design::TeachingDesign = serde_json::from_value(design.clone()).unwrap();
    typed.validate(&memory::teaching::TeachingBinding {
        tutor_session_id:"cache-fixture-tutor".into(),session_revision:1,control_revision:1,
        source_id:book.base.book_id.clone(),source_revision:book.source_fingerprint().into(),map_revision:None,
        chat_session_id:"cache-fixture-chat".into(),turn_id:format!("cache-fixture-turn-{step}"),
    }).unwrap();
    let pending = if step == 1 { json!(["learner:comparison-1"]) } else { json!([]) };
    let facts = if step == 0 { json!([]) } else { json!([format!("learner:question-{step}")]) };
    json!({"status":"practice","outside_current_request":false,"current_request":request,
        "agent_working_state":{"open_questions":["Which explanation resolves the learner's current confusion?"]},
        "current_design":{"design_ref":format!("design:generalization-{step}"),"design":design},
        "learner_context":{"current_request":request,"pending_assessment_refs":pending,"uninterpreted_fact_refs":facts,
            "recent_facts":if step == 0 { json!([]) } else { json!([{"event_id":format!("learner:question-{step}"),
                "usage":{"kind":"user_message"},"response_preview":{"text":request}}]) },
            "understanding":"Unconfirmed. A follow-up question and prior assistant explanation do not establish mastery."}})
}

fn recorded_scenarios() -> Vec<(AgentRequestPlan, Value)> {
    let requests = [
        "请根据工作区这两份材料，直接讲清楚：为什么训练误差越来越低，模型反而可能变差？把训练集、验证集和测试集的用途讲清楚，不要考我。",
        "我还是觉得训练误差越低越好。甲是 0.01/0.30，乙是 0.08/0.12，为什么要选乙？请先把这个数字例子完整讲明白，再说明怎样把它做成直观比较。",
        "那正则化是不是故意让模型变笨？先用文字讲清楚 L_train 加 lambda 乘平方权重和；两个模型的损失为 0.05、0.09，平方权重和为 10、1，lambda 从 0 变为 0.01 会怎样？",
        "我理解了算式，但 lambda 越大就一定越好吗？请解释过强惩罚、欠拟合和验证集选择之间的关系，并说明一个比较页面应该让我观察什么。",
    ];
    let answers = [
        "训练误差只反映模型对参与拟合的数据的表现。模型还可能把噪声当成规律，因此训练误差下降、验证误差上升时，泛化可能变差。验证集用于模型和超参数选择，测试集留作最终评估。",
        "甲的训练误差更低，但在同一验证集上误差 0.30 高于乙的 0.12；以未见数据预测为目标，这份验证结果支持乙。它不保证所有未来数据上的表现。比较时应并列展示训练和验证两列，不能只画训练误差。",
        "lambda=0 时两个目标值是 0.05 和 0.09，前者较小。lambda=0.01 时分别是 0.05+0.01×10=0.15、0.09+0.01×1=0.10，后者较小。惩罚改变优化偏好；这个算式没有给出验证误差，不能据此证明泛化改善。",
        "lambda 过大可能压制模型对主要关系的拟合，使训练和验证误差都较高。应通过验证集或交叉验证选择强度，测试集不用于反复调参。比较页面应在相同数据与指标下展示训练目标组成、训练误差和验证误差，区分优化偏好与实际验证表现。",
    ];
    let expectations = [
        json!(["train fits parameters; validation selects; test is final", "training noise can lower train error and worsen validation", "no claim of assessed mastery"]),
        json!(["prefer B on the stated validation result 0.12 versus 0.30", "same split and metric", "validation is not a future-data guarantee"]),
        json!(["lambda=0 gives 0.05 and 0.09", "lambda=0.01 gives 0.15 and 0.10", "objective preference alone does not prove validation improvement"]),
        json!(["too much regularization can underfit", "choose lambda with validation or cross-validation", "do not repeatedly tune on test set", "no claim that a presentation has already been delivered"]),
    ];
    let mut recordings = Vec::new();
    for active in [false, true] {
        let mut fixture = WorkspaceCacheFixture::new(active);
        for step in 0..4 {
            // Every follow-up starts a new Run in production. Sampling guidance
            // and the active result ledger are not persisted as raw history.
            fixture.fragments = ContextFragmentLedger::default();
            fixture.results = ActiveToolResultLedger::default();
            fixture.messages.push(Message::user(requests[step]));
            let index = recordings.len();
            let refs = fixture.read_sources(index);
            let tutor = active.then(|| tutor_state(step, requests[step], &fixture.current));
            let plan = fixture.sample(tutor.as_ref());
            let metadata = json!({"index":index,"branch":if active {"active_tutor"} else {"ordinary_control"},
                "step":step,"run_boundary":"new_run_fresh_fragments_and_active_result_ledger","request":requests[step],"representation":tutor.as_ref().map(|value|value["current_design"]["design"]["representation"]["kind"].clone()),
                "source_refs":refs,"source_bindings":fixture.evidence.bindings(),"expected_quality":expectations[step],
                "pending_assessment_refs":tutor.as_ref().map(|value|value["learner_context"]["pending_assessment_refs"].clone()),
                "uninterpreted_fact_refs":tutor.as_ref().map(|value|value["learner_context"]["uninterpreted_fact_refs"].clone())});
            recordings.push((plan, metadata));
            // Same declared, source-grounded history for both variants. These
            // are fixture explanations, not provider outputs or delivered UI.
            fixture.answer(format!("{} [[source:{}]] [[source:{}]]", answers[step], refs[0], refs[1]));
        }
    }
    recordings
}

#[test]
fn workspace_cache_record_source_grounded_followups() {
    let samples = recorded_scenarios();
    assert_eq!(samples.len(), 8);
    for (plan, metadata) in &samples {
        let serialized = serde_json::to_string(&plan.ordered_messages()).unwrap();
        assert!(serialized.contains("0.01") && serialized.contains("lambda"));
        for reference in metadata["source_refs"].as_array().unwrap() {
            assert!(serialized.contains(reference.as_str().unwrap()));
        }
    }
    if let Some(directory) = std::env::var_os("WORKSPACE_CACHE_RECORD_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        for (index, (plan, _)) in samples.iter().enumerate() {
            let (native, _) = crate::native_chat_request_projection("workspace-cache-controlled", plan);
            let react = crate::react_chat_request_projection("workspace-cache-controlled", plan);
            for (kind, body) in [("native", native), ("react", react)] {
                std::fs::write(directory.join(format!("{kind}-{index:02}.json")), serde_json::to_vec_pretty(&body).unwrap()).unwrap();
            }
        }
        let manifest = json!({"version":"workspace_cache_fixture.v1",
            "kind":"fixed_transcript_production_request_builder",
            "scope":"Production build_sample_request and Native/ReAct projection; real Book reads, tool-result projection and citation bindings. Selected materials are read directly. No live server retrieval, autonomous tool selection, paid provider call or actual presentation delivery is performed by this test.",
            "history":"Each follow-up starts with fresh sampling fragments and active result ledger, retaining only raw messages. Identical declared explanations are appended in baseline/candidate; generated provider output is evaluated separately and does not change later replay inputs. Active Tutor designs and learner-reference lists are fixed valid-shaped request-builder inputs, not claimed persisted events or delivered presentations.",
            "materials":[{"book_id":"overfitting","publication_id":"cache-fixture-v1","lid":"1.1","text":OVERFITTING_SOURCE},
                {"book_id":"regularization","publication_id":"cache-fixture-v1","lid":"1.1","text":REGULARIZATION_SOURCE}],
            "samples":samples.iter().map(|(_, metadata)|metadata).collect::<Vec<_>>()});
        std::fs::write(directory.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    }
}

#[test]
fn workspace_cache_active_tools_stay_stable_across_design_and_fact_changes() {
    let samples = recorded_scenarios();
    for range in [0..4, 4..8] {
        let baseline = serde_json::to_value(&samples[range.start].0.tools).unwrap();
        for index in range.skip(1) {
            assert_eq!(serde_json::to_value(&samples[index].0.tools).unwrap(), baseline,
                "sample {index}: a design or fact-reference change rewrote the tool definitions");
        }
    }
}
