use super::*;
use serde_json::{json, Value};

fn source() -> read_tools::music::MusicSourceSnapshot {
    serde_json::from_value(json!({"target":{"publication":{"book_id":"music","publication_id":"pub1"},
        "audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1000,"end_ms":2000},
        "score":{"score":{"score_id":"score","revision":"1"},"source_ids":["note1"]},"occurrence_ids":["repeat2"],"event_ids":["return"],"provenance_ids":["score-import"]},
        "title":"Theme","label":"Theme · 00:01 · 谱面","preview":"Theme returns","material":{"kind":"score","explanation":"This note is written again"}})).unwrap()
}

#[test]
fn music_citations_require_this_turn_read_and_preserve_publication_and_occurrence() {
    let mut source=source();
    source.target.score.as_mut().unwrap().source_ids.push("note2".into());
    source.target.provenance_ids.push("interpretation".into());
    let mut ledger=TurnEvidenceLedger::default();
    assert_eq!(ledger.present_music(&source.target,"book-source-revision").unwrap_err().error_code,"SOURCE_NOT_OBSERVED");
    ledger.music_observed.push(source.clone());
    let receipt=ledger.present_music(&source.target,"book-source-revision").unwrap();
    assert_eq!(ledger.present_music(&source.target,"book-source-revision").unwrap().source_ref_id,receipt.source_ref_id);
    let mut reordered=source.target.clone();
    reordered.score.as_mut().unwrap().source_ids.reverse();reordered.provenance_ids.reverse();
    assert_eq!(ledger.present_music(&reordered,"book-source-revision").unwrap().source_ref_id,receipt.source_ref_id,
        "Identity lists are sets; reordering cannot change the citation or discard its original snapshot");
    reordered.score.as_mut().unwrap().source_ids[0]="unread-note".into();
    assert!(ledger.present_music(&reordered,"book-source-revision").is_err());
    let binding=ledger.bindings().pop().unwrap();assert!(binding.valid_evidence());
    assert!(binding.media.is_none());assert_eq!(binding.music,Some(source.clone()));
    let stored=serde_json::to_string(&binding).unwrap();
    assert_eq!(serde_json::from_str::<SourceBinding>(&stored).unwrap(),binding);
    let mut other=source.target.clone();other.publication.publication_id="pub2".into();
    assert!(ledger.present_music(&other,"book-source-revision").is_err());
    other=source.target;other.occurrence_ids=vec!["repeat3".into()];assert!(ledger.present_music(&other,"book-source-revision").is_err());
}

#[test]
fn music_playback_cue_requires_this_turn_observed_event_in_exact_publication() {
    let source=source();let mut ledger=TurnEvidenceLedger::default();
    let request:crate::music::MusicInteractRequest=serde_json::from_value(json!({"operation":"act","publication":source.target.publication,
        "recording":source.target.audio.recording,"based_on":"actual-receipt","action":{"kind":"play","start_ms":1000,"end_ms":2000,"cue_event_id":"return"}})).unwrap();
    assert_eq!(ledger.check_music_cue(&request).unwrap_err().error_code,"MUSIC_CUE_NOT_OBSERVED");
    ledger.music_observed.push(source.clone());ledger.check_music_cue(&request).unwrap();
    ledger.music_observed[0].target.publication.publication_id="another-publication".into();
    assert!(ledger.check_music_cue(&request).is_err());
}

