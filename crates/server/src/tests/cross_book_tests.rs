//! Real publication, resident run, private SQLite and chat history; only the model is scripted.
use super::*;
use super::mu5_tests::Fixture;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::published_library::PublishedBookRef;

const RUST_SOURCE: &str = "# Rust 借用\n\n共享借用允许读取值而不取得所有权。借用期间，原来的所有者仍然拥有这个值。";

#[derive(Clone)]
struct Supplement {
    reference: PublishedBookRef,
    quote: String,
    user_quote: String,
    step: Arc<AtomicUsize>,
    messages: Arc<Mutex<Vec<Vec<Message>>>>,
}
impl ModelAdapter for Supplement {
    fn complete(&self, _:CompletionRequest)->Result<ParsedResponse,AdapterError> {
        Err(AdapterError {spend_stop:None,message:"unused fixture completion".into()})
    }
    fn chat(&self, plan:&AgentRequestPlan)->Result<AssistantTurn,AdapterError> {
        let messages=plan.ordered_messages();
        self.messages.lock().unwrap().push(messages.clone());
        let raw_step=self.step.fetch_add(1,Ordering::SeqCst);
        let result=|id:&str| -> Value {
            let value:Value=messages.iter().rev().find(|m|m.tool_call_id.as_deref()==Some(id))
                .and_then(|m|m.content.as_deref()).and_then(|s|serde_json::from_str(s).ok()).unwrap_or(Value::Null);
            value.get("model_body").cloned().unwrap_or(value)
        };
        // Walk three real catalog pages before reading. Valid pagination must not
        // trigger the no-progress guard for current-book locators and evidence.
        if matches!(raw_step, 1 | 2) {
            let args=if raw_step==1 {json!({"limit":1})} else {result("cb-catalog-1")["next_calls"][0]["arguments"].clone()};
            assert!(args.is_object());
            return Ok(AssistantTurn {provider_continuation:None,text:None,tool_calls:vec![runtime::ToolCall {
                id:format!("cb-catalog-{raw_step}"),name:"library.list".into(),arguments:args.to_string()}],usage_total_tokens:Some(5)});
        }
        let step=if raw_step>=3 {raw_step-2}else{raw_step};
        let tool=|name:&str,args:Value|Ok(AssistantTurn {provider_continuation:None,text:None,
            tool_calls:vec![runtime::ToolCall {id:format!("cb-{step}"),name:name.into(),arguments:args.to_string()}],usage_total_tokens:Some(5)});
        match step {
            0 => tool("tool.search",json!({"task":"Find supplementary reference-book source material for a knowledge gap", "required_capabilities":["source_read"],"scope":"corpus","operation":"read_source","effect_mode":"read_only","max_results":3})),
            1 => tool("library.list",result("cb-catalog-2")["next_calls"][0]["arguments"].clone()),
            2 => tool("library.read",json!({"operation":"search_text","published_book_ref":self.reference,"query":"共享借用"})),
            3 => {
                let found=result("cb-2");
                assert!(found["data"]["occurrences"].as_array().is_some_and(|items|!items.is_empty()),"{found}");
                tool("library.read",json!({"operation":"text","published_book_ref":self.reference,"lid":found["data"]["occurrences"][0]["start_lid"]}))
            },
            4 => tool("library.learning",json!({"operation":"read","published_book_ref":self.reference})),
            5 => tool("source.present",json!({"published_book_ref":self.reference,"quote":self.quote})),
            6 if !self.user_quote.is_empty() => {
                let source=result("cb-5");
                assert!(source["source_ref_id"].is_string(),"{source}");
                let prior=result("cb-4");
                assert!(prior["interpretations"].is_array(),"{prior}");
                let mut args=json!({"operation":"observe","published_book_ref":self.reference,"operation_id":"borrow-feedback",
                    "source_ref_ids":[source["source_ref_id"]],"learner_quote":self.user_quote,"learning_focus":"共享借用与所有权",
                    "expected_performance":"说明借用时谁拥有值","capability":"explanation","interpretation":format!("用户本轮表达：{}",self.user_quote),
                    "teaching_implication":"用所有者和只读引用的关系补讲，并保留尚未验证的部分。"});
                if let Some(previous)=prior["interpretations"].as_array().and_then(|items|items.first()) { args["supersedes"]=previous["evidence_id"].clone(); }
                tool("library.learning",args)
            },
            6 | 7 => {
                if step==7 { assert!(result("cb-6")["evidence_ref"].is_string(),"{}",result("cb-6")); }
                let reference=result("cb-5")["source_ref_id"].as_str().unwrap().to_owned();
                Ok(AssistantTurn {provider_continuation:None,text:Some(format!("共享借用允许读取，所有权仍在原所有者手中。[[source:{reference}]]")),tool_calls:vec![],usage_total_tokens:Some(5)})
            },
            _ => panic!("unexpected extra model step {step}: {}",json!(messages)),
        }
    }
}

