//! Host-owned teaching cadence. A committed turn creates one durable task;
//! the following turn consumes its result before the Resident starts sampling.
use crate::*;
use memory::teaching::{TeachingBinding, TeachingFact};
use memory::tutor_coach::{CoachJob, CoachJobInput, CoachJobOrigin, CoachJobStatus};
use runtime::run_context::{CancellationToken, CancellableAdapter};
use std::sync::{Arc, Mutex};

fn now() -> String { crate::multi_user_host::now().to_string() }
fn error(code: &str, message: &str) -> ToolError {
    crate::user_storage_paths::error(code, "unavailable", message)
}

fn binding_context(user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &str) -> Result<Option<Value>, ToolError> {
    match user.learning_store_for_workspace(workspace)?.teaching_event(&format!("binding:{turn}")) {
        Ok(event) => Ok(Some(event.payload)),
        Err(e) if e.error_code == "TEACHING_INVALID" => Ok(None),
        Err(e) => Err(e),
    }
}

/// Called after the original input and teaching binding have been saved. The
/// dependency, not an opportunistic "latest result", is frozen at admission.
pub(crate) fn ensure_turn(
    user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &AgentTurnRef,
    context: Option<&Value>, timestamp: &str,
) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(workspace)?;
    if store.coach_turn_dependency(&turn.session_id, &turn.turn_id)?.is_some() { return Ok(()); }
    let Some(context) = context.filter(|c| c["status"] == "active") else {
        store.ensure_coach_turn_dependency(&turn.session_id, &turn.turn_id, None, timestamp)?;
        return Ok(());
    };
    let binding: TeachingBinding = serde_json::from_value(context["binding"].clone())
        .map_err(|_| error("TUTOR_COACH_INPUT_INVALID", "教学指导缺少已接纳的阶段。"))?;
    let revision = context["workspace_goal_revision"].as_u64().map(|v| v as u32);
    let matches_current_source = |job: &CoachJob| job.input.binding.source_id == binding.source_id
            && job.input.binding.source_revision == binding.source_revision
            && job.input.workspace_goal_revision == revision;
    let latest_after = store.latest_coach_job(&binding.tutor_session_id, CoachJobOrigin::AfterTurn)?;
    let preceding_coach_job_ref = latest_after.as_ref().map(|job|job.input.job_ref.as_str());
    let latest = latest_after.as_ref().filter(|job|matches_current_source(job)).cloned();
    let bootstrap = store.latest_coach_job(&binding.tutor_session_id, CoachJobOrigin::StageStart)?;
    // Re-entering a material after a different source needs a current starting
    // plan, rather than the stage's original, source-bound bootstrap.
    let reusable = latest.or_else(|| bootstrap.as_ref().filter(|job|matches_current_source(job)
        && job.input.context["preceding_coach_job_ref"].as_str() == preceding_coach_job_ref).cloned());
    let job = match reusable {
        Some(job) => job,
        None => {
            let session = user.agent_history.sessions.iter().find(|s| s.id == turn.session_id);
            let saved = session.and_then(|s| s.turns.iter().find(|t| t.turn_id == turn.turn_id));
            let packet = json!({"trigger":"stage_start", "source_turn_ref":null,
                "preceding_coach_job_ref":preceding_coach_job_ref,
                "stage_ref":binding.tutor_session_id, "teaching_context":context,
                "current_request":saved.map(|t| t.domain.interaction.as_ref().map(|a|a.user_supplement.as_str()).unwrap_or(&t.user)),
                "accepted_action":saved.and_then(|t|t.domain.interaction.as_ref()),
                "publication":saved.and_then(|t| t.published_book_ref.as_ref()),
                "music_question":saved.and_then(|t| t.music_question.as_ref()),
                "workspace_goal":saved.and_then(|t| t.domain.scene.as_ref()).and_then(|s| s.workspace_goal.as_ref()),
                "provider_binding":saved.and_then(|t| t.admission_input.as_ref()).map(|f| f.coach_provider_binding()),
                "instruction":"Stage start guidance only. Source locations are not source text; specify reading needs when the actual material has not been read."});
            let (job_ref, source_ref) = if bootstrap.is_some() {
                (format!("coach:start:{}:{}", binding.tutor_session_id, turn.turn_id), turn.turn_id.clone())
            } else {
                (format!("coach:start:{}", binding.tutor_session_id), binding.tutor_session_id.clone())
            };
            store.ensure_coach_job(&CoachJobInput {
                job_ref,
                origin: CoachJobOrigin::StageStart, source_ref,
                binding, workspace_goal_revision: revision, context: packet,
            }, timestamp)?
        }
    };
    store.ensure_coach_turn_dependency(&turn.session_id, &turn.turn_id, Some(&job.input.job_ref), timestamp)?;
    Ok(())
}

