//! Host-owned teaching cadence. A committed turn creates one durable task;
//! foreground turns freeze only ready guidance and never wait for background work.
use crate::*;
use memory::teaching::{TeachingBinding, TeachingFact};
use memory::tutor_coach::{CoachJob, CoachJobInput, CoachJobOrigin, CoachJobStatus};
use runtime::run_context::{CancellationToken, CancellableAdapter};
use std::sync::{Arc, Mutex};

mod observations;

fn now() -> String { crate::multi_user_host::now().to_string() }
fn error(code: &str, message: &str) -> ToolError {
    crate::user_storage_paths::error(code, "unavailable", message)
}

fn media_question_context<T: Serialize>(question: Option<&T>) -> Value {
    let mut context = json!(question);
    // The coach receives source locators and delivery facts. Image pixels stay
    // in the original turn for the Resident's multimodal request, never as a
    // repeated base64 string in a text-only planning prompt.
    if let Some(image) = context.get_mut("image").and_then(Value::as_object_mut) {
        image.remove("png_base64");
    }
    context
}

fn binding_context(user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &str) -> Result<Option<Value>, ToolError> {
    match user.learning_store_for_workspace(workspace)?.teaching_event(&format!("binding:{turn}")) {
        Ok(event) => Ok(Some(event.payload)),
        Err(e) if e.error_code == "TEACHING_INVALID" => Ok(None),
        Err(e) => Err(e),
    }
}

/// Called after the original input and teaching binding have been saved. The
/// available guidance snapshot is frozen at admission; later jobs cannot hold up a reply.
pub(crate) fn ensure_turn(
    user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &AgentTurnRef,
    context: Option<&Value>, timestamp: &str,
) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(workspace)?;
    if store.coach_turn_dependency(&turn.session_id, &turn.turn_id)?.is_some() { return Ok(()); }
    let Some(context) = context.filter(|c| c["status"] == "practice") else {
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
                "music_question":media_question_context(saved.and_then(|t| t.music_question.as_ref())),
                "video_question":media_question_context(saved.and_then(|t| t.video_question.as_ref())),
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
                binding: binding.clone(), workspace_goal_revision: revision, context: packet,
            }, timestamp)?
        }
    };
    // Prefer the newest applicable ready result; a pending newest job must not
    // conceal an older ready plan for the same source and accepted goal.
    let ready = store.ready_coach_for_binding(&binding, revision)?;
    let selected = ready.as_ref().unwrap_or(&job);
    store.ensure_coach_turn_dependency(&turn.session_id, &turn.turn_id, Some(&selected.input.job_ref), timestamp)?;
    drop(store);
    prepare_execution(user, workspace, turn)?;
    Ok(())
}

/// This is reached only after all real delivery facts were appended. It also
/// repairs a saved answer whose original task registration was interrupted.
pub(crate) fn after_delivery(user: &user_runtime::UserRuntime, turn: &AgentTurnRef, timestamp: &str) -> Result<(), ToolError> {
    enqueue_delivery(user, turn, timestamp, false)
}

/// Explicit repair uses its own durable identity, leaving any old guidance intact.
pub(crate) fn enqueue_observation_backfill(user: &user_runtime::UserRuntime, turn: &AgentTurnRef, timestamp: &str) -> Result<(), ToolError> {
    enqueue_delivery(user, turn, timestamp, true)
}

pub(crate) fn freeze_observation_reads(user: &user_runtime::UserRuntime, job: &CoachJob, book: &Book, refresh: bool) -> Result<CoachJob, ToolError> {
    observations::freeze_original_reads(user, job, book, refresh)
}

/// Only an explicit maintenance retry may replay a failed/unknown paid attempt.
pub(crate) fn retry_observation_backfill(user: &user_runtime::UserRuntime, turn: &AgentTurnRef, timestamp: &str) -> Result<(), ToolError> {
    enqueue_observation_backfill(user, turn, timestamp)?;
    let session=user.agent_history.sessions.iter().find(|session|session.id==turn.session_id)
        .ok_or_else(||memory::teaching::invalid("原始对话不存在"))?;
    let mut store=user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?;
    let binding=store.teaching_event(&format!("binding:{}",turn.turn_id))?.binding;
    if let Some(job)=store.coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::ObservationBackfill,&turn.turn_id)? {
        if matches!(job.status,CoachJobStatus::Failed|CoachJobStatus::Interrupted) {
            store.retry_coach_job(&job.input.job_ref,timestamp)?;
        }
    }
    Ok(())
}

