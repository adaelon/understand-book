use super::*;
use memory::learning::{TutorAction, TutorMaterial, TutorMaterialRole, TutorMutation};

fn multi_material_stage() -> (tempfile::TempDir, AppState, String) {
    let source = "Average speed includes all travel time. ".repeat(12);
    let (root, state) = crate::tests::tutor_loop_tests::fixture_with_book(
        true, Book::new(base_schema::sample_base(), &source), &source);
    let mut store = state.user.learning_store().unwrap();
    let before = store.state().unwrap();
    let stage = store.mutate(&TutorMutation {
        operation_id: "coach-two-materials".into(), expected_revision: before.control.revision,
        action: TutorAction::Start { user_intent: "比较两份材料对时间条件的说明".into(),
            explicit_constraints: vec![], default_teaching_intent: None, learning_purpose: None,
            material_scope: vec![
                TutorMaterial { source_id: "book-a".into(), scope_refs: vec![], role: TutorMaterialRole::Primary },
                TutorMaterial { source_id: "book-b".into(), scope_refs: vec![], role: TutorMaterialRole::Supporting },
            ] },
    }, "start").unwrap().control.current_tutor_session_id.unwrap();
    (root, state, stage)
}

fn bound_turn(stage: &str, source: &str, revision: &str, id: &str) -> (AgentTurnRef, Value) {
    let turn = AgentTurnRef { session_id: "coach-source-chat".into(), turn_id: id.into(), user_turn_ordinal: 1 };
    let binding = TeachingBinding { tutor_session_id: stage.into(), session_revision: 1, control_revision: 1,
        source_id: source.into(), source_revision: revision.into(), map_revision: None,
        chat_session_id: turn.session_id.clone(), turn_id: turn.turn_id.clone() };
    (turn, json!({"status":"active","binding":binding,"workspace_goal_revision":null}))
}

fn dependency_job(user: &user_runtime::UserRuntime, turn: &AgentTurnRef) -> CoachJob {
    let store = user.learning_store().unwrap();
    let reference = store.coach_turn_dependency(&turn.session_id, &turn.turn_id).unwrap().unwrap().job_ref.unwrap();
    store.coach_job(&reference).unwrap().unwrap()
}

#[test]
fn coach_bootstrap_switches_material_without_reusing_the_stage_original() {
    let (_root, state, stage) = multi_material_stage();
    let (a, context_a) = bound_turn(&stage, "book-a", "v1", "source-a-first");
    ensure_turn(&state.user, None, &a, Some(&context_a), "t1").unwrap();
    let first = dependency_job(&state.user, &a);
    let mut store = state.user.learning_store().unwrap();
    store.claim_coach_job(&first.input.job_ref, "t2").unwrap();
    store.finish_coach_job(&first.input.job_ref, 1, json!({"focus":"only book A"}), "t3").unwrap();

    let (b, context_b) = bound_turn(&stage, "book-b", "v1", "source-b-first");
    ensure_turn(&state.user, None, &b, Some(&context_b), "t4").unwrap();
    ensure_turn(&state.user, None, &b, Some(&context_b), "retry").unwrap();
    let second = dependency_job(&state.user, &b);
    assert_ne!(second.input.job_ref, first.input.job_ref);
    assert_eq!(second.input.binding.source_id, "book-b");
    assert_eq!(second.input.source_ref, b.turn_id);
    assert_eq!(second.status, CoachJobStatus::Pending);
    assert_eq!(store.list_coach_jobs(None, 20).unwrap().len(), 2);

    // Returning to A starts from the current request, not its older initial plan.
    let (again, context_again) = bound_turn(&stage, "book-a", "v1", "source-a-return");
    ensure_turn(&state.user, None, &again, Some(&context_again), "t5").unwrap();
    let returned = dependency_job(&state.user, &again);
    assert_ne!(returned.input.job_ref, first.input.job_ref);
    assert_eq!(returned.input.source_ref, again.turn_id);
    assert_eq!(returned.input.binding.source_id, "book-a");
    let (same, context_same) = bound_turn(&stage, "book-a", "v1", "source-a-next");
    ensure_turn(&state.user, None, &same, Some(&context_same), "t6").unwrap();
    assert_eq!(dependency_job(&state.user, &same).input.job_ref, returned.input.job_ref);
    assert_eq!(store.list_coach_jobs(None, 20).unwrap().len(), 3);
}

