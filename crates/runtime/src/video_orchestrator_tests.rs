// Included inside orchestrator::tests so the normal Resident fixtures are reused.
mod video_runtime {
    use super::*;
    use crate::video::{FrameBatch, ReadFrames, VideoImage};
    use base_schema::{FrameLocation, FrameRead, FrameRegion, MediaObservation, PublishedBookRef};

    struct Port {
        store: MemoryStore,
        reader: Reader,
        mode: &'static str,
    }
    impl ResidentStatePort for Port {
        fn review_video_audio(&mut self, r: crate::video_audio::ReviewAudio, cancel: &crate::run_context::CancellationToken) -> Result<crate::video_audio::AudioReview, ToolError> {
            if self.mode == "audio_error" { return Err(crate::video::unavailable()); }
            if self.mode == "audio_cancel" { cancel.cancel(); }
            Ok(crate::video_audio::AudioReview { run_id:"original-run".into(),review_result_ref:"audio-result".into(),
                publication:PublishedBookRef { book_id:"b".into(),publication_id:"original-publication".into() },location:r.location,question:r.question,
                reviewed_text:"underflow and overflow".into(),reviewed_words:vec![],uncertainties:vec!["unclear word remains".into()],provider:serde_json::json!({"engine":"fixture"}),transcript:vec![] })
        }
        fn media_source_snapshot(&mut self, target: base_schema::EvidenceTarget) -> Result<crate::video_source::MediaSource, ToolError> {
            Ok(crate::video_source::MediaSource { publication:crate::video_source::publication(&target).unwrap().clone(),target,duration_ms:2000.0,
                transcript:vec![],translations:vec![],observations:vec![],audio_reviews:vec![] })
        }
        fn search_video(&mut self, request: read_tools::video_search::VideoSearch) -> Result<serde_json::Value, ToolError> {
            assert_eq!(request.query, "float16");
            Ok(serde_json::json!({"candidates":[{"screen_text":"float16", "read_hint":serde_json::from_str::<serde_json::Value>(&super::video_runtime::request(123.0,false)).unwrap()}],"discourse":{"candidates":[{"discourse_id":"example","via":[{"relation":{"type":"exemplifies"}}],"read_hints":[serde_json::from_str::<serde_json::Value>(&super::video_runtime::request(123.0,false)).unwrap()]}]},"evidence_status":"candidate_only"}))
        }
        fn submit_private<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore) -> R,
        ) -> Result<R, ToolError> {
            Ok(f(&mut self.store))
        }
        fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
            Ok(f(&self.reader))
        }
        fn apply_reader<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R,
        ) -> Result<R, ToolError> {
            Ok(f(&mut self.store, &mut self.reader))
        }
        fn reader_input(&mut self, b: &Book, q: &str) -> crate::run_context::ReaderInputSnapshot {
            let mut snapshot = crate::run_context::ReaderInputSnapshot::capture(b, &self.reader, q);
            if self.mode == "bound_question" {
                snapshot.video_question = Some(serde_json::from_value(serde_json::json!({"draft_id":"input-at-08:12","publication":{"book_id":"b","publication_id":"original-publication"},"material":{"book_id":"b","video":{"media_id":"v","media_revision":"1"},"transcript":{"transcript_id":"t","transcript_revision":"1","video":{"media_id":"v","media_revision":"1"}}},"frame":{"video":{"media_id":"v","media_revision":"1"},"actual_pts_ms":492000,"width":1920,"height":1080},"region":null,"range":null})).unwrap());
                let question = snapshot.video_question.as_mut().unwrap();
                question.range = Some(base_schema::MediaLocation { video: question.material.video.clone(), start_ms: 400000.0, end_ms: 510000.0 });
                question.region = Some(FrameRegion { frame: question.frame.clone(), rect_normalized: [0.1, 0.2, 0.3, 0.4] });
            }
            snapshot
        }
        fn read_video_frames(
            &mut self,
            r: ReadFrames,
            _: &crate::run_context::CancellationToken,
        ) -> Result<FrameBatch, ToolError> {
            if self.mode == "decode_error" {
                return Err(crate::video::unavailable());
            }
            let images = if self.mode == "path_only" {
                vec![]
            } else {
                r.sampling
                    .iter()
                    .map(|t| {
                        let frame = FrameLocation {
                            video: r.location.video.clone(),
                            actual_pts_ms: *t + 10.0,
                            width: 1920,
                            height: 1080,
                        };
                        VideoImage {
                            read: FrameRead {
                                requested_ms: *t,
                                frame: frame.clone(),
                            },
                            region: r.region.map(|rect_normalized| FrameRegion {
                                frame,
                                rect_normalized,
                            }),
                            png_base64: format!("fixture-pixels-{t}"),
                            image_width: 1920,
                            image_height: 1080,
                        }
                    })
                    .collect()
            };
            Ok(FrameBatch {
                run_id: "original-run".into(),
                publication: PublishedBookRef {
                    book_id: "b".into(),
                    publication_id: "original-publication".into(),
                },
                location: r.location,
                images,
                transcript: vec![],
            })
        }
    }

    #[test]
    fn video_audio_review_discovery_observation_source_history_failure_and_cancel() {
        for mode in ["success", "audio_error", "audio_cancel"] {
            let b=book();
            let mut port=Port {store:MemoryStore::open(tmp(&format!("vi15-{mode}"))).unwrap(),reader:Reader::new(&b,1),mode};
            let r=serde_json::json!({"location":{"video":{"media_id":"v","media_revision":"1"},"start_ms":0,"end_ms":2000},"question":"underflow or overflow?"});
            let target=serde_json::json!({"kind":"audio","publication":{"book_id":"b","publication_id":"original-publication"},"location":r["location"],"review_result_ref":"audio-result"});
            let turns=vec![turn_calls(vec![call("discover","tool.search",r#"{"task":"video.review_audio 原音频 听核 讲者措辞","required_capabilities":["source_read"],"scope":"passage","operation":"read_source","effect_mode":"read_only","max_results":4}"#)]),
                turn_calls(vec![call("audio","video.review_audio",&r.to_string())]),
                turn_calls(vec![call("source","source.present",&serde_json::json!({"target":target}).to_string())]),turn_final("Audio review completed.")];
            let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);
            let snapshot=default_profile_snapshot(&b,&port.store,"t0");
            let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
            let result=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
                &mut EphemeralCompactionCheckpointSink::default(),"Review the original audio wording","t0");
            if mode=="audio_cancel" { assert!(result.is_err()); } else { result.unwrap(); }
            let plans=adapter.seen_plans.borrow();
            assert!(plans[1].tools.iter().any(|t| t.name=="video.review_audio"));
            assert!(plans.iter().all(|p|p.video_images.is_empty()));
            if mode=="success" {
                assert_eq!(context.media_observations.len(),1);
                assert!(matches!(&context.media_observations[0],MediaObservation::Audio {run_id,tool_call_id,..} if run_id=="original-run" && tool_call_id=="audio"));
                assert!(serde_json::to_string(&plans[2].ordered_messages()).unwrap().contains("unclear word remains"));
                let bindings=context.evidence_ledger.bindings(); assert_eq!(bindings.len(),1,"{}",serde_json::to_string(&plans.last().unwrap().ordered_messages()).unwrap());
                let saved=serde_json::to_string(&bindings).unwrap();
                let restored:Vec<SourceBinding>=serde_json::from_str(&saved).unwrap();
                assert_eq!(restored[0].media.as_ref().unwrap().audio_reviews[0],context.audio_reviews[0]);
                let new_context=RunContext::new(context.messages.clone(),OuterConfig::default(),adapter.model_runtime_profile());
                assert!(new_context.evidence_ledger.check_media_target(&b,&context.audio_reviews[0].target(),&new_context.media_observations).is_err());
            } else { assert!(context.media_observations.is_empty());assert!(context.audio_reviews.is_empty());assert!(context.evidence_ledger.bindings().is_empty()); }
        }
    }
    struct CancelOnImage<'a> {
        inner: &'a RequestPlanRecordingAdapter,
        token: crate::run_context::CancellationToken,
    }
    impl ModelAdapter for CancelOnImage<'_> {
        fn complete(&self, r: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            self.inner.complete(r)
        }
        fn chat(&self, p: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            let result = self.inner.chat(p);
            if !p.video_images.is_empty() {
                self.token.cancel();
            }
            result
        }
    }
    fn request(t: f64, crop: bool) -> String {
        let mut v = serde_json::json!({"location":{"video":{"media_id":"v","media_revision":"1"},"start_ms":0,"end_ms":2000},"sampling":[t]});
        if crop {
            v["region"] = serde_json::json!([0.1, 0.1, 0.5, 0.5]);
        }
        v.to_string()
    }
    fn discovery() -> AssistantTurn {
        turn_calls(vec![call(
            "discover",
            "tool.search",
            r#"{"task":"video.read_frames video frames crop 视频画面取帧","required_capabilities":["source_read"],"scope":"passage","operation":"read_source","effect_mode":"read_only","max_results":4}"#,
        )])
    }

    #[test]
    fn video_graph_recall_is_a_pointer_until_actual_frames_are_observed() {
        for followup in [false,true] {
            let mut b=book();
            b.base.graph_nodes.push(serde_json::from_value(serde_json::json!({"id":"video:graph:x","type":"concept","name":"matrix",
                "identity":{"kind":"video_object","meaning":"input matrix"},"occurrences":[],"source_lid":null,
                "evidence":[{"support_role":"visible dimensions","target":{"kind":"visual","publication":{"book_id":"b","publication_id":"original-publication"},
                    "frames":[{"video":{"media_id":"v","media_revision":"1"},"actual_pts_ms":133,"width":1920,"height":1080}],"regions":[]}}]})).unwrap());
            let mut port=Port{store:MemoryStore::open(tmp(&format!("vi13-{followup}"))).unwrap(),reader:Reader::new(&b,1),mode:"success"};
            let mut turns=vec![turn_calls(vec![call("concept","book.concept",r#"{"query":"matrix"}"#)])];
            if followup {turns.push(discovery());turns.push(turn_calls(vec![call("frames","video.read_frames",&request(123.0,false))]));}
            turns.push(turn_final("Located matrix candidate."));
            let adapter=RequestPlanRecordingAdapter::new(turns,vec![]);
            let snapshot=default_profile_snapshot(&b,&port.store,"t0");
            let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
            run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
                &mut EphemeralCompactionCheckpointSink::default(),"总结 matrix 的知识对象","t0").unwrap();
            let plans=adapter.seen_plans.borrow();
            let result=serde_json::to_string(&plans[1].ordered_messages()).unwrap();
            assert!(result.contains("candidate_only") && result.contains("read_hints"),"{result}");
            assert_eq!(context.media_observations.len(),usize::from(followup));
            assert!(context.evidence_ledger.bindings().is_empty());
        }
    }

    #[test]
    fn video_search_is_discoverable_but_only_followup_frames_create_evidence() {
        for followup in [false, true] {
            let b = book();
            let mut port = Port { store: MemoryStore::open(tmp(&format!("vi12-{followup}"))).unwrap(), reader: Reader::new(&b, 1), mode:"success" };
            let mut turns = vec![turn_calls(vec![call("discover", "tool.search",
                r#"{"task":"video.search 视觉检索 前文图示","required_capabilities":["lexical_locate"],"scope":"document","operation":"locate_literal","effect_mode":"read_only","max_results":4}"#)]),
                turn_calls(vec![call("search","video.search",r#"{"query":"float16"}"#)])];
            if followup { turns.push(discovery()); turns.push(turn_calls(vec![call("frames","video.read_frames",&request(123.0,false))])); }
            turns.push(turn_final("Finished locating candidate frames."));
            let adapter = RequestPlanRecordingAdapter::new(turns,vec![]);
            let snapshot = default_profile_snapshot(&b,&port.store,"t0");
            let mut context = RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
            run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
                &mut EphemeralCompactionCheckpointSink::default(),"Find earlier float16 code","t0").unwrap();
            let plans = adapter.seen_plans.borrow();
            assert!(plans[1].tools.iter().any(|t| t.name=="video.search"));
            assert!(serde_json::to_string(&plans[2].ordered_messages()).unwrap().contains("candidate_only"));
            assert!(serde_json::to_string(&plans[2].ordered_messages()).unwrap().contains("exemplifies"));
            assert_eq!(context.media_observations.len(),usize::from(followup));
            let publication = PublishedBookRef { book_id:"b".into(),publication_id:"original-publication".into() };
            let target = base_schema::EvidenceTarget::Visual { publication, frames:vec![FrameLocation { video:base_schema::VideoSourceRef { media_id:"v".into(),media_revision:"1".into() }, actual_pts_ms:133.0,width:1920,height:1080 }],regions:vec![] };
            assert_eq!(context.evidence_ledger.check_media_target(&b,&target,&context.media_observations).is_ok(),followup);
            assert!(context.evidence_ledger.bindings().is_empty());
        }
    }

    #[test]
    fn video_observations_require_successful_main_request_and_exact_pixels() {
        for mode in [
            "success",
            "crop",
            "failed_model",
            "path_only",
            "decode_error",
            "cancelled_model",
            "finalization",
        ] {
            let b = book();
            let mut port = Port {
                store: MemoryStore::open(tmp(&format!("vi5-{mode}"))).unwrap(),
                reader: Reader::new(&b, 1),
                mode,
            };
            let mut turns = vec![
                discovery(),
                turn_calls(vec![call(
                    "frames",
                    "video.read_frames",
                    &request(123.0, mode == "crop"),
                )]),
            ];
            if mode != "failed_model" {
                if mode == "success" {
                    for i in 1..4 {
                        turns.push(turn_calls(vec![call(
                            &format!("changed-{i}"),
                            "video.read_frames",
                            &request(123.0 + 500.0 * i as f64, false),
                        )]));
                    }
                }
                turns.push(turn_final("The visible image was processed."));
            }
            let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
            let snapshot = default_profile_snapshot(&b, &port.store, "t0");
            let mut context = RunContext::new(
                new_session(),
                OuterConfig {
                    max_turns: if mode == "finalization" {
                        Some(2)
                    } else {
                        None
                    },
                    ..Default::default()
                },
                adapter.model_runtime_profile(),
            );
            let cancelled = CancelOnImage {
                inner: &adapter,
                token: context.cancellation.clone(),
            };
            let active_adapter: &dyn ModelAdapter = if mode == "cancelled_model" {
                &cancelled
            } else {
                &adapter
            };
            let result = run_context(
                &b,
                &mut port,
                active_adapter,
                &mut context,
                &snapshot,
                &ResidentTurnResources::default(),
                None,
                &mut EphemeralCompactionCheckpointSink::default(),
                "Explain the code in these video frames",
                "t0",
            );
            let plans = adapter.seen_plans.borrow();
            assert!(
                plans[1].tools.iter().any(|t| t.name == "video.read_frames"),
                "video tool was not discoverable"
            );
            let expected = match mode {
                "success" => 4,
                "crop" | "finalization" => 1,
                _ => 0,
            };
            assert_eq!(
                context.media_observations.len(),
                expected,
                "{mode}: {result:?}"
            );
            if mode == "failed_model" || mode == "cancelled_model" {
                assert!(result.is_err());
                assert_eq!(plans[2].video_images.len(), 1);
            } else {
                assert!(result.is_ok(), "{mode}: {result:?}");
            }
            for (i, observation) in context.media_observations.iter().enumerate() {
                let MediaObservation::Visual {
                    run_id,
                    publication,
                    model_request_id,
                    frames,
                    regions,
                    ..
                } = observation
                else {
                    panic!()
                };
                assert_eq!(run_id, "original-run");
                assert_eq!(publication.publication_id, "original-publication");
                assert_eq!(
                    model_request_id,
                    &plans[i + 2].video_images[0].model_request_id
                );
                assert_eq!(
                    plans[i + 2].video_images[0].image.png_base64,
                    format!("fixture-pixels-{}", 123 + 500 * i)
                );
                if mode == "crop" {
                    assert!(frames.is_empty());
                    assert_eq!(regions.len(), 1);
                } else {
                    assert!(regions.is_empty());
                    assert_eq!(frames[0].frame.actual_pts_ms, 133.0 + 500.0 * i as f64);
                }
            }
            assert!(
                context.messages.iter().all(|m| !m
                    .content
                    .as_deref()
                    .unwrap_or_default()
                    .contains("fixture-pixels")),
                "pixels leaked to persisted history"
            );
            assert!(
                plans.iter().all(|p| p.preview_images.is_empty()),
                "video incorrectly became a presentation preview"
            );
        }
    }

    #[test]
    fn video_question_frame_is_present_in_first_model_input_before_observation() {
        let b = book();
        let mut port = Port { store: MemoryStore::open(tmp("vi6-bound")).unwrap(), reader: Reader::new(&b, 1), mode: "bound_question" };
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("Please read the frozen frame.")], vec![]);
        let profile = default_profile_snapshot(&b, &port.store, "t0");
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        run_context(&b, &mut port, &adapter, &mut context, &profile, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "Explain this frame; player has since reached 08:27", "t0").unwrap();
        let plans = adapter.seen_plans.borrow();
        let messages = serde_json::to_string(&plans[0].ordered_messages()).unwrap();
        assert!(messages.contains("492000"));
        assert!(messages.contains("original-publication"));
        assert!(messages.contains("not observed evidence"));
        assert!(messages.contains("400000")); assert!(messages.contains("510000"));
        assert!(messages.contains("chronological batches")); assert!(messages.contains("independent focus"));
        assert!(context.media_observations.is_empty());
    }

    #[test]
    fn video_budget_omission_and_path_only_never_create_observations() {
        let b = book();
        let mut port = Port {
            store: MemoryStore::open(tmp("vi5-budget")).unwrap(),
            reader: Reader::new(&b, 1),
            mode: "success",
        };
        let batch = port
            .read_video_frames(
                serde_json::from_str(&request(123.0, false)).unwrap(),
                &Default::default(),
            )
            .unwrap();
        for budget in [300, 5500] {
            let mut plan = AgentRequestPlan::for_ad_hoc(
                ModelRuntimeProfile::fallback("test", ProviderToolProtocol::Native),
                &[Message::user("same question")],
                &[],
            );
            plan.active_context.remaining_tokens = budget;
            let before = plan.active_context.estimated_input_tokens;
            crate::video::attach(&mut plan, vec![batch.clone(), batch.clone()]);
            assert_eq!(plan.video_images.len(), if budget == 300 { 0 } else { 1 });
            assert!(plan.active_context.fits);
            assert!(plan.active_context.estimated_input_tokens > before);
            assert!(plan.input.iter().any(|m| m
                .content
                .as_deref()
                .unwrap_or_default()
                .contains("remain unobserved")));
            let mut ledger = vec![];
            crate::video::accept(&plan, &mut ledger);
            assert_eq!(ledger.len(), plan.video_images.len());
        }
    }
}