/// This is reached only after all real delivery facts were appended. It also
/// repairs a saved answer whose original task registration was interrupted.
pub(crate) fn after_delivery(user: &user_runtime::UserRuntime, turn: &AgentTurnRef, timestamp: &str) -> Result<(), ToolError> {
    let Some(session) = user.agent_history.sessions.iter().find(|s| s.id == turn.session_id) else { return Ok(()); };
    let Some(saved) = session.turns.iter().find(|t| t.turn_id == turn.turn_id) else { return Ok(()); };
    let Some(outcome) = saved.outcome.as_ref().filter(|o| saved.status == AgentAssistantStatus::Completed
        && !o.incomplete && !crate::observability::lifecycle::delivery_failed(o)) else { return Ok(()); };
    let workspace = session.learning_workspace_id.as_deref();
    let Some(context) = binding_context(user, workspace, &turn.turn_id)?.filter(|c| c["status"] == "active") else { return Ok(()); };
    let binding: TeachingBinding = serde_json::from_value(context["binding"].clone())
        .map_err(|_| error("TUTOR_COACH_INPUT_INVALID", "教学回合缺少阶段身份。"))?;
    let mut store = user.learning_store_for_workspace(workspace)?;
    if store.coach_job_for_source(&binding.tutor_session_id, CoachJobOrigin::AfterTurn, &turn.turn_id)?.is_some() { return Ok(()); }
    let events = store.teaching_turn_events(&binding.tutor_session_id, &turn.turn_id)?;
    if events.iter().any(|event| event.payload["outside"] == true) { return Ok(()); }
    let actual_delivery = events.iter().any(|event| event.kind == TeachingFact::MessageDelivered
        || event.payload["usage"]["kind"] == "assistant_delivery");
    if !actual_delivery { return Ok(()); }
    let actual_design = events.iter().rev().find(|e| e.kind == TeachingFact::DesignRevised);
    let dialogue: Vec<_> = session.turns.iter().filter(|t| t.user_turn_ordinal <= saved.user_turn_ordinal)
        .rev().take(6).collect::<Vec<_>>().into_iter().rev().map(|t| json!({
            "turn_ref":t.turn_id,"user":t.domain.interaction.as_ref().map(|a|a.user_supplement.as_str()).unwrap_or(&t.user),"interaction":t.domain.interaction,
            "answer":t.outcome.as_ref().map(|o| &o.answer),"status":t.status,
            "music_question":t.music_question,
        })).collect();
    let packet = json!({"trigger":"after_turn","source_turn_ref":turn.turn_id,
        "teaching_context":context,"current_design":actual_design,
        "dialogue":dialogue,"actual_delivery":{"answer":outcome.answer,"answer_view":outcome.answer_view},
        "source_bindings":saved.source_bindings,"turn_facts":events,
        "consumed_guidance":store.coach_turn_context(&turn.session_id,&turn.turn_id)?.and_then(|d|d.context),
        "publication":saved.published_book_ref,
        "music_question":saved.music_question,
        "workspace_goal":saved.domain.scene.as_ref().and_then(|s|s.workspace_goal.as_ref()),
        "provider_binding":saved.admission_input.as_ref().map(|f| f.coach_provider_binding()),
        "instruction":"All observations stop at source_turn_ref. The next user response has not happened. Plan conditional next steps; distinguish actual learner expressions from teacher support."});
    store.ensure_coach_job(&CoachJobInput {
        job_ref: format!("coach:after:{}:{}", binding.tutor_session_id, turn.turn_id),
        origin: CoachJobOrigin::AfterTurn, source_ref: turn.turn_id.clone(),
        workspace_goal_revision:context["workspace_goal_revision"].as_u64().map(|v|v as u32),
        binding, context:packet,
    }, timestamp)?;
    Ok(())
}