#[test]
fn coach_source_revision_change_starts_fresh_then_uses_latest_matching_after_turn() {
    let (_root, state, stage) = multi_material_stage();
    let (old, old_context) = bound_turn(&stage, "book-a", "v1", "revision-old");
    ensure_turn(&state.user, None, &old, Some(&old_context), "t0").unwrap();
    let old_start = dependency_job(&state.user, &old);
    let mut store = state.user.learning_store().unwrap();
    let mut completed = old_start.input.clone();
    completed.job_ref = "coach-after-old-revision".into();
    completed.origin = CoachJobOrigin::AfterTurn;
    completed.source_ref = "old-completed-turn".into();
    store.ensure_coach_job(&completed, "t1").unwrap();

    let (new, new_context) = bound_turn(&stage, "book-a", "v2", "revision-new");
    ensure_turn(&state.user, None, &new, Some(&new_context), "t2").unwrap();
    let fresh = dependency_job(&state.user, &new);
    assert_eq!(fresh.input.origin, CoachJobOrigin::StageStart);
    assert_eq!(fresh.input.binding.source_revision, "v2");
    assert_eq!(fresh.input.source_ref, new.turn_id);
    assert_ne!(fresh.input.job_ref, old_start.input.job_ref);

    let mut completed = fresh.input.clone();
    completed.job_ref = "coach-after-new-revision".into();
    completed.origin = CoachJobOrigin::AfterTurn;
    completed.source_ref = "new-completed-turn".into();
    store.ensure_coach_job(&completed, "t3").unwrap();
    let (next, next_context) = bound_turn(&stage, "book-a", "v2", "revision-next");
    ensure_turn(&state.user, None, &next, Some(&next_context), "t4").unwrap();
    assert_eq!(dependency_job(&state.user, &next).input.job_ref, completed.job_ref);
}

#[test]
fn coach_return_after_another_material_delivery_does_not_reuse_earlier_bootstrap() {
    let (_root, state, stage) = multi_material_stage();
    let (a, context_a) = bound_turn(&stage, "book-a", "v1", "a-original");
    ensure_turn(&state.user, None, &a, Some(&context_a), "t0").unwrap();
    let first = dependency_job(&state.user, &a);

    // B has a delivered round even though this host has no B bootstrap record.
    let (b, context_b) = bound_turn(&stage, "book-b", "v1", "b-delivered");
    let mut store = state.user.learning_store().unwrap();
    let after_b = CoachJobInput { job_ref: "coach-after-b".into(), origin: CoachJobOrigin::AfterTurn,
        binding: serde_json::from_value(context_b["binding"].clone()).unwrap(), source_ref: b.turn_id,
        workspace_goal_revision: None, context: json!({"actual_delivery":{"answer":"解释了B材料"}}) };
    store.ensure_coach_job(&after_b, "t1").unwrap();
    store.claim_coach_job(&after_b.job_ref, "t2").unwrap();
    store.finish_coach_job(&after_b.job_ref, 1, json!({"focus":"B的后续活动"}), "t3").unwrap();

    let (returned, context_returned) = bound_turn(&stage, "book-a", "v1", "a-after-b");
    ensure_turn(&state.user, None, &returned, Some(&context_returned), "t4").unwrap();
    let fresh = dependency_job(&state.user, &returned);
    assert_ne!(fresh.input.job_ref, first.input.job_ref);
    assert_eq!(fresh.input.source_ref, returned.turn_id);
    assert_eq!(fresh.input.context["preceding_coach_job_ref"], after_b.job_ref);

    let (same, context_same) = bound_turn(&stage, "book-a", "v1", "a-same-predecessor");
    ensure_turn(&state.user, None, &same, Some(&context_same), "t5").unwrap();
    assert_eq!(dependency_job(&state.user, &same).input.job_ref, fresh.input.job_ref);
    assert_eq!(store.list_coach_jobs(None, 20).unwrap().len(), 3);

    let mut newer_b = after_b;
    newer_b.job_ref = "coach-after-b-newer".into();
    newer_b.source_ref = "b-newer-delivery".into();
    store.ensure_coach_job(&newer_b, "t6").unwrap();
    let (later, context_later) = bound_turn(&stage, "book-a", "v1", "a-new-predecessor");
    ensure_turn(&state.user, None, &later, Some(&context_later), "t7").unwrap();
    let refreshed = dependency_job(&state.user, &later);
    assert_ne!(refreshed.input.job_ref, fresh.input.job_ref);
    assert_eq!(refreshed.input.context["preceding_coach_job_ref"], newer_b.job_ref);
}
