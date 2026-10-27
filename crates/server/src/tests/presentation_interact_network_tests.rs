use super::*;
use runtime::presentation::*;
use runtime::presentation_interact::*;
use runtime::run_context::CancellationToken;
use crate::agent_stream::{RunDescriptor, RunStream};

#[test]
fn om4_network_command_wait_releases_locks_and_uses_workspace_saved_receipt() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "om4");
    let w = f.action(&f.a, &w, "om4", "chat/new", json!({}));
    let first = f.run(&w, "om4");
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let reference = {
        let mut user = handle.lock().unwrap();
        let private = first.scope.private_user(&user);
        let content = PresentationContent { action_contract:vec![PresentationActionContract::SetParameters {parameters:vec!["speed".into()]}],
            teaching_binding:None, method_refs:None, method_observations:Default::default(), animation_assets:Default::default(),
            title:"Speed".into(), content_files:BTreeMap::from([("index.html".into(),"<p>Speed</p>".into())]),
            entrypoint:"index.html".into(), readable_content:"Speed".into(), source_bindings:vec![], assumptions:vec![],
            state_contract:json!({"speed":"speed, 1..3 m/s"}), initial_state:json!({"speed":1}) };
        let candidate=private.create_presentation_candidate(&first.scope.chat_session_id,&first.scope.turn_id,None,content).unwrap();
        let reference=private.persist_presentation_candidate(&first.scope.chat_session_id,&first.scope.turn_id,&candidate.candidate_id).unwrap();
        let mut session=user.agent_history.sessions.iter().find(|s|s.id==first.scope.chat_session_id).unwrap().clone();
        let turn=session.turns.last_mut().unwrap();
        turn.status=AgentAssistantStatus::Completed;
        turn.outcome=Some(super::super::presentation_store_tests::outcome(&reference));
        turn.domain.effects.push(crate::session_event::EffectRecord {
            effect_id:format!("presentation:{}:1",reference.presentation_id), disposition:None,
            effect:crate::session_event::DeliveredEffect::Presentation {reference:reference.clone()},
        });
        let turn_ref=AgentTurnRef {session_id:session.id.clone(),turn_id:first.scope.turn_id.clone(),user_turn_ordinal:1};
        crate::session_runtime::finish(&mut user,&turn_ref,&session,"done").unwrap();
        reference
    };
    let mut port=f.run(&w,"om4");
    let stream=RunStream::new(RunDescriptor {book_id:f.x.book_id.clone(),session_id:port.scope.chat_session_id.clone(),turn_id:port.scope.turn_id.clone()});
    port.observe(&stream);
    let request=InteractRequest::Observe {board_target:None, reference:reference.clone()};
    let task=std::thread::spawn(move||{
        let result=port.interact_presentation(request,&CancellationToken::default());
        (port,result)
    });
    let wait=|| {
        let until=std::time::Instant::now()+Duration::from_secs(5);
        loop {
            if let Some(command)=stream.snapshot().presentation_command {break command;}
            assert!(std::time::Instant::now()<until);
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let command=wait();
    let claimed=f.action(&f.a,&w,"om4","presentation/interaction",json!({"operation":"claim","command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page"}));
    assert_eq!(claimed["result"]["claimed"],true);
    let saved=f.action(&f.a,&claimed,"om4","presentation/save",json!({"session_id":first.scope.chat_session_id,"turn_id":first.scope.turn_id,"reference":reference,
        "state":{"values":{"page":{"speed":1}},"visible_step":null,"observed_result":"10 metres","source_ref_ids":[]},
        "teacher_interaction":{"command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page","status":"observed","support":[]}}));
    let done=f.action(&f.a,&saved,"om4","presentation/interaction",json!({"operation":"complete","command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page","status":"observed","receipt":saved["result"]}));
    assert_eq!(done["result"]["accepted"],true);
    let (mut port,result)=task.join().unwrap();
    let result=result.unwrap();
    assert_eq!(result["state"]["observed_result"],"10 metres");
    let action=InteractRequest::Act {board_target:None, reference:reference.clone(),based_on:result["saved_state_ref"].as_str().unwrap().into(),
        action:PresentationAction::SetParameters {values:BTreeMap::from([("speed".into(),json!(2))])}};
    let task=std::thread::spawn(move||port.interact_presentation(action,&CancellationToken::default()));
    let command=wait();
    assert_eq!(command.expected_values["speed"],1);
    let claimed=f.action(&f.a,&done,"om4","presentation/interaction",json!({"operation":"claim","command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page"}));
    let saved=f.action(&f.a,&claimed,"om4","presentation/save",json!({"session_id":first.scope.chat_session_id,"turn_id":first.scope.turn_id,"reference":reference,
        "state":{"values":{"page":{"speed":2}},"visible_step":null,"observed_result":"20 metres","source_ref_ids":[]},
        "teacher_interaction":{"command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page","status":"applied","support":[]}}));
    f.action(&f.a,&saved,"om4","presentation/interaction",json!({"operation":"complete","command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"network-page","status":"applied","receipt":saved["result"]}));
    let result=task.join().unwrap().unwrap();
    assert_eq!(result["status"],"applied");
    assert_eq!(result["reference"]["revision"],1);
    assert_eq!(result["state"]["values"]["page"]["speed"],2);
}