#[test]
fn music_budget_never_truncates_source_identity_or_registers_unread_analysis() {
    let book=Book::load(concat!(env!("CARGO_MANIFEST_DIR"),"/../book-tool-contracts/fixtures/example-book")).unwrap();
    let source=source();
    let result=read_tools::music::MusicReadResult{publication:source.target.publication.clone(),recording:source.target.audio.recording.clone(),
        observation_kind:"prepared_music_material".into(),heard_audio:false,body:json!({"view":"score"}),sources:vec![source.clone()]};
    let raw=serde_json::to_string(&result).unwrap();
    let policy=crate::tool_registry::ToolOutputPolicy{result_policy:crate::tool_registry::ToolResultPolicy::EvidenceProjection,max_model_body_bytes:10_000};
    let truncated=crate::tool_result::project_tool_result("music.read",r#"{"view":"score"}"#,&raw,policy,300,&book);
    assert!(truncated.truncated);
    let mut ledger=TurnEvidenceLedger::default();
    observe_tool_evidence(&mut ledger,"music.read","{}",&truncated.model_body_json(),&book);
    assert!(ledger.music_observed.is_empty());
    let complete=crate::tool_result::project_tool_result("music.read","{}",&raw,policy,10_000,&book);
    observe_tool_evidence(&mut ledger,"music.read","{}",&complete.model_body_json(),&book);
    assert_eq!(ledger.music_observed,vec![source.clone()]);
    observe_tool_evidence(&mut ledger,"music.read","{}",&complete.model_body_json(),&book);
    assert_eq!(ledger.music_observed.len(),1,"Reading the same prepared target twice is not new progress");
    let progress=TurnProgressLedger::from_turn(&ledger,&TurnLocatorLedger::default());
    assert_eq!(progress.phase,ProgressPhase::EvidenceReady);
    let mut next=result;next.sources[0].target.occurrence_ids=vec!["repeat3".into()];
    next.sources[0].target.audio.start_ms=2000.0;next.sources[0].target.audio.end_ms=3000.0;
    observe_tool_evidence(&mut ledger,"music.read","{}",&serde_json::to_string(&next).unwrap(),&book);
    assert_eq!(ledger.music_observed.len(),2,"A distinct actual performance occurrence advances prepared-source progress");
    assert!(ledger.evidence.is_empty(),"Music analysis does not manufacture textual LIDs");
}

#[test]
fn music_review_keeps_cited_and_target_snapshots_without_superseded_passages() {
    let first=source();let mut second=first.clone();second.target.audio.start_ms=2000.0;second.target.audio.end_ms=3000.0;
    second.target.occurrence_ids=vec!["repeat3".into()];
    let mut ledger=TurnEvidenceLedger::default();ledger.music_observed=vec![first.clone(),second.clone()];
    let first_ref=ledger.present_music(&first.target,"revision").unwrap().source_ref_id;
    let second_ref=ledger.present_music(&second.target,"revision").unwrap().source_ref_id;
    let proposed=json!({"text":format!("A source [[source:{second_ref}]]"),"moves":[]});
    let filtered=teaching_review_source_bindings(ledger.bindings(),&Value::Null,&proposed,&[]);
    assert_eq!(filtered.len(),1);assert_eq!(filtered[0].source_ref_id,second_ref);
    assert_eq!(filtered[0].music,Some(second.clone()),"The used snapshot stays complete, including actual material and locators");
    let design=json!({"design":{"target":{"music_bindings":[{"target":first.target}]}}});
    let filtered=teaching_review_source_bindings(ledger.bindings(),&design,&proposed,&[]);
    assert_eq!(filtered.len(),2,"An existing teaching target still needs its own source evidence");
    let movement=json!({"text":"No explicit citation","moves":[{"target":{"music_bindings":[{"target":second.target}]}}]});
    assert_eq!(teaching_review_source_bindings(ledger.bindings(),&Value::Null,&movement,&[])[0].source_ref_id,second_ref);
    let pages=vec![json!({"text":format!("Page source [[source:{first_ref}]]")})];
    assert_eq!(teaching_review_source_bindings(ledger.bindings(),&Value::Null,&json!({}),&pages)[0].source_ref_id,first_ref);
}

#[test]
fn music_budget_returns_complete_prefix_and_observes_only_delivered_sources() {
    let book = Book::load(concat!(env!("CARGO_MANIFEST_DIR"),"/../book-tool-contracts/fixtures/example-book")).unwrap();
    let mut first = source(); first.material["read"] = json!({"returned_count":1,"events":[{"explanation":"A".repeat(1200)}]});
    let mut second = first.clone(); second.target.event_ids = vec!["later".into()];
    let result = read_tools::music::MusicReadResult { publication:first.target.publication.clone(), recording:first.target.audio.recording.clone(),
        observation_kind:"prepared_music_material".into(), heard_audio:false,
        body:json!({"view":"events","offset":0,"returned_count":2,"total":2,"next_offset":null}), sources:vec![first.clone(),second] };
    let mut prefix = result.clone(); prefix.sources.truncate(1); prefix.body["returned_count"] = json!(1); prefix.body["next_offset"] = json!(1);
    let budget = serde_json::to_vec(&prefix).unwrap().len();
    let policy = crate::tool_registry::ToolOutputPolicy { result_policy:crate::tool_registry::ToolResultPolicy::EvidenceProjection,max_model_body_bytes:budget };
    let delivered = crate::tool_result::project_tool_result("music.read",r#"{"view":"events","query":"theme","limit":2}"#, &serde_json::to_string(&result).unwrap(),policy,budget,&book);
    assert!(delivered.truncated);
    assert!(delivered.model_body_json().len() <= budget);
    assert_eq!(delivered.model_body["body"]["next_offset"], 1);
    match delivered.continuation.as_ref().unwrap() {
        crate::tool_result::ToolContinuation::ToolCursor { tool, arguments } => {
            assert_eq!(tool,"music.read"); assert_eq!(arguments["offset"],1); assert_eq!(arguments["query"],"theme");
        }, _ => panic!("Expected a usable next page"),
    }
    let mut ledger = TurnEvidenceLedger::default();
    observe_tool_evidence(&mut ledger,"music.read","{}",&delivered.model_body_json(),&book);
    assert_eq!(ledger.music_observed,vec![first.clone()]);
    assert!(ledger.present_music(&first.target,"revision").is_ok());
    assert!(ledger.present_music(&result.sources[1].target,"revision").is_err());
}