fn enqueue_delivery(user: &user_runtime::UserRuntime, turn: &AgentTurnRef, timestamp: &str, backfill: bool) -> Result<(), ToolError> {
    let Some(session) = user.agent_history.sessions.iter().find(|s| s.id == turn.session_id) else { return Ok(()); };
    let Some(saved) = session.turns.iter().find(|t| t.turn_id == turn.turn_id) else { return Ok(()); };
    let Some(outcome) = saved.outcome.as_ref().filter(|o| saved.status == AgentAssistantStatus::Completed
        && !crate::observability::lifecycle::delivery_failed(o)) else { return Ok(()); };
    let workspace = session.learning_workspace_id.as_deref();
    let Some(context) = binding_context(user, workspace, &turn.turn_id)?.filter(|c| matches!(c["status"].as_str(), Some("practice" | "course" | "observing"))) else { return Ok(()); };
    let binding: TeachingBinding = serde_json::from_value(context["binding"].clone())
        .map_err(|_| error("TUTOR_COACH_INPUT_INVALID", "教学回合缺少阶段身份。"))?;
    let mut store = user.learning_store_for_workspace(workspace)?;
    let origin = if backfill { CoachJobOrigin::ObservationBackfill } else { CoachJobOrigin::AfterTurn };
    if store.coach_job_for_source(&binding.tutor_session_id, origin, &turn.turn_id)?.is_some() { return Ok(()); }
    let events = store.teaching_turn_events(&binding.tutor_session_id, &turn.turn_id)?;
    if events.iter().any(|event| event.payload["outside"] == true) { return Ok(()); }
    let actual_delivery = events.iter().any(|event| event.kind == TeachingFact::MessageDelivered
        || event.payload["usage"]["kind"] == "assistant_delivery");
    if !actual_delivery { return Ok(()); }
    let observation_facts = observations::freeze_facts(&store, &events)?;
    if context["status"] != "practice" && observation_facts.is_empty() { return Ok(()); }
    let actual_design = events.iter().rev().find(|e| e.kind == TeachingFact::DesignRevised);
    let dialogue: Vec<_> = session.turns.iter().filter(|t| t.user_turn_ordinal <= saved.user_turn_ordinal)
        .rev().take(6).collect::<Vec<_>>().into_iter().rev().map(|t| json!({
            "turn_ref":t.turn_id,"user":t.domain.interaction.as_ref().map(|a|a.user_supplement.as_str()).unwrap_or(&t.user),"interaction":t.domain.interaction,
            "answer":t.outcome.as_ref().map(|o| &o.answer),"status":t.status,
            "music_question":media_question_context(t.music_question.as_ref()),
            "video_question":media_question_context(t.video_question.as_ref()),
        })).collect();
    let packet = json!({"trigger":if backfill {"observation_backfill"} else {"after_turn"},"source_turn_ref":turn.turn_id,
        "teaching_context":context,"observation_only":backfill,"current_design":actual_design,
        "dialogue":dialogue,"actual_delivery":{"relative_to_expression":"after","answer":outcome.answer,"answer_view":outcome.answer_view},
        "personal_notes":saved.domain.personal_notes,
        "known_focuses":store.focus_candidates(&binding.source_id,&binding.source_revision,24)?,
        "source_bindings":saved.source_bindings,"turn_facts":events,"observation_facts":observation_facts,
        "consumed_guidance":store.coach_turn_context(&turn.session_id,&turn.turn_id)?.and_then(|d|d.context),
        "publication":saved.published_book_ref,
        "music_question":media_question_context(saved.music_question.as_ref()),
        "video_question":media_question_context(saved.video_question.as_ref()),
        "workspace_goal":saved.domain.scene.as_ref().and_then(|s|s.workspace_goal.as_ref()),
        "provider_binding":saved.admission_input.as_ref().map(|f| f.coach_provider_binding()),
        "instruction":"All observations stop at source_turn_ref. The next user response has not happened. Observe this completed exchange; distinguish learner expressions from teacher support and question purposes. A teacher purpose is not learner evidence."});
    store.ensure_coach_job(&CoachJobInput {
        job_ref: format!("coach:{}:{}:{}", if backfill { "observation" } else { "after" }, binding.tutor_session_id, turn.turn_id),
        origin, source_ref: turn.turn_id.clone(),
        workspace_goal_revision:context["workspace_goal_revision"].as_u64().map(|v|v as u32),
        binding, context:packet,
    }, timestamp)?;
    Ok(())
}

