// Included in orchestrator::tests to exercise the real tool dispatcher and projections.
mod music_dispatch {
    use super::*;
    struct Port { store:MemoryStore,reader:Reader,player_ms:f64,paused:bool,loop_range:Option<crate::music::MusicLoop>,calls:Vec<String>,question_image:Option<base_schema::VideoQuestionImage> }
    fn source() -> read_tools::music::MusicSourceSnapshot {
        serde_json::from_value(serde_json::json!({"target":{"publication":{"book_id":"b","publication_id":"music-pub1"},
            "audio":{"recording":{"recording_id":"rec","revision":"1"},"start_ms":1000,"end_ms":2000},"score":null,
            "occurrence_ids":["repeat2"],"event_ids":["theme"],"provenance_ids":["curated"]},
            "title":"Theme","label":"Theme at 00:01","preview":"Prepared analysis","material":{"kind":"score","text":"The theme returns"}})).unwrap()
    }

#[test]
fn music_available_answer_keeps_actual_material_and_applied_playback() {
    let b=book();
    let adapter=RequestPlanRecordingAdapter::new(vec![turn_final("The original recording was started. The prepared score identifies the returning theme; the full explanation remains unfinished.")],vec![]);
    let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
    let music=source();context.evidence_ledger.music_observed.push(music.clone());
    let interaction=serde_json::json!({"request":{"operation":"act","action":{"kind":"play","start_ms":1000,"end_ms":2000}},
        "result":{"receipt":{"status":"applied","paused":false,"current_ms":1001,"publication":music.target.publication,"recording":music.target.audio.recording},"heard_audio":false}});
    context.music_interactions.push(interaction.clone());
    let result=delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(),"Play this theme",&memory::teaching::invalid("Teaching delivery remains unfinished"),None).unwrap();
    assert!(result.incomplete);assert!(result.teaching_delivery_move_refs.is_empty());
    let plans=adapter.seen_plans.borrow();assert_eq!(plans.len(),1);assert!(plans[0].tools.is_empty());
    let payload:serde_json::Value=serde_json::from_str(plans[0].ordered_messages()[1].content.as_deref().unwrap()).unwrap();
    assert_eq!(payload["read_evidence"][0]["music"],serde_json::json!(music));
    assert_eq!(payload["music_interactions"],serde_json::json!([interaction]));
    assert_eq!(payload["read_evidence"][0]["heard_audio"],false);
    assert!(payload["read_evidence"][0]["source_ref_id"].as_str().unwrap().starts_with("src_"));
    assert_eq!(context.evidence_ledger.bindings()[0].music,Some(music));
}

    impl ResidentStatePort for Port {
        fn music_source_context(&mut self)->Result<Option<serde_json::Value>,ToolError> {
            Ok(Some(serde_json::json!({"publication":source().target.publication,"recording":source().target.audio.recording,
                "question_context":{"draft_id":"frozen-question","audio":source().target.audio,"occurrence_id":"repeat2","image":self.question_image},"player_ms":self.player_ms})))
        }
        fn read_music(&mut self,_:Option<&base_schema::PublishedBookRef>,request:read_tools::music::MusicRead)->Result<read_tools::music::MusicReadResult,ToolError> {
            self.calls.push("read".into());assert_eq!(request.view,"events");let source=source();
            Ok(read_tools::music::MusicReadResult{publication:source.target.publication.clone(),recording:source.target.audio.recording.clone(),observation_kind:"prepared_music_material".into(),heard_audio:false,body:serde_json::json!({"view":"events"}),sources:vec![source]})
        }
        fn interact_music(&mut self,request:crate::music::MusicInteractRequest,_:&crate::run_context::CancellationToken)->Result<crate::music::MusicPlaybackReceipt,ToolError> {
            use crate::music::*;self.calls.push("interact".into());
            let status=match &request {MusicInteractRequest::Observe{..}=>MusicReceiptStatus::Observed,MusicInteractRequest::Act{based_on,action,..}=>{
                assert_eq!(based_on,"actual-state-1");match action { MusicAction::Loop{start_ms,end_ms} => { self.loop_range=Some(MusicLoop{start_ms:*start_ms,end_ms:*end_ms});self.player_ms=*start_ms; }, MusicAction::Play{..}=>{self.player_ms=1500.0;self.paused=false;}, _=>panic!("unexpected music action") };MusicReceiptStatus::Applied}};
            Ok(MusicPlaybackReceipt{receipt_id:"actual-state-1".into(),command_id:"browser-command".into(),publication:request.publication().clone(),recording:request.recording().clone(),status,
                current_ms:self.player_ms,paused:self.paused,playback_rate:1.0,loop_range:self.loop_range.clone(),focused_part_ids:vec![],focused_voices:vec![],occurrence_id:Some("repeat2".into()),demonstration_id:None,demonstration_state:None,displayed_cue:None,error:None})
        }
        fn submit_private<R>(&mut self,f:impl FnOnce(&mut MemoryStore)->R)->Result<R,ToolError>{Ok(f(&mut self.store))}
        fn read_live_reader<R>(&mut self,f:impl FnOnce(&Reader)->R)->Result<R,ToolError>{Ok(f(&self.reader))}
        fn apply_reader<R>(&mut self,f:impl FnOnce(&mut MemoryStore,&mut Reader)->R)->Result<R,ToolError>{Ok(f(&mut self.store,&mut self.reader))}
        fn reader_input(&mut self,b:&Book,q:&str)->crate::run_context::ReaderInputSnapshot{crate::run_context::ReaderInputSnapshot::capture(b,&self.reader,q)}
    }
    #[test]
    fn music_question_image_reaches_first_and_finalization_model_requests_as_pixels() {
        let b = book();
        let image = base_schema::VideoQuestionImage { png_base64:"music-question-pixels".into(), width:800, height:240, origin:base_schema::VideoQuestionImageOrigin::Selection };
        let mut port = Port { store:MemoryStore::open(tmp("music-question-image")).unwrap(),reader:Reader::new(&b,1),player_ms:1200.0,paused:true,loop_range:None,calls:vec![],question_image:Some(image.clone()) };
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_calls(vec![call("read","music.read",r#"{"view":"events"}"#)]), turn_final("The five horizontal lines form a staff.")],vec![]);
        let profile = default_profile_snapshot(&b,&port.store,"t0");
        let mut context = RunContext::new(new_session(),OuterConfig { max_turns:Some(1),..Default::default() },adapter.model_runtime_profile());
        run_context(&b,&mut port,&adapter,&mut context,&profile,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"What do these lines mean?","t0").unwrap();
        let plans = adapter.seen_plans.borrow();
        assert_eq!(plans.len(),2);
        for plan in plans.iter() {
            assert_eq!(plan.question_image.as_ref(),Some(&image));
            let messages = serde_json::to_string(&plan.ordered_messages()).unwrap();
            assert!(!messages.contains("music-question-pixels"),"PNG bytes must not be serialized as prompt text");
            assert!(messages.contains("attached_image"));
            let body = crate::native_chat_request_projection("fixture",plan).0;
            assert!(body["messages"].as_array().unwrap().iter().any(|m| m["content"].as_array().is_some_and(|parts|
                parts.iter().any(|part|part["image_url"]["url"] == "data:image/png;base64,music-question-pixels"))));
        }
        assert!(plans[1].tools.is_empty());
        assert!(context.music_interactions.is_empty(),"Notation questions must not require audio playback");
    }
    #[test]
    fn music_runtime_reads_cites_and_waits_for_actual_receipt_with_frozen_question() {
        let b=book();let mut port=Port{store:MemoryStore::open(tmp("music-runtime" )).unwrap(),reader:Reader::new(&b,1),player_ms:1200.0,paused:true,loop_range:None,calls:vec![],question_image:None};
        let target=source().target;
        let observe=serde_json::json!({"operation":"observe","publication":target.publication,"recording":target.audio.recording});
        let act=serde_json::json!({"operation":"act","publication":target.publication,"recording":target.audio.recording,"based_on":"actual-state-1","action":{"kind":"play","start_ms":1000,"end_ms":2000}});
        let mut looping=act.clone();looping["action"]=serde_json::json!({"kind":"loop","start_ms":1000,"end_ms":2000});
        let turns=vec![turn_calls(vec![call("read","music.read",r#"{"view":"events"}"#)]),
            turn_calls(vec![call("source","source.present",&serde_json::json!({"music_target":target}).to_string()),call("discover","tool.search",r#"{"task":"music.interact 播放 伴听","required_capabilities":["presentation_interaction"],"scope":"passage","operation":"compare","effect_mode":"read_only","max_results":2}"#)]),
            turn_calls(vec![call("observe","music.interact",&observe.to_string())]),turn_calls(vec![call("loop","music.interact",&looping.to_string())]),turn_calls(vec![call("play","music.interact",&act.to_string())]),turn_final("Listen to the returning theme.")];
        let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);let snapshot=default_profile_snapshot(&b,&port.store,"t0");
        let mut context=RunContext::new(new_session(),OuterConfig { max_turns:Some(8),..Default::default() },adapter.model_runtime_profile());
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,&mut EphemeralCompactionCheckpointSink::default(),"Play this passage and explain the returning theme","t0").unwrap();
        assert!(!outcome.incomplete,"Actual observe/loop/play must not be mistaken for stalled tool batches");
        assert_eq!(port.calls,vec!["read","interact","interact","interact"]);
        assert!(context.media_observations.is_empty());assert!(context.audio_reviews.is_empty());
        assert_eq!(context.music_interactions.len(),3);
        assert_eq!(context.music_interactions[0]["result"]["receipt"]["status"],"observed");
        assert_eq!(context.music_interactions[2]["request"]["action"]["kind"],"play");
        assert_eq!(context.music_interactions[1]["result"]["receipt"]["status"],"applied");
        assert_eq!(context.music_interactions[1]["result"]["heard_audio"],false);
        assert_eq!(context.evidence_ledger.bindings()[0].music.as_ref().unwrap().target.occurrence_ids,vec!["repeat2"]);
        let plans=adapter.seen_plans.borrow();
        assert!(plans[0].tools.iter().any(|tool|tool.name=="music.read"));
        assert!(plans[0].tools.iter().any(|tool|tool.name=="music.interact"),"Current music playback is an immediately available capability");
        for pair in plans.windows(2) {
            assert!(!ActiveContextBudget::from_plan(&pair[1]).over_high_watermark,
                "This regression covers ordinary music steps below context pressure");
            let prior = crate::native_chat_request_projection("fixture", &pair[0]).0;
            let current = crate::native_chat_request_projection("fixture", &pair[1]).0;
            assert!(current["messages"].as_array().unwrap().starts_with(prior["messages"].as_array().unwrap()),
                "A playback or teaching state update must append after the entire prior sent prefix");
        }
        let latest_messages=plans.last().unwrap().ordered_messages();
        let snapshots=latest_messages.iter().filter_map(|message|message.content.as_deref())
            .filter(|text|text.starts_with("runtime_state_snapshot.v1\n")).collect::<Vec<_>>();
        assert!(snapshots.len()>1,"Earlier decisions stay available until actual pressure");
        let latest=snapshots.last().unwrap();
        assert!(latest.contains("The latest active_keys manifest replaces earlier selections"));
        assert!(latest.contains("Tool-loop sampling 6 of 8."),"The newest budget body remains authoritative");
        assert!(snapshots[0].contains("Tool-loop sampling 1 of 8."),"Prior state is retained as history, not reapplied");
        let last=serde_json::to_string(&plans.last().unwrap().ordered_messages()).unwrap();
        assert!(last.contains("actual-state-1"));assert!(last.contains("frozen-question"));
        assert!(last.contains("prepared_music_material"));
        let contexts:Vec<_>=plans.iter().flat_map(|plan|plan.ordered_messages()).filter_map(|message|message.content.clone()).filter(|text|text.contains("Music listening context")).collect();
        assert!(contexts.iter().all(|text|text.contains("1200")),"Question context must not move to the player result");
    }
}