fn applicable(user: &user_runtime::UserRuntime, job: &CoachJob) -> Result<bool, ToolError> {
    let state = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?.state()?;
    Ok(state.control.enabled && state.control.current_tutor_session_id.as_deref() == Some(&job.input.binding.tutor_session_id)
        && state.workspace_goal_revision == job.input.workspace_goal_revision
        && state.sessions.get(&job.input.binding.tutor_session_id).is_some_and(|stage|
            stage.status == memory::learning::TutorSessionStatus::Active
            && stage.material_scope.iter().any(|m|m.source_id == job.input.binding.source_id)))
}

/// Return false only while the exact dependency is still running/queued. Called
/// before claiming a network Run, or before starting a local model request.
pub(crate) fn prepare_execution(user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &AgentTurnRef) -> Result<bool, ToolError> {
    let mut store = user.learning_store_for_workspace(workspace)?;
    let Some(dependency) = store.coach_turn_dependency(&turn.session_id, &turn.turn_id)? else { return Ok(true); };
    if dependency.context.is_some() { return Ok(true); }
    let Some(job_ref) = dependency.job_ref else { return Ok(true); };
    let job = store.coach_job(&job_ref)?.ok_or_else(||error("TUTOR_COACH_TASK_MISSING","教学指导任务记录缺失。"))?;
    let context = if !applicable(user, &job)? {
        store.cancel_coach_job(&job_ref, &now())?;
        json!({"status":"unavailable","source_turn_ref":job.input.source_ref,"reason":"教学阶段已暂停或改变。"})
    } else { match job.status {
        CoachJobStatus::Pending | CoachJobStatus::Running => return Ok(false),
        CoachJobStatus::Ready => json!({"status":"ready","coach_ref":job.guidance_ref,
            "source_turn_ref":job.input.source_ref,"origin":job.input.origin,"guidance":job.guidance}),
        _ => {
            if let Some(code) = job.error.as_ref().and_then(|e|e["error_code"].as_str()) {
                if let Some(stop) = runtime::model_spend::SpendStop::from_code(code) { return Err(stop.tool_error()); }
                if job.error.as_ref().is_some_and(|e|e["category"] == "permission") {
                    return Err(ToolError {error_code:code.into(),category:"permission".into(),
                        message:"教练调用权限不可用。".into()});
                }
            }
            json!({"status":"unavailable","coach_job_ref":job.input.job_ref,
                "source_turn_ref":job.input.source_ref,"reason":job.error,"job_status":job.status})
        }
    }};
    store.freeze_coach_turn_context(&turn.session_id, &turn.turn_id, context, &now())?;
    Ok(true)
}

pub(crate) fn turn_context(user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &AgentTurnRef) -> Result<Option<Value>, ToolError> {
    Ok(user.learning_store_for_workspace(workspace)?.coach_turn_context(&turn.session_id,&turn.turn_id)?.and_then(|d| d.context))
}

fn claim(user: &user_runtime::UserRuntime) -> Result<Option<CoachJob>, ToolError> {
    for job in user.learning_store()?.list_coach_jobs_all_scopes(Some(CoachJobStatus::Pending), 32)? {
        let mut store = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?;
        if !applicable(user, &job)? { store.cancel_coach_job(&job.input.job_ref, &now())?; continue; }
        if let Some(job) = store.claim_coach_job(&job.input.job_ref, &now())? { return Ok(Some(job)); }
    }
    Ok(None)
}

fn commit(user: &user_runtime::UserRuntime, job: &CoachJob, result: Result<Value, ToolError>) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?;
    let current=store.coach_job(&job.input.job_ref)?;
    if !current.is_some_and(|current|current.status==CoachJobStatus::Running && current.attempt==job.attempt) {return Ok(());}
    if !applicable(user, job)? { store.cancel_coach_job(&job.input.job_ref, &now())?; return Ok(()); }
    match result {
        Ok(guidance) => { store.finish_coach_job(&job.input.job_ref, job.attempt, guidance, &now())?; }
        Err(error) => { store.fail_coach_job(&job.input.job_ref, job.attempt, json!(error), &now())?; }
    }
    Ok(())
}