fn guidance_applicable(user: &user_runtime::UserRuntime, job: &CoachJob) -> Result<bool, ToolError> {
    let state = user.learning_store_for_source(job.learning_workspace_id.as_deref(), &job.input.binding.source_id)?.state()?;
    let context = binding_context(user, job.learning_workspace_id.as_deref(), &job.input.binding.turn_id)?;
    Ok(state.workspace_goal_revision == job.input.workspace_goal_revision
        && context.is_some_and(|context| context["status"] == "practice"))
}

fn applicable(user: &user_runtime::UserRuntime, job: &CoachJob) -> Result<bool, ToolError> {
    if job.input.origin != CoachJobOrigin::StageStart && job.input.context.get("observation_facts").is_some() {
        // A later pause or change of teaching stage invalidates advice, not the
        // historical learner expression that still needs a durable observation.
        return Ok(true);
    }
    guidance_applicable(user, job)
}

/// Freeze what is available now. Background waiting, failures and spending
/// stops remain attached to their task and do not reject the next foreground turn.
pub(crate) fn prepare_execution(user: &user_runtime::UserRuntime, workspace: Option<&str>, turn: &AgentTurnRef) -> Result<bool, ToolError> {
    let mut store = user.learning_store_for_workspace(workspace)?;
    let Some(dependency) = store.coach_turn_dependency(&turn.session_id, &turn.turn_id)? else { return Ok(true); };
    if dependency.context.is_some() { return Ok(true); }
    let Some(job_ref) = dependency.job_ref else { return Ok(true); };
    let job = store.coach_job(&job_ref)?.ok_or_else(||error("TUTOR_COACH_TASK_MISSING","教学指导任务记录缺失。"))?;
    let context = if !guidance_applicable(user, &job)? {
        json!({"status":"unavailable","source_turn_ref":job.input.source_ref,"reason":"教学阶段已暂停或改变。"})
    } else { match job.status {
        CoachJobStatus::Pending | CoachJobStatus::Running => json!({"status":"unavailable","coach_job_ref":job.input.job_ref,
            "source_turn_ref":job.input.source_ref,"job_status":job.status,"reason":"后台指导尚未就绪，直接处理当前请求。"}),
        CoachJobStatus::Ready => json!({"status":"ready","coach_ref":job.guidance_ref,
            "source_turn_ref":job.input.source_ref,"origin":job.input.origin,"guidance":job.guidance}),
        _ => {
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
    claim_jobs(user, user.learning_store()?.list_coach_jobs_all_scopes(Some(CoachJobStatus::Pending), 32)?)
}

fn claim_for_source(user: &user_runtime::UserRuntime, book: &Book) -> Result<Option<CoachJob>, ToolError> {
    claim_jobs(user, user.learning_store()?.pending_coach_jobs_for_source(&book.base.book_id, book.source_fingerprint(), 32)?)
}

fn claim_jobs(user: &user_runtime::UserRuntime, jobs: Vec<CoachJob>) -> Result<Option<CoachJob>, ToolError> {
    for job in jobs {
        let mut store = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?;
        if !applicable(user, &job)? { store.cancel_coach_job(&job.input.job_ref, &now())?; continue; }
        if let Some(job) = store.claim_coach_job(&job.input.job_ref, &now())? { return Ok(Some(job)); }
    }
    Ok(None)
}

fn commit(user: &user_runtime::UserRuntime, job: &CoachJob, result: Result<Value, ToolError>) -> Result<(), ToolError> {
    commit_with_material(user, job, result, None)
}

fn commit_with_material(user: &user_runtime::UserRuntime, job: &CoachJob, result: Result<Value, ToolError>, material: Option<(&Book, &Path)>) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?;
    let current=store.coach_job(&job.input.job_ref)?;
    if !current.is_some_and(|current|current.status==CoachJobStatus::Running && current.attempt==job.attempt) {return Ok(());}
    if !applicable(user, job)? { store.cancel_coach_job(&job.input.job_ref, &now())?; return Ok(()); }
    match result {
        Ok(mut guidance) => {
            match observations::commit(user, job, &mut guidance, material) {
                Ok(()) => { store.finish_coach_job(&job.input.job_ref, job.attempt, guidance, &now())?; }
                Err(failure) if failure.category == "validation" || failure.error_code == "TEACHING_INVALID" => {
                    store.fail_coach_job(&job.input.job_ref, job.attempt, json!(failure), &now())?;
                }
                Err(failure) => return Err(failure),
            }
        }
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

fn execute(adapter: &dyn ModelAdapter, job: &CoachJob, book: &Book, cancel: &CancellationToken) -> Result<Value, ToolError> {
    cancel.check()?;
    adapter.set_model_purpose("tutor_coach");
    adapter.set_run_cancellation(cancel.clone());
    let adapter = CancellableAdapter { inner:adapter, cancellation:cancel.clone() };
    #[cfg(test)]
    let candidates = std::cell::RefCell::new(Vec::<Value>::new());
    let generated = runtime::tutor_coach::execute_coach_with_validation(&adapter, job.input.context.clone(), |guidance| {
        let result = observations::validate_material(job, guidance, book);
        #[cfg(test)]
        candidates.borrow_mut().push(json!({"guidance":guidance,"error":result.as_ref().err().map(|e|&e.message)}));
        result.map_err(|e|runtime::AdapterError {spend_stop:None,message:e.message})
    });
    // Opt-in configured-provider tests keep the exact hydrated input and public
    // result even when later evidence acceptance rejects the candidate.
    #[cfg(test)]
    if let Ok(directory)=std::env::var("COURSE_FEEDBACK_CAPTURE") {
        let directory=std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("course trial capture directory");
        let capture=json!({"job_ref":job.input.job_ref,"attempt":job.attempt,"input":job.input.context,
            "model_input":runtime::tutor_coach::model_input(&job.input.context),"evidence_candidates":candidates.into_inner(),
            "guidance":generated.as_ref().ok(),"error":generated.as_ref().err().map(|e|&e.message)});
        std::fs::write(directory.join(format!("{}.json",uuid::Uuid::now_v7())),serde_json::to_vec_pretty(&capture).unwrap()).expect("course trial capture");
    }
    let result = generated.map_err(|e| e.spend_stop.map(|s|s.tool_error())
            .unwrap_or_else(||error("TUTOR_COACH_FAILED", &e.message)))?;
    cancel.check()?;
    serde_json::to_value(result).map_err(|e|error("TUTOR_COACH_FAILED", &e.to_string()))
}

// Understanding extraction can include one structured-output correction. It
// runs in the background and must not inherit the short selection-translation
// deadline, which cancelled real coach responses around the one-minute mark.
const COACH_TASK_TIMEOUT: Duration = Duration::from_secs(180);

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
                if parent.is_cancelled() || !active() || started.elapsed() >= COACH_TASK_TIMEOUT {
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
        let material=port.with_app(|state|(state.workspace.book.clone(),state.workspace.book_dir.clone()));
        let prepared=port.with_app(|state|observations::freeze_original_reads(&state.user,&job,&material.0,false));
        let current=prepared.as_ref().unwrap_or(&job);
        let result=prepared.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()})
            .and_then(|job|observations::with_material(job,&material.0,&material.1)).and_then(|job|execute(adapter,&job,&material.0,cancel));
        adapter.set_model_purpose("resident");
        save_result(cancel,result,|result|port.with_app(|state|commit_with_material(&state.user,current,result,Some((&material.0,&material.1)))))?;
    }
    Ok(())
}

pub(crate) fn tick_network(access: &Arc<authorization::Authorization>, handle: &user_registry::UserHandle, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (owner, job) = {
        let user = handle.lock().unwrap();
        let Some(job) = claim(&user)? else { return Ok(false); };
        (user.user_id().expect("network owner").to_owned(), job)
    };
    let publication = serde_json::from_value::<published_library::PublishedBookRef>(job.input.context["publication"].clone())
        .map_err(|_|error("TUTOR_COACH_SOURCE_UNAVAILABLE","教练材料来源已不可用。"))
        .and_then(|reference|access.library.lock().unwrap().load(&owner,&reference));
    let prepared=publication.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()}).and_then(|publication|
        observations::freeze_original_reads(&handle.lock().unwrap(),&job,&publication.book,false));
    let job=prepared.as_ref().unwrap_or(&job);
    let result = with_task_cancellation(cancel,
        || applicable(&handle.lock().unwrap(),&job).unwrap_or(false), |cancel| {
        let publication = publication.as_ref().map_err(|e| error(&e.error_code,&e.message))?;
        prepared.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()})?;
        if !job.input.context["provider_binding"].is_null()
            && job.input.context["provider_binding"] != access.runs.coach_provider_binding()? {
            return Err(error("PROVIDER_BINDING_UNAVAILABLE","教练任务的模型配置已改变，请重试。"));
        }
        let adapter = access.runs.review_adapter()?;
        let scope = runtime::model_spend::ChargeScope::ReaderTask { user_id:owner.clone(),
            task_ref:format!("{}:{}",job.input.job_ref,job.attempt) };
        adapter.set_spend_context(scope.clone(),Some(access.spend.port_with_attribution(scope,publication.reference.clone(),
            runtime::model_spend::ChargeAttribution {parent_task_ref:job.input.job_ref.clone(),
                learning_workspace_id:job.learning_workspace_id.clone(),related_learning_workspace_ids:vec![]},cancel.clone())));
        let limited = service_limits::LimitedAdapter { inner:adapter.as_ref(),resources:access.resources.clone(),
            owner:owner.clone(),background:true,cancellation:cancel.clone(),usage:Mutex::new(Default::default()),stream:None,
            authorization:Some((access,&publication.reference)) };
        execute(&limited,&observations::with_material(job,&publication.book,publication.directory())?,&publication.book,cancel)
    });
    save_result(cancel,result,|result|commit_with_material(&handle.lock().unwrap(),job,result,
        publication.as_ref().ok().map(|publication|(publication.book.as_ref(),publication.directory()))))?;
    Ok(true)
}