fn publish_rust(f:&Fixture)->PublishedBookRef {
    let dir=tempfile::tempdir().unwrap();
    let mut base=multi_leaf_base("cross-book-rust",1);
    for node in &mut base.lid_nodes {node.span.end=RUST_SOURCE.encode_utf16().count();}
    let book=Book::new(base,RUST_SOURCE);
    super::tutor_tests::t15_foundation(dir.path(),&book);
    std::fs::write(dir.path().join("source.txt"),RUST_SOURCE).unwrap();
    super::tutor_tests::t15_close(dir.path(),&book,"pass1",&["base.json","source.txt"]);
    let mut library=f.access.library.lock().unwrap();
    let reference=library.publish(dir.path()).unwrap().reference;
    library.grant("A",&reference).unwrap();
    reference
}

fn supplement(f:&Fixture,w:&Value,reference:&PublishedBookRef,message:&str,key:&str,extra:Value)->(Value,Supplement) {
    let probe=Supplement {reference:reference.clone(),quote:RUST_SOURCE.into(),user_quote:message.into(),
        step:Arc::new(AtomicUsize::new(0)),messages:Arc::new(Mutex::new(vec![]))};
    let copy=probe.clone();
    f.access.runs.configure_fake(move ||Box::new(copy.clone()));
    let mut input=Fixture::stamp(w,"page");
    input["session_id"]=w["selected_chat"].clone();input["client_request_id"]=json!(key);input["message"]=json!(message);
    input.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let (status,receipt)=f.call(&f.a,"POST",&format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap()),input);
    assert_eq!(status,202,"{receipt}");
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let now=crate::multi_user_host::now();
    let context=f.access.context(f.access.auth.authenticate(&f.a,now).unwrap(),now).unwrap();
    if f.access.runs.status(&context,receipt["turn_id"].as_str().unwrap()).unwrap()["persistence_state"]=="failed" {
        f.access.runs.retry_save(&context,receipt["turn_id"].as_str().unwrap()).unwrap();
        panic!("initial delivery save must succeed");
    }
    let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
    let user=handle.lock().unwrap();
    let saved=user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|json!(t.turn_id)==receipt["turn_id"]).unwrap();
    assert_eq!(saved.status,AgentAssistantStatus::Completed,"{}",json!(saved));
    assert_eq!(saved.source_bindings.len(),1,"{}",json!(saved));
    assert_eq!(saved.source_bindings[0].book_id,reference.book_id);
    assert_eq!(saved.source_bindings[0].publication_id.as_deref(),Some(reference.publication_id.as_str()));
    (receipt,probe)
}

