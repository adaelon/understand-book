pub use reader_service::tests::tutor_loop_tests::*;
use super::*;
use memory::teaching::TeachingFact;
use runtime::presentation::*;


#[path = "closed_book_tests.rs"]
mod closed_book_tests;

#[test]
fn tutor_t18_publications_add_materials_without_rebinding_prior_activities() {
    use crate::{control_store::{ControlStore, ServiceWriter}, published_library::PublishedLibrary};
    let (root, mut state) = fixture();
    let service = tempfile::tempdir().unwrap();
    let mut control = ControlStore::open(ServiceWriter::acquire(service.path()).unwrap()).unwrap();
    control.create_user("reader").unwrap();
    let mut library = PublishedLibrary::new(control);
    let mut receipt: Value = serde_json::from_slice(&std::fs::read(root.path().join("teaching_readiness.json")).unwrap()).unwrap();
    let mut map: Value = serde_json::from_slice(&std::fs::read(root.path().join("teaching/versions/v1/map.json")).unwrap()).unwrap();
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    let session_before = state.user.learning_store().unwrap().state().unwrap();
    let mut deliveries = Vec::new();
    for revision in 0..3 {
        if revision > 0 {
            let version = format!("v{revision}");
            let path = format!("teaching/versions/{version}/map.json");
            receipt["teaching_map_revision"] = json!(version);
            receipt["map_path"] = json!(path);
            map["objects"]["objects"][0]["object_revision"] = json!(revision);
            map["cognitive_materials"]["materials"][0]["purpose"] = json!(format!("Material revision {revision}"));
            std::fs::create_dir_all(root.path().join(format!("teaching/versions/{version}"))).unwrap();
            std::fs::write(root.path().join(path), map.to_string()).unwrap();
            std::fs::write(root.path().join("teaching_readiness.json"), receipt.to_string()).unwrap();
        }
        let publication = library.publish(root.path()).unwrap();
        assert!(library.load("reader", &publication.reference).is_err());
        library.grant("reader", &publication.reference).unwrap();
        let loaded = library.load("reader", &publication.reference).unwrap();
        state.workspace.bind_publication(loaded);
        let ready = crate::tutor_api::readiness_view(&state.workspace.book, &state.workspace.book_dir);
        assert_eq!(ready["status"], "ready");
        assert!(ready.get("required_stages").is_none());
        let run = prepare(&mut state);
        let expected_map = (revision > 0).then(||format!("v{revision}"));
        assert_eq!(run.tutor.as_ref().unwrap()["binding"]["map_revision"], json!(expected_map));
        if revision > 0 {
            let material = step_with_design(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
            assert!(material.to_string().contains(&format!("Material revision {revision}")));
        }
        let mut movement = source_move(&state);
        movement["move"]["actions"] = json!(["submit","revise","request_hint"]);
        movement["move"]["hint"] = json!("Use total time.");
        movement["move"]["assessment"] = json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
        if revision > 0 {
            movement["move"]["object_ids"] = json!(["speed"]);
            movement["move"]["target"]["object_refs"] = json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":revision}]);
        }
        step_with_design(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
        finish(&mut state, &run.turn_ref, &outcome(None));
        let delivery = state.user.learning_store().unwrap().teaching_event(&format!("delivery:{}:source",run.turn_ref.turn_id)).unwrap();
        json_post(&mut state, "/tutor/display", json!({"delivery_ref":delivery.event_id}));
        deliveries.push(delivery);
    }
    assert!(!state.workspace.book_dir.join("teaching/versions/v1/map.json").exists());
    for (index, saved) in deliveries.iter().enumerate() {
        let response = json_post(&mut state, "/tutor/action", json!({"operation_id":format!("old-{index}"),"delivery_ref":saved.event_id,"action":"submit","response":"A"}));
        assert_eq!(response["assessment"], "correct");
        assert_eq!(state.user.learning_store().unwrap().teaching_event(&saved.event_id).unwrap(), *saved);
        let evidence = state.user.learning_store().unwrap().evidence(&format!("assessment:action:old-{index}:{}",if index==0 {"source"} else {"speed"})).unwrap();
        assert_eq!(evidence.map_revision, saved.binding.map_revision);
        assert_eq!(serde_json::to_value(evidence.target).unwrap(), saved.payload["move"]["target"]);
    }
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(), session_before);
    let corrected = json_post(&mut state, "/tutor/correct", json!({"evidence_ref":"assessment:action:old-0:source","operation_id":"t18-correction","text":"I understand the denominator; explain the conditions."}));
    let session_id = session_before.control.current_tutor_session_id.clone().unwrap();
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"t18-pause","expected_revision":session_before.control.revision,"action":{"kind":"pause","session_id":session_id}}));
    let paused = prepare_agent_chat(&mut state,r#"{"message":"Explain the conditions"}"#,"later").unwrap();
    assert_eq!(paused.tutor.as_ref().unwrap()["status"], "observing");
    finish(&mut state, &paused.turn_ref, &outcome(None));
    let original_book = state.workspace.book.clone();
    let mut other = sample_base(); other.book_id = "other-book".into();
    state.workspace.book = Arc::new(Book::new(other, "other source"));
    let control_before = state.user.learning_store().unwrap().state().unwrap();
    assert_eq!(reader_service::tests::host_calls::teaching::start_request(&state.private_context(), "now").unwrap()["started"], false);
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(), control_before);
    state.workspace.book = original_book;
    assert_eq!(reader_service::tests::host_calls::teaching::start_request(&state.private_context(), "now").unwrap()["started"], true);
    post(&mut state, "/agent/new", "{}");
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let next = prepare(&mut state).tutor.unwrap();
    assert_ne!(next["binding"]["tutor_session_id"], session_id);
    assert!(next["learner_context"]["interpretations"].as_array().unwrap().iter().any(|i|
        i["evidence_ref"] == corrected["evidence_ref"] && i["correction"] == "I understand the denominator; explain the conditions."));
}