// A completed provider call stays owned by this worker until its result is
// saved. A transient database failure must not leave an orphaned Running job
// or turn into a second paid model call.
fn save_result(
    cancel: &CancellationToken, result: Result<Value, ToolError>,
    mut save: impl FnMut(Result<Value, ToolError>) -> Result<(), ToolError>,
) -> Result<(), ToolError> {
    let mut reported = false;
    loop {
        let attempt=result.as_ref().map(Clone::clone).map_err(|error|ToolError {
            error_code:error.error_code.clone(),category:error.category.clone(),message:error.message.clone(),
        });
        match save(attempt) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if cancel.is_cancelled() { return Err(error); }
                if !reported { eprintln!("Teaching coach result pending persistence: {}", error.error_code); reported = true; }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn execute(adapter: &dyn ModelAdapter, job: &CoachJob, cancel: &CancellationToken) -> Result<Value, ToolError> {
    cancel.check()?;
    adapter.set_model_purpose("tutor_coach");
    adapter.set_run_cancellation(cancel.clone());
    let adapter = CancellableAdapter { inner:adapter, cancellation:cancel.clone() };
    let result = runtime::tutor_coach::execute_coach(&adapter, job.input.context.clone())
        .map_err(|e| e.spend_stop.map(|s|s.tool_error())
            .unwrap_or_else(||error("TUTOR_COACH_FAILED", &e.message)))?;
    cancel.check()?;
    serde_json::to_value(result).map_err(|e|error("TUTOR_COACH_FAILED", &e.to_string()))
}

fn with_task_cancellation(
    parent: &CancellationToken, active: impl Fn() -> bool + Sync,
    run: impl FnOnce(&CancellationToken) -> Result<Value, ToolError>,
) -> Result<Value, ToolError> {
    let cancel=CancellationToken::default();
    let done=std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let started=std::time::Instant::now();
            while !done.load(std::sync::atomic::Ordering::Acquire) {
                if parent.is_cancelled() || !active() || started.elapsed() >= crate::SELECTION_TRANSLATION_TIMEOUT {
                    cancel.cancel(); break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        let result=run(&cancel);
        done.store(true,std::sync::atomic::Ordering::Release);
        result
    })
}

/// The synchronous in-process API has no host worker. It executes the same
/// durable task directly; desktop/network hosts use their background worker.
pub(crate) fn drive_inline(
    port: &impl crate::agent_run::AppStatePort, adapter: &dyn ModelAdapter,
    workspace: Option<&str>, turn: &AgentTurnRef, after: bool, cancel: &CancellationToken,
) -> Result<(), ToolError> {
    let job=port.with_app(|state| {
        let mut store=state.user.learning_store_for_workspace(workspace)?;
        let job=if after {
            let Some(context)=binding_context(&state.user,workspace,&turn.turn_id)? else {return Ok(None);};
            let Some(stage)=context["binding"]["tutor_session_id"].as_str() else {return Ok(None);};
            store.coach_job_for_source(stage,CoachJobOrigin::AfterTurn,&turn.turn_id)?
        } else {
            match store.coach_turn_dependency(&turn.session_id,&turn.turn_id)?.and_then(|d|d.job_ref) {
                Some(reference)=>store.coach_job(&reference)?,None=>None,
            }
        };
        let Some(job)=job else {return Ok(None);};
        if !applicable(&state.user,&job)? {store.cancel_coach_job(&job.input.job_ref,&now())?;return Ok(None);}
        store.claim_coach_job(&job.input.job_ref,&now())
    })?;
    if let Some(job)=job {
        let result=execute(adapter,&job,cancel);
        adapter.set_model_purpose("resident");
        save_result(cancel,result,|result|port.with_app(|state|commit(&state.user,&job,result)))?;
    }
    Ok(())
}

pub(crate) fn tick_network(access: &Arc<authorization::Authorization>, handle: &user_registry::UserHandle, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (owner, job) = {
        let user = handle.lock().unwrap();
        let Some(job) = claim(&user)? else { return Ok(false); };
        (user.user_id().expect("network owner").to_owned(), job)
    };
    let result = with_task_cancellation(cancel,
        || applicable(&handle.lock().unwrap(),&job).unwrap_or(false), |cancel| {
        let publication: published_library::PublishedBookRef = serde_json::from_value(job.input.context["publication"].clone())
            .map_err(|_|error("TUTOR_COACH_SOURCE_UNAVAILABLE","教练材料来源已不可用。"))?;
        if !job.input.context["provider_binding"].is_null()
            && job.input.context["provider_binding"] != access.runs.coach_provider_binding()? {
            return Err(error("PROVIDER_BINDING_UNAVAILABLE","教练任务的模型配置已改变，请重试。"));
        }
        let adapter = access.runs.task_adapter()?;
        let scope = runtime::model_spend::ChargeScope::ReaderTask { user_id:owner.clone(),
            task_ref:format!("{}:{}",job.input.job_ref,job.attempt) };
        adapter.set_spend_context(scope.clone(),Some(access.spend.port_with_attribution(scope,publication.clone(),
            runtime::model_spend::ChargeAttribution {parent_task_ref:job.input.job_ref.clone(),
                learning_workspace_id:job.learning_workspace_id.clone(),related_learning_workspace_ids:vec![]},cancel.clone())));
        let limited = service_limits::LimitedAdapter { inner:adapter.as_ref(),resources:access.resources.clone(),
            owner,cancellation:cancel.clone(),usage:Mutex::new(Default::default()),stream:None,
            authorization:Some((access,&publication)) };
        execute(&limited,&job,cancel)
    });
    save_result(cancel,result,|result|commit(&handle.lock().unwrap(),&job,result))?;
    Ok(true)
}

pub(crate) fn tick_local(state: &Arc<Mutex<AppState>>, provider: Option<ProviderConfig>, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let job = { let state=state.lock().unwrap();
        if !state.user.store.private_storage_available() { return Ok(false); }
        let Some(job)=claim(&state.user)? else {return Ok(false);}; job };
    let result = with_task_cancellation(cancel,
        || applicable(&state.lock().unwrap().user,&job).unwrap_or(false), |cancel| {
        provider.ok_or_else(||error("TUTOR_COACH_PROVIDER_UNCONFIGURED","教练模型尚未配置。"))
            .and_then(|config| {
            let adapter=ProviderRegistry::adapter_from_config_with_timeout(config,crate::SELECTION_TRANSLATION_TIMEOUT);
            execute(adapter.as_ref(),&job,cancel)
        })
    });
    save_result(cancel,result,|result|commit(&state.lock().unwrap().user,&job,result))?;
    Ok(true)
}

pub(crate) fn recover_user(user: &user_runtime::UserRuntime) -> Result<(), ToolError> {
    if !user.store.private_storage_available() { return Ok(()); }
    for workspace in user.learning_store()?.coach_job_workspaces()? {
        user.learning_store_for_workspace(workspace.as_deref())?.recover_interrupted_coach_jobs(&now())?;
    }
    // Only the most recent successful teaching delivery in the active stage
    // needs repair. An incomplete answer or a later ordinary question is not a
    // replacement for that delivery.
    for session in &user.agent_history.sessions {
        let store=user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?;
        let view=store.state()?;
        let Some(stage)=view.control.current_tutor_session_id.as_ref().filter(|id|
            view.control.enabled && view.sessions.get(*id).is_some_and(|s|s.status==memory::learning::TutorSessionStatus::Active)) else {continue;};
        for turn in session.turns.iter().rev().filter(|t|t.status==AgentAssistantStatus::Completed
            && t.outcome.as_ref().is_some_and(|o|!o.incomplete && !crate::observability::lifecycle::delivery_failed(o))) {
            let Some(context)=binding_context(user,session.learning_workspace_id.as_deref(),&turn.turn_id)? else {continue;};
            if context["status"]!="active" || context["binding"]["tutor_session_id"].as_str()!=Some(stage.as_str()) {continue;}
            crate::teaching::record_user_delivery(user,&AgentTurnRef {session_id:session.id.clone(),
                turn_id:turn.turn_id.clone(),user_turn_ordinal:turn.user_turn_ordinal},&session.updated_at)?;
            if store.coach_job_for_source(stage,CoachJobOrigin::AfterTurn,&turn.turn_id)?.is_some() {break;}
        }
    }
    Ok(())
}

pub(crate) fn recover_local(state: &Arc<Mutex<AppState>>) -> Result<(), ToolError> {
    recover_user(&state.lock().unwrap().user)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod source_tests;