#[test]
fn cross_book_resident_loop_preserves_main_task_references_learning_and_followups() {
    let f=Fixture::new();
    let rust=publish_rust(&f);
    let w=f.create(&f.a,&f.x,"page");
    let w=f.action(&f.a,&w,"page","chat/new",json!({}));
    let (first,_)=supplement(&f,&w,&rust,"我不懂共享借用，所有权去了哪里？","cross-first",json!({}));
    let current=f.get(&f.a,&w);
    assert_eq!(current["reader"],w["reader"]);assert_eq!(current["selected_chat"],w["selected_chat"]);assert_eq!(current["published_book_ref"],json!(f.x));
    let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
    let source=handle.lock().unwrap().agent_history.sessions[0].turns[0].source_bindings[0].clone();
    assert!(source.label_snapshot.contains("Rust 借用"));
    let resolve=f.action(&f.a,&current,"page","agent/source.resolve",json!({"turn_id":first["turn_id"],"source_ref_id":source.source_ref_id}));
    assert_eq!(resolve["result"]["highlighted_quote"],RUST_SOURCE);
    let window = f.action(&f.a, &current, "page", "agent/source.read", json!({
        "source": {"turn_id": first["turn_id"], "source_ref_id": source.source_ref_id},
        "include_outline": false
    }));
    assert_eq!(window["result"]["published_book_ref"], json!(rust));
    assert!(window["result"]["segments"].as_array().unwrap().iter()
        .any(|segment| !segment["highlights"].as_array().unwrap().is_empty()));
    assert_eq!(window["reader"], current["reader"]);
    assert_eq!(window["revision"], current["revision"]);
    let opened=f.action(&f.a,&current,"page","agent/source.open",json!({"turn_id":first["turn_id"],"source_ref_id":source.source_ref_id}));
    assert_eq!(opened["result"]["opened"],false);assert_eq!(opened["reader"],current["reader"]);
    assert!(opened["result"]["source_url"].as_str().unwrap().contains(&rust.publication_id));
    let (second,follow)=supplement(&f,&current,&rust,"所以共享借用只读取，原所有者依然拥有值。","cross-feedback",json!({}));
    let initial=json!(follow.messages.lock().unwrap()[0]).to_string();
    assert!(initial.contains("Reference materials from delivered answers"));assert!(initial.contains(&rust.publication_id));
    let publication=f.access.library.lock().unwrap().load("A",&rust).unwrap();
    {
        let user=handle.lock().unwrap();
        let understanding=crate::teaching::understanding(&crate::PrivateBookContext { learning_workspace_id:None,user:&user,book:&publication.book,book_dir:publication.directory(),messages:&[],selected_chat:None},&rust.book_id,None,0).unwrap();
        let interpreted=understanding["interpretations"].as_array().unwrap();
        assert_eq!(interpreted.len(),1);assert_eq!(interpreted[0]["learner_quote"],"所以共享借用只读取，原所有者依然拥有值。");
        assert_eq!(interpreted[0]["assistance_refs"].as_array().unwrap().len(),1);
        assert_eq!(interpreted[0]["status"],"unassessed");
        let (history,_)=crate::session_store::load_chat_storage(&user.history_path).unwrap();
        assert_eq!(history.sessions[0].turns[0].source_bindings[0],source);
    }
    let current=f.get(&f.a,&w);
    // UI continuation without authored feedback keeps the reference, but creates no new understanding claim.
    let (continuation,continued)=supplement(&f,&current,&rust,"","cross-continue",json!({"entry_action":"continue_answer",
        "answer_ref":{"turn_id":second["turn_id"]},"expected_chat_head":second["turn_id"]}));
    assert_eq!(continued.step.load(Ordering::SeqCst),9);
    let user=handle.lock().unwrap();
    assert_eq!(user.learning_store().unwrap().current_interpretations(&rust.book_id,publication.book.source_fingerprint(),0,10).unwrap().len(),1);
    drop(user);
    let current=f.get(&f.a,&w);
    let clarification="我还是不清楚借用期间能不能修改值。";
    supplement(&f,&current,&rust,clarification,"cross-clarify",json!({"entry_action":"clarify_answer",
        "answer_ref":{"turn_id":second["turn_id"]},"expected_chat_head":continuation["turn_id"]}));
    let records=handle.lock().unwrap().learning_store().unwrap()
        .current_interpretations(&rust.book_id,publication.book.source_fingerprint(),0,10).unwrap();
    assert_eq!(records.len(),1);
    assert_eq!(records[0].1.learner_quote,clarification);
    assert_eq!(records[0].1.assistance_refs.len(),3);
    let bob=f.access.users.lock().unwrap().get("B","now").unwrap();
    assert!(bob.lock().unwrap().learning_store().unwrap().current_interpretations(&rust.book_id,publication.book.source_fingerprint(),0,10).unwrap().is_empty());
    f.access.library.lock().unwrap().revoke("A",&rust).unwrap();
    let current=f.get(&f.a,&w);
    let mut input=Fixture::stamp(&current,"page");input["turn_id"]=first["turn_id"].clone();input["source_ref_id"]=json!(source.source_ref_id);
    assert_ne!(f.call(&f.a,"POST",&format!("/api/workspaces/{}/agent/source.resolve",w["workspace_id"].as_str().unwrap()),input).0,200);
    assert_eq!(f.get(&f.a,&w)["reader"],w["reader"]);
}