pub(crate) fn tick_local(state: &Arc<Mutex<AppState>>, provider: Option<ProviderConfig>, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (job,book,directory) = { let state=state.lock().unwrap();
        if !state.user.store.private_storage_available() { return Ok(false); }
        let Some(job)=claim_for_source(&state.user, &state.workspace.book)? else {return Ok(false);};
        (job,state.workspace.book.clone(),state.workspace.book_dir.clone()) };
    let prepared=observations::freeze_original_reads(&state.lock().unwrap().user,&job,&book,false);
    let job=prepared.as_ref().unwrap_or(&job);
    let result = with_task_cancellation(cancel,
        || applicable(&state.lock().unwrap().user,&job).unwrap_or(false), |cancel| {
        provider.ok_or_else(||error("TUTOR_COACH_PROVIDER_UNCONFIGURED","教练模型尚未配置。"))
            .and_then(|config| {
            prepared.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()})?;
            let adapter=ProviderRegistry::adapter_from_config_with_timeout(config,COACH_TASK_TIMEOUT);
            execute(adapter.as_ref(),&observations::with_material(job,&book,&directory)?,&book,cancel)
        })
    });
    save_result(cancel,result,|result|commit_with_material(&state.lock().unwrap().user,job,result,Some((&book,&directory))))?;
    Ok(true)
}

