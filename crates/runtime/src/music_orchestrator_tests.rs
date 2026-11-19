// Included in orchestrator::tests to exercise the real tool dispatcher and projections.
mod music_dispatch {
    use super::*;
    use serde_json::json;
    struct Port { store:MemoryStore,reader:Reader,player_ms:f64,paused:bool,loop_range:Option<crate::music::MusicLoop>,calls:Vec<String>,question_image:Option<base_schema::VideoQuestionImage>,manifest:Option<base_schema::MusicSourceManifest> }
    fn source() -> read_tools::music::MusicSourceSnapshot {
        serde_json::from_value(serde_json::json!({"target":{"publication":{"book_id":"b","publication_id":"music-pub1"},
            "audio":{"recording":{"recording_id":"rec","revision":"1"},"start_ms":1000,"end_ms":2000},"score":null,
            "occurrence_ids":["repeat2"],"event_ids":["theme"],"provenance_ids":["curated"]},
            "title":"Theme","label":"Theme at 00:01","preview":"Prepared analysis","material":{"kind":"score","text":"The theme returns"}})).unwrap()
    }

    fn manifest(count: usize) -> base_schema::MusicSourceManifest {
        let mut manifest:base_schema::MusicSourceManifest = serde_json::from_str(include_str!("../../base-schema/tests/fixtures/music.json")).unwrap();
        manifest.book_id = source().target.publication.book_id;
        manifest.recording.source = source().target.audio.recording;
        let prototype = manifest.score_objects[0].clone();
        manifest.score_objects = (0..count).map(|i| {
            let mut note = prototype.clone(); note.source_id = format!("note{i}");
            note.svg_element_id = note.source_id.clone(); note.measure = (i / 8 + 1).to_string();
            note.beat = (i % 8) as f64 * 0.25; note.duration_beats = 0.25; note
        }).collect();
        manifest.recording.duration_ms = count.div_ceil(8) as f64 * 2000.0;
        manifest.occurrences = manifest.score_objects.chunks(8).enumerate().map(|(i, notes)| base_schema::MusicOccurrence {
            occurrence_id:format!("bar{i}"),order:i as u32,measure:(i+1).to_string(),start_ms:i as f64*2000.0,end_ms:(i+1) as f64*2000.0,
            score_object_ids:notes.iter().map(|n|n.source_id.clone()).collect(),
        }).collect();
        manifest.note_alignments.clear();manifest.coverage.clear();manifest.structure.clear();
        let event = manifest.events[0].clone();
        manifest.events = (0..3).map(|i| {
            let mut event = event.clone();event.event_id = format!("event{i}");event.title="Example theme".into();
            event.relations.clear();event.evidence[0].score_object_ids=vec!["note0".into()];event
        }).collect();
        manifest.validate().unwrap();
        manifest
    }

    fn music_port(book:&Book, name:&str, manifest:base_schema::MusicSourceManifest) -> Port {
        Port { store:MemoryStore::open(tmp(name)).unwrap(),reader:Reader::new(book,1),player_ms:0.0,paused:true,loop_range:None,calls:vec![],question_image:None,manifest:Some(manifest) }
    }

    #[test]
    fn music_score_byte_budget_keeps_complete_measures_and_unread_notes_out_of_evidence() {
        let b=book(); let manifest=manifest(16);
        let request:read_tools::music::MusicRead=serde_json::from_value(json!({"view":"score","limit":16})).unwrap();
        let raw=read_tools::music::read(&manifest,&source().target.publication,&request).unwrap();
        let prefix=read_tools::music::crop_score_result(&raw,1).unwrap();
        let budget=serde_json::to_vec(&prefix).unwrap().len();
        let projected=crate::music::project_read(&serde_json::to_string(&request).unwrap(),&serde_json::to_value(&raw).unwrap(),budget).unwrap();
        assert!(projected.truncated);
        assert_eq!(projected.model_body["body"]["returned_count"],8);
        assert_eq!(projected.model_body["body"]["next_offset"],8);
        assert_eq!(projected.model_body["body"]["range_complete"],false);
        let mut ledger=TurnEvidenceLedger::default();
        observe_tool_evidence(&mut ledger,"music.read","{}",&projected.model_body.to_string(),&b);
        assert_eq!(crate::music_example::objects(&ledger.music_observed[0]).len(),8);
        assert!(!ledger.music_observed[0].target.score.as_ref().unwrap().source_ids.contains(&"note8".into()));
        assert_eq!(ledger.music_observed[0].target.occurrence_ids,vec!["bar0"]);
        assert!(ledger.present_music(&raw.sources[0].target,"rev").is_err());
    }

