//! Opt-in real provider evaluation: identical question/material, different personal starting points.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const SOURCE:&str="Transformer 推理时，模型的权重矩阵通常保持固定。输入词元的初始嵌入经过注意力层和前馈层变成上下文化表示。注意力分数与输入序列有关，同一组权重处理不同句子会产生不同的激活。因此同一个词在不同上下文中的隐藏向量可以变化，并不表示权重在这一次推理中被重新训练。类比固定函数 f(x)=2x，系数 2 不变，输入不同仍会有不同输出。训练时优化器根据梯度更新权重，与普通推理要区别。";
pub const QUESTION:&str="同一个词在不同句子里的向量为什么不同？请根据我已有的表达选择起点，用纯文字解释，不生成演示也先不要出题。";
pub struct Captured {
    inner: Box<dyn ModelAdapter + Send>,
    directory: PathBuf,
    sequence: AtomicUsize,
}
impl Captured {
    fn capture(&self, kind: &str, input: Value) -> Result<usize, AdapterError> {
        let n = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        if n > 32 {
            return Err(AdapterError {
                spend_stop: None,
                message: "PN live evaluation reached its 32-call limit".into(),
            });
        }
        std::fs::write(
            self.directory.join(format!("request-{n:02}.json")),
            serde_json::to_vec_pretty(&json!({"kind":kind,"input":input})).unwrap(),
        )
        .unwrap();
        Ok(n)
    }
}
impl ModelAdapter for Captured {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn projection_identity(&self) -> Option<String> {
        self.inner.projection_identity()
    }
    fn tool_update_mode(&self) -> Option<runtime::deepseek_messages::ToolUpdateMode> {
        self.inner.tool_update_mode()
    }
    fn set_run_cancellation(&self, t: runtime::run_context::CancellationToken) {
        self.inner.set_run_cancellation(t)
    }
    fn set_model_purpose(&self, p: &str) {
        self.inner.set_model_purpose(p)
    }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.capture("complete", json!(request))?;
        self.inner.complete(request)
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        self.capture("structured", json!(request))?;
        self.inner.complete_structured(request)
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let n = self.capture("chat", json!({"messages":plan.ordered_messages()}))?;
        let result = self.inner.chat(plan)?;
        std::fs::write(
            self.directory.join(format!("response-{n:02}.json")),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        Ok(result)
    }
}
#[test]
#[ignore = "PN real provider comparison; uses configured credentials, maximum 32 model calls"]
fn personal_notes_real_agent_starting_points() {
    let directory = PathBuf::from(
        std::env::var("PN_LIVE_OUTPUT")
            .expect("PN_LIVE_OUTPUT must name the evaluation output directory"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let config = ProviderConfig::from_env().expect("Configure the normal model provider first");
    let adapter = Captured {
        inner: runtime::ProviderRegistry::adapter_from_config(config.clone()),
        directory: directory.clone(),
        sequence: AtomicUsize::new(0),
    };
    if std::env::var("PN_LIVE_CASE").as_deref() == Ok("personal") {
        let (_root, mut state) = super::tutor_loop_tests::fixture_with_book(
            false,
            Book::new(sample_base(), "XXXYYY"),
            "XXXYYY",
        );
        super::session_runtime_tests::enable_jsonl(&mut state);
        let note=state.user.store.create_personal_note(serde_json::from_value(json!({"content":"我觉得多读几遍就说明理解了，但一到解释给别人听就卡住。","authorship":"user"})).unwrap(),"thought","1").unwrap();
        let chat = crate::personal_chat::create(&mut state.user, "2").unwrap();
        let prepared=crate::personal_chat::prepare(&mut state,&json!({"session_id":chat["session_id"],"note_refs":[{"note_id":note.note_id,"revision":1}],"message":"请从这条想法出发，和我讨论重复阅读与能否解释的区别。请给一个小例子，不查外部资料。","response_language":"zh-CN"}).to_string(),"3").unwrap();
        assert!(prepared.scope.book.is_none());
        let start = std::time::Instant::now();
        let result = agent_run::execute_prepared(
            &agent_run::BorrowedAppPort(RefCell::new(&mut state)),
            &adapter,
            prepared,
            Default::default(),
        );
        let reply: Value = serde_json::from_str(&result.reply.body).unwrap();
        std::fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&json!({"case":"personal","model":config.model,"status":result.reply.status,"calls":adapter.sequence.load(Ordering::SeqCst),"elapsed_ms":start.elapsed().as_millis(),"reply":reply})).unwrap()).unwrap();
        assert_eq!(result.reply.status, 200, "{}", result.reply.body);
        let session = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| Some(session.id.as_str()) == chat["session_id"].as_str())
            .unwrap();
        assert!(
            !session.turns[0].domain.personal_notes.as_ref().unwrap()["references"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        return;
    }
    let mut reports = vec![];
    for (name,thought,correction) in [
        ("confused","同一个词向量应该固定；上下文变了向量却变了，难道模型在每句话里重新训练参数吗？",None),
        ("clear","参数不变而激活随输入改变，就像固定函数。词向量的不同来自上下文，而不是推理时更新权重。",None),
        ("corrected","我原来觉得同一个词在不同句子里改变向量，说明推理也在更新权重。",Some("现在我已经区分参数和激活：权重固定，激活依赖这次输入。但还想知道注意力具体怎样利用上下文。")),
        ("unrelated","我的番茄炖牛腩要先煎香，再加入番茄慢慢炖。",None),
    ] {
        if std::env::var("PN_LIVE_CASE").is_ok_and(|selected|selected!=name){continue;}
        let mut base=sample_base();base.book_id=format!("pn-live-{name}");base.graph_nodes.clear();base.graph_edges.clear();
        for node in &mut base.lid_nodes {node.span=Span{start:0,end:SOURCE.encode_utf16().count()};}
        let (_root,mut state)=super::tutor_loop_tests::fixture_with_book(false,Book::new(base,SOURCE),SOURCE);
        state.services.adapter=runtime::ProviderRegistry::adapter_from_config(config.clone());
        super::session_runtime_tests::enable_jsonl(&mut state);
        let value=serde_json::from_value(json!({"content":thought,"authorship":"user","context":{"book_id":"previous-material","label":"另一份资料中的想法"},"source_refs":[]})).unwrap();
        let note=state.user.store.create_personal_note(value,name,"1").unwrap();
        if let Some(text)=correction {state.user.store.record_note_discussion(memory::NoteDiscussion{reference:memory::NoteRef{note_id:note.note_id.clone(),revision:Some(1)},turn:memory::NoteTurnRef{session_id:"earlier-discussion".into(),turn_id:"correction-turn".into()},created_at:"2".into(),expression:Some(text.into())}).unwrap();}
        route_agent_new(&mut state,"3");
        let prepared=prepare_agent_chat(&mut state,&json!({"message":QUESTION,"response_language":"zh-CN"}).to_string(),"4").unwrap_or_else(|r|panic!("{}",r.body));
        let started=std::time::Instant::now();
        let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,prepared,Default::default());
        let reply:Value=serde_json::from_str(&result.reply.body).unwrap();
        reports.push(json!({"case":name,"thought":thought,"correction":correction,"note_id":note.note_id,"elapsed_ms":started.elapsed().as_millis(),"status":result.reply.status,"reply":reply}));
        std::fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&json!({"question":QUESTION,"source":SOURCE,"model":config.model,"calls":adapter.sequence.load(Ordering::SeqCst),"cases":reports})).unwrap()).unwrap();
        assert_eq!(result.reply.status,200,"case {name}: {}",result.reply.body);
        if name=="unrelated" {
            assert!(!reply["answer"].as_str().unwrap_or_default().contains("牛腩"),"Unrelated personal data must not become a forced analogy");
            for trace in reply["trace"].as_array().unwrap() {if trace["tool"]=="tutor.step" {let args:Value=serde_json::from_str(trace["args"].as_str().unwrap()).unwrap();for key in ["design","move"] {assert!(args[key]["note_refs"].as_array().is_none_or(|refs|refs.is_empty()),"Unrelated note adopted: {args}");}}}
        }
    }
}
