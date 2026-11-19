use super::*;
use reader_service::teaching_coach::tests::{RecordingAdapter, local_fixture, finish_observation};
use memory::tutor_coach::{CoachJobOrigin, CoachJobStatus};

#[test]
fn coach_network_pending_and_running_observations_do_not_block_turns() {
    let f=crate::tests::mu5_tests::Fixture::new();
    let adapter=RecordingAdapter::default();let factory=adapter.clone();
    f.access.runs.configure_fake(move ||Box::new(factory.clone()));
    let (source_root,_source_state)=local_fixture();
    let reference=f.access.library.lock().unwrap().publish(source_root.path()).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&reference).unwrap();
    let w=f.create(&f.a,&reference,"coach-network");
    let mut w=f.action(&f.a,&w,"coach-network","chat/new",json!({}));
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","now").unwrap();
    let publication=f.access.library.lock().unwrap().load("A",&reference).unwrap();
    let path=format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap());
    let mut pending=None;
    for index in 0..2 {
        let input=json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":"coach-network",
            "session_id":w["selected_chat"],"client_request_id":format!("ordinary-{index}"),"message":"Explain the stopped interval."});
        let (status,accepted)=f.call(&f.a,"POST",&path,input.clone());assert_eq!(status,202,"{accepted}");
        assert!(f.access.runs.run_one(&f.access).unwrap());
        assert_eq!(f.call(&f.a,"POST",&path,input).1["turn_id"],accepted["turn_id"]);
        {
            let user=handle.lock().unwrap();
            let saved=user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|t.turn_id==accepted["turn_id"]).unwrap();
            assert_eq!(saved.status,AgentAssistantStatus::Completed,"{:?}",saved.error);
            assert!(!saved.outcome.as_ref().unwrap().incomplete);
            let dependency=user.learning_store().unwrap().coach_turn_context(w["selected_chat"].as_str().unwrap(),&saved.turn_id).unwrap().unwrap();
            assert!(dependency.job_ref.is_none() && dependency.context.is_none());
            if index==0 {pending=claim(&user).unwrap();}
        }
        w=f.get(&f.a,&w);
    }
    assert!(!adapter.samples.lock().unwrap().iter().any(|(kind,_)|kind=="coach"),"foreground does not wait for observations");
    let job=pending.unwrap();assert_eq!(job.input.origin,CoachJobOrigin::AfterTurn);
    finish_observation(&adapter,&handle.lock().unwrap(),&job,&publication.book).unwrap();
    let user=handle.lock().unwrap();let jobs=user.learning_store().unwrap().list_coach_jobs(None,10).unwrap();
    assert_eq!(jobs.len(),2);assert!(jobs.iter().all(|job|job.input.origin==CoachJobOrigin::AfterTurn));
    assert_eq!(jobs.iter().filter(|job|job.status==CoachJobStatus::Ready).count(),1);
    assert_eq!(jobs.iter().filter(|job|job.status==CoachJobStatus::Pending).count(),1);
}