    #[test]
    fn music_comparison_facts_compile_with_both_sources_and_no_hand_copied_values() {
        let b=book(); let manifest=manifest(16);
        let mut port=music_port(&b,"music-facts",manifest.clone());
        let mut ledger=TurnEvidenceLedger::default();
        let request:read_tools::music::MusicRead=serde_json::from_value(json!({"view":"score","limit":16})).unwrap();
        let read=read_tools::music::read(&manifest,&source().target.publication,&request).unwrap();
        observe_tool_evidence(&mut ledger,"music.read","{}",&serde_json::to_string(&read).unwrap(),&b);
        let args=json!({"items":[{"kind":"comparison","key":"same","left":{"part_id":"piano","voice":"1","from_id":"note0","through_id":"note7"},"right":{"part_id":"piano","voice":"1","from_id":"note8","through_id":"note15"}}]}).to_string();
        resident_tool_registry().registration("music.example").unwrap().validate_arguments(&args).unwrap();
        let result=register_music_examples(&args,&mut ledger,&mut port,"rev").unwrap();
        assert_eq!(result["items"][0]["facts"]["data"]["rhythm_equal"],true,"{result}");
        let mut provenance=AnswerProvenanceLedger::default();
        provenance.observe_tool_result("music.example",&result.to_string());
        let raw=format!("两段可逐项比较。\n\n{}\n\n听 {}",result["items"][0]["facts_marker"].as_str().unwrap(),result["items"][0]["examples"][0]["marker"].as_str().unwrap());
        let answer=compile_agent_answer(&raw,&ledger.bindings(),&provenance).unwrap();
        assert!(answer.answer.contains("两段谱面比较"));
        assert!(!answer.answer.contains("[[musicfacts:"));
        assert_eq!(answer.bindings.len(),2);
        assert!(answer.view.parts.iter().any(|p|matches!(p,AgentAnswerPart::Sources{source_ref_ids} if source_ref_ids.len()==2)));
        assert!(compile_agent_answer("[[musicfacts:made-up]]",&ledger.bindings(),&provenance).is_err());
        if let Ok(path)=std::env::var("MUSIC_FACTS_VIEW_OUTPUT") { std::fs::write(path,serde_json::to_vec_pretty(&answer.view).unwrap()).unwrap(); }
    }