pub(crate) fn recover_user(user: &user_runtime::UserRuntime) -> Result<(), ToolError> {
    if !user.store.private_storage_available() { return Ok(()); }
    for workspace in user.learning_store()?.coach_job_workspaces()? {
        user.learning_store_for_workspace(workspace.as_deref())?.recover_interrupted_coach_jobs(&now())?;
    }
    // Repair the latest completed exchange under the current goal. Legacy
    // teaching stages are historical records, not new work to schedule.
    for session in &user.agent_history.sessions {
        let candidates:Vec<_>=session.turns.iter().rev().filter(|t|t.status==AgentAssistantStatus::Completed
            && t.outcome.as_ref().is_some_and(|o|!crate::observability::lifecycle::delivery_failed(o))).collect();
        let mut observed=false;
        for turn in candidates {
            let Some(context)=binding_context(user,session.learning_workspace_id.as_deref(),&turn.turn_id)? else {continue;};
            let (Some(stage),Some(source))=(context["binding"]["tutor_session_id"].as_str(),
                context["binding"]["source_id"].as_str()) else {continue;};
            let current=user.learning_store_for_source(session.learning_workspace_id.as_deref(),source)?.state()?;
            if !matches!(context["status"].as_str(), Some("observing" | "practice" | "course"))
                || context["workspace_goal_revision"] != json!(current.workspace_goal_revision) {
                continue;
            }
            let complete_active=context["status"]=="practice" && !turn.outcome.as_ref().unwrap().incomplete;
            if !observed || complete_active {
                // The answer can be durable before its delivery trace and job
                // registration. This idempotent entry repairs both in order.
                crate::teaching::record_user_delivery(user,&AgentTurnRef {session_id:session.id.clone(),
                    turn_id:turn.turn_id.clone(),user_turn_ordinal:turn.user_turn_ordinal},&session.updated_at)?;
                observed |= user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?
                    .coach_job_for_source(stage,CoachJobOrigin::AfterTurn,&turn.turn_id)?.is_some();
            }
            if complete_active {break;}
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

#[cfg(test)]
mod observation_tests;