    #[test]
    #[ignore = "requires the local Beethoven evaluation publication; no model calls"]
    fn music_real_m01_passages_preserve_ends_and_calculate_the_observed_errors() {
        let dir=std::env::var("MUSIC_PASSAGE_PUBLICATION_DIR").expect("Set MUSIC_PASSAGE_PUBLICATION_DIR");
        let publication_book=Book::load(&dir).unwrap();
        let mut manifest=publication_book.load_music_source(std::path::Path::new(&dir)).unwrap();
        let b=book(); manifest.book_id=source().target.publication.book_id;
        let mut port=music_port(&b,"music-real-m01",manifest.clone());
        let mut ledger=TurnEvidenceLedger::default(); let mut reads=Vec::new();
        for (part,first,last) in [("P48","17","18"),("P48","19","20"),("P51","218","219"),("P48","80","81"),("P48","489","489"),("P4","80","81")] {
            let start=manifest.occurrences.iter().find(|o|o.measure==first).unwrap().start_ms;
            let end=manifest.occurrences.iter().find(|o|o.measure==last).unwrap().end_ms;
            let mut request:read_tools::music::MusicRead=serde_json::from_value(json!({"view":"score","voices":[{"part_id":part,"voice":"1:1"}],"location":{"recording":manifest.recording.source,"start_ms":start,"end_ms":end},"limit":64})).unwrap();
            let mut ids=Vec::new(); let mut pages=0;
            loop {
                let raw=read_tools::music::read(&manifest,&source().target.publication,&request).unwrap();
                let delivered=crate::music::project_read(&serde_json::to_string(&request).unwrap(),&serde_json::to_value(&raw).unwrap(),16*1024).expect("A complete measure must fit the live tool budget");
                let value=delivered.model_body.clone();
                assert!(value["body"]["returned_scope"].as_array().unwrap().iter().all(|u|u["measure_complete"]==true));
                ids.extend(value["sources"][0]["material"]["read"]["objects"].as_array().unwrap().iter().map(|n|n["source_id"].as_str().unwrap().to_owned()));
                observe_tool_evidence(&mut ledger,"music.read","{}",&value.to_string(),&b);
                pages+=1;
                reads.push(json!({"part":part,"from":first,"through":last,"bytes":value.to_string().len(),"body":value["body"]}));
                let Some(next)=value["body"]["next_offset"].as_u64() else { break; };
                assert!(next as usize>request.offset); request.offset=next as usize;
            }
            let expected:Vec<_>=manifest.score_objects.iter().filter(|n|n.part_id==part&&n.voice=="1:1"&&n.measure.parse::<u32>().unwrap()>=first.parse().unwrap()&&n.measure.parse::<u32>().unwrap()<=last.parse().unwrap()&&!n.is_hidden_muted()).map(|n|n.source_id.clone()).collect();
            assert_eq!(ids,expected); assert!(pages<=2);
        }
        let selection=|part:&str,from:&str,through:&str|json!({"part_id":part,"voice":"1:1","from_id":from,"through_id":through});
        let mut span=selection("P48","s-p16-m17-n1","s-p16-m18-n5");span["key"]=json!("span");span["kind"]=json!("score");
        let mut rhythm=selection("P48","s-p16-m80-n1","s-p16-m81-n6");rhythm["key"]=json!("rhythm");rhythm["kind"]=json!("score");
        let args=json!({"items":[span,rhythm,{"key":"comparison","kind":"comparison","left":selection("P48","s-p16-m19-n1","s-p16-m20-n4"),"right":selection("P51","s-p19-m218-n1","s-p19-m219-n4")}]}).to_string();
        let result=register_music_examples(&args,&mut ledger,&mut port,"rev").unwrap();
        assert_eq!(result["items"][0]["facts"]["data"]["endpoint_span"]["written_interval"]["number"],10,"{result}");
        let rhythm=&result["items"][1]["facts"]["data"]["measures"][0]["onsets"];
        assert_eq!(rhythm.as_array().unwrap().len(),6);
        assert_eq!(rhythm[4]["notes"][0]["duration_beats"],0.5);
        assert_eq!(rhythm[5]["notes"][0]["rest"],true);
        for item in result["items"].as_array().unwrap() { assert!(json!({"items":[item]}).to_string().len()<=16*1024); }
        let oboe:Vec<_>=manifest.score_objects.iter().filter(|n|n.part_id=="P4"&&n.voice=="1:1"&&matches!(n.measure.as_str(),"80"|"81")&&!n.is_hidden_muted()).collect();
        let oboe=read_tools::music::facts::passage(&manifest.score_objects,&oboe);
        assert_eq!(oboe["measures"][0]["onsets"][0]["notes"][0]["rest"],true);
        assert_eq!(oboe["endpoint_span"]["from"],"F4"); assert_eq!(oboe["endpoint_span"]["to"],"D5");
        assert_eq!(oboe["endpoint_span"]["written_interval"]["number"],6);
        assert_eq!(oboe["endpoint_span"]["written_interval"]["quality"],"major");
        let comparison=&result["items"][2]["facts"]["data"];
        assert_eq!(comparison["rhythm_equal"],true,"{comparison}");
        assert_eq!(comparison["constant_pitch_class_shift"],-2);
        assert!(comparison["constant_semitones"].is_null());
        let sharp=manifest.score_objects.iter().find(|n|n.source_id=="s-p16-m489-n4").unwrap();
        assert_eq!(read_tools::music::facts::written_name(sharp).as_deref(),Some("G♯5"));
        let mut provenance=AnswerProvenanceLedger::default();provenance.observe_tool_result("music.example",&result.to_string());
        let answer=compile_agent_answer(result["items"][2]["facts_marker"].as_str().unwrap(),&ledger.bindings(),&provenance).unwrap();
        if let Ok(path)=std::env::var("MUSIC_FACTS_VIEW_OUTPUT") { std::fs::write(path,serde_json::to_vec_pretty(&json!({"reads":reads,"result":result,"view":answer.view})).unwrap()).unwrap(); }
    }

    #[test]
    fn music_catalog_pages_make_progress_without_becoming_read_evidence() {
        let b=book();let mut port=music_port(&b,"music-catalog-progress",manifest(9));
        let mut turns:Vec<_>=(0..3).map(|offset| turn_calls(vec![call(&format!("catalog{offset}"),"music.read",&serde_json::json!({"view":"catalog","offset":offset,"limit":1}).to_string())])).collect();
        turns.push(turn_calls(vec![call("event","music.read",r#"{"view":"events","event_id":"event0","limit":1}"#)]));
        turns.push(turn_final("The prepared theme has now been read."));
        let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);
        let snapshot=default_profile_snapshot(&b,&port.store,"t0");
        let mut context=RunContext::new(new_session(),OuterConfig{max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Analyze the whole movement's material","t0").unwrap();
        assert!(!outcome.incomplete,"Distinct catalog pages must not trigger the no-progress fallback");
        assert_eq!(port.calls.len(),4,"All three discovery pages and the actual event read execute");
        assert_eq!(context.evidence_ledger.music_discovered.len(),3);
        assert_eq!(context.evidence_ledger.music_observed.len(),1,"Only the event read becomes analysis evidence");
        assert_eq!(context.evidence_ledger.music_observed[0].target.event_ids,vec!["event0"]);
    }

    #[test]
    fn music_catalog_synonyms_do_not_manufacture_progress_for_the_same_entries() {
        let b=book();let mut port=music_port(&b,"music-catalog-repeated",manifest(9));
        let mut turns:Vec<_>=["Example","Exam","Ex"].iter().enumerate().map(|(i,query)| turn_calls(vec![call(&format!("catalog{i}"),"music.read",&serde_json::json!({"view":"catalog","query":query}).to_string())])).collect();
        turns.push(turn_final("The same catalog entries were found; their analysis has not been read."));
        let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);
        let snapshot=default_profile_snapshot(&b,&port.store,"t0");
        let mut context=RunContext::new(new_session(),OuterConfig{max_turns:Some(8),..Default::default()},adapter.model_runtime_profile());
        let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Analyze the whole movement's material","t0").unwrap();
        assert!(outcome.incomplete);
        assert_eq!(outcome.warning.as_deref(),Some("AGENT_NO_PROGRESS"));
        assert_eq!(context.evidence_ledger.music_discovered.len(),3);
        assert!(context.evidence_ledger.music_observed.is_empty());
        assert!(context.evidence_ledger.bindings().is_empty(),"Discovery cannot authorize a citation");
    }

    #[test]
    fn music_examples_keep_complete_sources_across_model_pages_and_the_real_object_limit() {
        let b=book();
        for count in [9,200,201] {
            let manifest=manifest(count);
            let mut port=music_port(&b,&format!("music-example-pages-{count}"),manifest.clone());
            let mut ledger=TurnEvidenceLedger::default();
            for offset in [0,(count-1)/8*8] {
                let request:read_tools::music::MusicRead=serde_json::from_value(serde_json::json!({"view":"score","offset":offset,"limit":8})).unwrap();
                let result=read_tools::music::read(&manifest,&source().target.publication,&request).unwrap();
                observe_tool_evidence(&mut ledger,"music.read",&serde_json::to_string(&request).unwrap(),&serde_json::to_string(&result).unwrap(),&b);
            }
            assert_eq!(ledger.music_observed[0].material["read"]["next_offset"],8);
            let args=serde_json::json!({"items":[{"key":"theme","kind":"score","part_id":"piano","voice":"1","from_id":"note0","through_id":format!("note{}",count-1)}]}).to_string();
            let result=register_music_examples(&args,&mut ledger,&mut port,"revision").unwrap();
            if count>200 {
                assert_eq!(result["items"][0]["error"]["error_code"],"INVALID_MUSIC_EXAMPLE");
                assert!(ledger.bindings().is_empty());
            } else {
                let example=&result["items"][0]["example"];
                assert_eq!(example["notes"].as_array().unwrap().len(),count,"{result}");
                let binding=ledger.bindings().into_iter().find(|s| Some(s.source_ref_id.as_str())==example["source_ref_id"].as_str()).unwrap();
                let frozen=binding.music.unwrap();
                assert_eq!(crate::music_example::objects(&frozen).len(),count);
                assert_eq!(frozen.target.score.as_ref().unwrap().source_ids.len(),count);
                assert!(frozen.material["read"]["next_offset"].is_null());
                assert_eq!(frozen.target.occurrence_ids.len(),count.div_ceil(8));
            }
        }
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
            self.calls.push("read".into());
            if let Some(manifest) = &self.manifest { return read_tools::music::read(manifest, &source().target.publication, &request); }
            assert_eq!(request.view,"events");let source=source();
            Ok(read_tools::music::MusicReadResult{publication:source.target.publication.clone(),recording:source.target.audio.recording.clone(),observation_kind:"prepared_music_material".into(),heard_audio:false,body:serde_json::json!({"view":"events"}),sources:vec![source]})
        }
        fn music_example_source(&mut self, reference:&base_schema::PublishedBookRef)->Result<base_schema::MusicSourceManifest,ToolError> {
            assert_eq!(reference, &source().target.publication);
            Ok(self.manifest.clone().expect("example fixture"))
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
        let mut port = Port { store:MemoryStore::open(tmp("music-question-image")).unwrap(),reader:Reader::new(&b,1),player_ms:1200.0,paused:true,loop_range:None,calls:vec![],question_image:Some(image.clone()),manifest:None };
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
        let b=book();let mut port=Port{store:MemoryStore::open(tmp("music-runtime" )).unwrap(),reader:Reader::new(&b,1),player_ms:1200.0,paused:true,loop_range:None,calls:vec![],question_image:None,manifest:None};
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



#[test]
fn inline_music_examples_compile_only_registered_refs_and_keep_plain_labels() {
    let mut provenance = AnswerProvenanceLedger::default();
    let example: crate::music_example::MusicExample = serde_json::from_value(serde_json::json!({"ref_id":"listen_test","kind":"illustration","title":"教学示例","label":"A4 → A5","tempo_bpm":66,"duration_ms":1818,"source_ref_id":null,"notes":[{"midi_pitch":69,"start_ms":0,"end_ms":909},{"midi_pitch":81,"start_ms":909,"end_ms":1818}]})).unwrap();
    provenance.observe_tool_result("music.example", &serde_json::json!({"items":[{"example":example}]}).to_string());
    let answer=compile_agent_answer("先听 [[listen:listen_test]]，再比一次 [[listen:listen_test]]。", &[], &provenance).unwrap();
    assert_eq!(answer.answer,"先听 A4 → A5，再比一次 A4 → A5。");
    assert_eq!(answer.view.parts.iter().filter(|p|matches!(p,AgentAnswerPart::MusicExample{..})).count(),2);
    assert!(compile_agent_answer("听 [[listen:invented]]", &[], &provenance).is_err());
    let registration=resident_tool_registry();
    registration.registration("music.example").unwrap().validate_arguments(r#"{"items":[{"key":"m20","kind":"score","part_id":"P48","voice":"1:1","from_id":"n1","through_id":"n4"}]}"#).unwrap();
}
