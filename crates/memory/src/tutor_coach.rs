//! System-owned coaching jobs and the exact guidance frozen for a teaching turn.
use crate::learning::{storage, LearningStore};
use crate::teaching::{TeachingBinding, TeachingEvent, TeachingFact};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const SCHEMA: &str = "
CREATE TABLE teaching_coach_jobs (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    learning_workspace_id TEXT NOT NULL,
    job_ref TEXT NOT NULL,
    session_id TEXT NOT NULL,
    origin TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    status TEXT NOT NULL,
    job TEXT NOT NULL,
    UNIQUE(learning_workspace_id, job_ref),
    UNIQUE(learning_workspace_id, session_id, origin, source_ref));
CREATE INDEX teaching_coach_jobs_pending ON teaching_coach_jobs(learning_workspace_id, status, seq);
CREATE TABLE teaching_coach_turns (
    learning_workspace_id TEXT NOT NULL,
    chat_session_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    context TEXT NOT NULL,
    PRIMARY KEY(learning_workspace_id, chat_session_id, turn_id));";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoachJobOrigin { StageStart, AfterTurn, ObservationBackfill }

impl CoachJobOrigin {
    fn as_str(self) -> &'static str {
        match self { Self::StageStart => "stage_start", Self::AfterTurn => "after_turn", Self::ObservationBackfill => "observation_backfill" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoachJobStatus { Pending, Running, Ready, Failed, Interrupted, Cancelled }

impl CoachJobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending", Self::Running => "running", Self::Ready => "ready",
            Self::Failed => "failed", Self::Interrupted => "interrupted", Self::Cancelled => "cancelled",
        }
    }
}

/// Host-frozen input. Source contents, publication and model bindings stay in context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachJobInput {
    pub job_ref: String,
    pub origin: CoachJobOrigin,
    pub binding: TeachingBinding,
    pub source_ref: String,
    pub workspace_goal_revision: Option<u32>,
    pub context: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachJobAttempt {
    pub number: u32,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: CoachJobStatus,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachJob {
    pub learning_workspace_id: Option<String>,
    pub input: CoachJobInput,
    pub status: CoachJobStatus,
    pub attempt: u32,
    pub attempts: Vec<CoachJobAttempt>,
    pub guidance_ref: Option<String>,
    pub guidance: Option<Value>,
    pub error: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachTurnDependency {
    pub chat_session_id: String,
    pub turn_id: String,
    pub job_ref: Option<String>,
    /// None while waiting. Once frozen, retries reuse the exact first context.
    pub context: Option<Value>,
    pub created_at: String,
    pub frozen_at: Option<String>,
}

fn conflict(message: &str) -> ToolError {
    ToolError { error_code: "TUTOR_COACH_CONFLICT".into(), category: "conflict".into(), message: message.into() }
}

fn get_job(connection: &Connection, scope: &str, job_ref: &str) -> Result<Option<CoachJob>, ToolError> {
    let json: Option<String> = connection.query_row(
        "SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND job_ref=?",
        params![scope,job_ref], |row|row.get(0)).optional().map_err(storage)?;
    json.map(|json|serde_json::from_str(&json).map_err(storage)).transpose()
}

fn required_job(connection: &Connection, scope: &str, job_ref: &str) -> Result<CoachJob, ToolError> {
    get_job(connection, scope, job_ref)?.ok_or_else(||conflict("教练任务不存在。"))
}

fn save_job(connection: &Connection, scope: &str, job: &CoachJob) -> Result<(), ToolError> {
    connection.execute("UPDATE teaching_coach_jobs SET status=?,job=? WHERE learning_workspace_id=? AND job_ref=?",
        params![job.status.as_str(),serde_json::to_string(job).map_err(storage)?,scope,job.input.job_ref]).map_err(storage)?;
    Ok(())
}

fn close_attempt(job: &mut CoachJob, status: CoachJobStatus, error: Option<Value>, now: &str) {
    job.status = status;
    job.error = error.clone();
    job.updated_at = now.into();
    if let Some(attempt) = job.attempts.last_mut().filter(|attempt|attempt.status == CoachJobStatus::Running) {
        attempt.status = status;
        attempt.finished_at = Some(now.into());
        attempt.error = error;
    }
}

fn get_turn(connection: &Connection, scope: &str, chat: &str, turn: &str) -> Result<Option<CoachTurnDependency>, ToolError> {
    let json: Option<String> = connection.query_row(
        "SELECT context FROM teaching_coach_turns WHERE learning_workspace_id=? AND chat_session_id=? AND turn_id=?",
        params![scope,chat,turn], |row|row.get(0)).optional().map_err(storage)?;
    json.map(|json|serde_json::from_str(&json).map_err(storage)).transpose()
}

// Course response facts and their scheduled analysis commit together.
pub(crate) fn ensure_in(connection: &Connection, scope: &str, input: &CoachJobInput, now: &str) -> Result<CoachJob, ToolError> {
        if input.job_ref.trim().is_empty() || input.source_ref.trim().is_empty() || input.binding.tutor_session_id.trim().is_empty() {
            return Err(conflict("教练任务需要任务、阶段和来源记录身份。"));
        }
        if let Some(old) = get_job(connection, scope, &input.job_ref)? {
            if old.input.origin != input.origin || old.input.source_ref != input.source_ref
                || old.input.binding.tutor_session_id != input.binding.tutor_session_id {
                return Err(conflict("同一教练任务身份不能指向另一个来源回合。"));
            }
            return Ok(old);
        }
        let existing: Option<String> = connection.query_row(
            "SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND session_id=? AND origin=? AND source_ref=?",
            params![scope,input.binding.tutor_session_id,input.origin.as_str(),input.source_ref],
            |row|row.get(0)).optional().map_err(storage)?;
        if let Some(existing) = existing { return serde_json::from_str(&existing).map_err(storage); }
        let job = CoachJob {
            learning_workspace_id: (!scope.is_empty()).then(||scope.to_owned()),
            input: input.clone(), status: CoachJobStatus::Pending, attempt: 0, attempts: Vec::new(),
            guidance_ref: None, guidance: None, error: None, created_at: now.into(), updated_at: now.into(),
        };
        connection.execute("INSERT INTO teaching_coach_jobs(learning_workspace_id,job_ref,session_id,origin,source_ref,status,job) VALUES(?,?,?,?,?,?,?)",
            params![scope,input.job_ref,input.binding.tutor_session_id,input.origin.as_str(),input.source_ref,
                job.status.as_str(),serde_json::to_string(&job).map_err(storage)?]).map_err(storage)?;
        Ok(job)
}

impl LearningStore {
    pub fn coach_job_for_turn(&self, turn: &str) -> Result<Option<Value>,ToolError> {
        self.connection.query_row("SELECT job_ref,status FROM teaching_coach_jobs WHERE learning_workspace_id=? AND source_ref=? AND origin IN ('after_turn','observation_backfill') ORDER BY seq DESC LIMIT 1",params![self.workspace_scope,turn],|row|Ok(serde_json::json!({"job_ref":row.get::<_,String>(0)?,"status":row.get::<_,String>(1)?,"workspace_id":self.learning_workspace_id()}))).optional().map_err(storage)
    }
    pub fn ready_coach_for_binding(&self, binding: &TeachingBinding, goal: Option<u32>) -> Result<Option<CoachJob>, ToolError> {
        // A recovered older turn can register and finish after a newer turn.
        // Prefer the original fact order, with insertion order only as fallback.
        let value:Option<String> = self.connection.query_row("SELECT jobs.job FROM teaching_coach_jobs AS jobs
            LEFT JOIN teaching_trace AS original ON original.learning_workspace_id=jobs.learning_workspace_id
                AND original.event_id='binding:' || jobs.source_ref
            WHERE jobs.learning_workspace_id=? AND jobs.session_id=? AND jobs.status='ready' AND jobs.origin!='observation_backfill'
            AND json_extract(jobs.job,'$.input.binding.source_id')=? AND json_extract(jobs.job,'$.input.binding.source_revision')=?
            AND json_extract(jobs.job,'$.input.workspace_goal_revision') IS ? ORDER BY original.seq DESC,jobs.seq DESC LIMIT 1",
            params![self.workspace_scope,binding.tutor_session_id,binding.source_id,binding.source_revision,goal], |row|row.get(0)).optional().map_err(storage)?;
        value.map(|value|serde_json::from_str(&value).map_err(storage)).transpose()
    }

    /// Actual help already delivered before this expression. The event sequence
    /// is the boundary, so later answers cannot become prior learner assistance.
    pub fn prior_chat_help_for_expression(&self, fact: &TeachingEvent) -> Result<Vec<TeachingEvent>, ToolError> {
        // Course imports already verify the exact independent attempt and retain
        // its original expression refs. The destination course's earlier lesson
        // is history, not help supplied during that attempt. Later course replies
        // do not carry these source refs and still collect their actual help.
        if fact.kind == crate::teaching::TeachingFact::LearnerAction
            && fact.payload["closed_book_result_ref"].is_string()
            && fact.payload["source_expression_refs"].is_array()
        {
            return Ok(Vec::new());
        }
        // Both the submitted answer and the accompanying reasoning were written
        // before the card displayed feedback. A usage fact can repeat either.
        if fact.payload["submission_field"].is_string() || fact.payload["action"] == "explain_back" {
            return Ok(Vec::new());
        }
        let sequence = self.teaching_event_sequence(&fact.event_id)?;
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace
            WHERE learning_workspace_id=?1 AND seq<?2
            AND json_extract(event,'$.binding.chat_session_id')=?3
            AND json_extract(event,'$.binding.source_id')=?4
            AND json_extract(event,'$.binding.source_revision')=?5
            AND (json_extract(event,'$.kind')='message_delivered'
                OR json_extract(event,'$.kind')='usage_observed' AND json_extract(event,'$.payload.usage.kind')='assistant_delivery')
            AND NOT EXISTS (SELECT 1 FROM teaching_trace r WHERE r.learning_workspace_id=?1
                AND r.event_id='response:'||?6 AND json_extract(r.event,'$.payload.submission_field') IS NOT NULL
                AND json_extract(r.event,'$.payload.response')=?7)
            ORDER BY seq DESC LIMIT 1").map_err(storage)?;
        let rows=query.query_map(params![self.workspace_scope,sequence,fact.binding.chat_session_id,
            fact.binding.source_id,fact.binding.source_revision,fact.binding.turn_id,fact.learner_expression()],|row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// Repeated delivery notifications retain the original frozen input and identity.
    pub fn ensure_coach_job(&mut self, input: &CoachJobInput, now: &str) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let job = ensure_in(&tx, &self.workspace_scope, input, now)?;
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    pub fn coach_job(&self, job_ref: &str) -> Result<Option<CoachJob>, ToolError> {
        get_job(&self.connection, &self.workspace_scope, job_ref)
    }

    pub fn coach_job_for_source(&self, session: &str, origin: CoachJobOrigin, source_ref: &str) -> Result<Option<CoachJob>, ToolError> {
        let json: Option<String> = self.connection.query_row(
            "SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND session_id=? AND origin=? AND source_ref=?",
            params![self.workspace_scope,session,origin.as_str(),source_ref], |row|row.get(0)).optional().map_err(storage)?;
        json.map(|json|serde_json::from_str(&json).map_err(storage)).transpose()
    }

    pub fn latest_coach_job(&self, session: &str, origin: CoachJobOrigin) -> Result<Option<CoachJob>, ToolError> {
        // Recovery can register an older turn after a newer task already exists.
        // The original turn binding orders after-turn guidance; task insertion
        // order remains the fallback for fixtures without persisted bindings.
        let query = match origin {
            CoachJobOrigin::AfterTurn => "SELECT jobs.job FROM teaching_coach_jobs AS jobs
                LEFT JOIN teaching_trace AS binding ON binding.learning_workspace_id=jobs.learning_workspace_id
                    AND binding.event_id='binding:' || jobs.source_ref
                WHERE jobs.learning_workspace_id=? AND jobs.session_id=? AND jobs.origin=?
                ORDER BY binding.seq DESC,jobs.seq DESC LIMIT 1",
            CoachJobOrigin::StageStart | CoachJobOrigin::ObservationBackfill => "SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND session_id=? AND origin=? ORDER BY seq DESC LIMIT 1",
        };
        let json: Option<String> = self.connection.query_row(
            query,
            params![self.workspace_scope,session,origin.as_str()], |row|row.get(0)).optional().map_err(storage)?;
        json.map(|json|serde_json::from_str(&json).map_err(storage)).transpose()
    }

    pub fn list_coach_jobs(&self, status: Option<CoachJobStatus>, limit: usize) -> Result<Vec<CoachJob>, ToolError> {
        let mut query = self.connection.prepare("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND (? IS NULL OR status=?) ORDER BY seq ASC LIMIT ?").map_err(storage)?;
        let status = status.map(CoachJobStatus::as_str);
        let rows = query.query_map(params![self.workspace_scope,status,status,limit], |row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// UI status reads avoid loading the retained model input for every job.
    pub fn understanding_jobs(&self, source:&str, revision:&str)->Result<Vec<Value>,ToolError>{
        let mut query=self.connection.prepare("SELECT job_ref,status FROM teaching_coach_jobs WHERE learning_workspace_id=? AND status IN ('pending','running','failed','interrupted') AND origin<>'stage_start' AND json_extract(job,'$.input.binding.source_id')=? AND json_extract(job,'$.input.binding.source_revision')=? ORDER BY seq DESC LIMIT 12").map_err(storage)?;
        let rows=query.query_map(params![self.workspace_scope,source,revision],|row|Ok(serde_json::json!({"job_ref":row.get::<_,String>(0)?,"status":row.get::<_,String>(1)?}))).map_err(storage)?;
        rows.map(|row|row.map_err(storage)).collect()
    }

    pub fn list_coach_jobs_all_scopes(&self, status: Option<CoachJobStatus>, limit: usize) -> Result<Vec<CoachJob>, ToolError> {
        let mut query = self.connection.prepare("SELECT job FROM teaching_coach_jobs WHERE (? IS NULL OR status=?) ORDER BY seq ASC LIMIT ?").map_err(storage)?;
        let status = status.map(CoachJobStatus::as_str);
        let rows = query.query_map(params![status,status,limit], |row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// A local reader can execute only jobs for its loaded material revision.
    /// Filter before the page limit so other books cannot hide eligible work.
    pub fn pending_coach_jobs_for_source(&self, source: &str, revision: &str, limit: usize) -> Result<Vec<CoachJob>, ToolError> {
        let mut query = self.connection.prepare("SELECT job FROM teaching_coach_jobs WHERE status='pending'
            AND json_extract(job,'$.input.binding.source_id')=?
            AND json_extract(job,'$.input.binding.source_revision')=? ORDER BY seq ASC LIMIT ?").map_err(storage)?;
        let rows = query.query_map(params![source,revision,limit], |row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// Account worker discovery, before selecting a workspace-scoped handle.
    pub fn coach_job_workspaces(&self) -> Result<Vec<Option<String>>, ToolError> {
        let mut query = self.connection.prepare("SELECT DISTINCT learning_workspace_id FROM teaching_coach_jobs WHERE status IN ('pending','running') ORDER BY learning_workspace_id").map_err(storage)?;
        let rows = query.query_map([], |row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|row.map(|scope|(!scope.is_empty()).then_some(scope)).map_err(storage)).collect()
    }

    pub fn claim_coach_job(&mut self, job_ref: &str, now: &str) -> Result<Option<CoachJob>, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if job.status != CoachJobStatus::Pending { return Ok(None); }
        job.status = CoachJobStatus::Running;
        job.attempt += 1;
        job.updated_at = now.into();
        job.error = None;
        job.attempts.push(CoachJobAttempt { number: job.attempt, started_at: now.into(), finished_at: None,
            status: CoachJobStatus::Running, error: None });
        save_job(&tx, &self.workspace_scope, &job)?;
        tx.commit().map_err(storage)?;
        Ok(Some(job))
    }

    /// The guidance fact and ready state commit together; no TeachingDesign is accepted here.
    pub fn finish_coach_job(&mut self, job_ref: &str, attempt: u32, guidance: Value, now: &str) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if job.status == CoachJobStatus::Ready && job.attempt == attempt && job.guidance.as_ref() == Some(&guidance) {
            return Ok(job);
        }
        if job.status != CoachJobStatus::Running || job.attempt != attempt { return Err(conflict("教练结果不属于当前运行尝试。")); }
        let event_id = format!("{job_ref}:guidance");
        let source_owner: Option<String> = tx.query_row(
            "SELECT session_id FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
            params![self.workspace_scope,job.input.source_ref], |row|row.get(0)).optional().map_err(storage)?;
        let causal_refs = if source_owner.as_deref() == Some(job.input.binding.tutor_session_id.as_str()) {
            vec![job.input.source_ref.clone()]
        } else { Vec::new() };
        let event = TeachingEvent { event_id: event_id.clone(), binding: job.input.binding.clone(), kind: TeachingFact::CoachGuided,
            causal_refs, payload: guidance.clone(), occurred_at: now.into() };
        tx.execute("INSERT INTO teaching_trace(learning_workspace_id,event_id,session_id,event) VALUES(?,?,?,?)",
            params![self.workspace_scope,event.event_id,event.binding.tutor_session_id,serde_json::to_string(&event).map_err(storage)?]).map_err(storage)?;
        job.guidance_ref = Some(event_id);
        job.guidance = Some(guidance);
        close_attempt(&mut job, CoachJobStatus::Ready, None, now);
        save_job(&tx, &self.workspace_scope, &job)?;
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    pub fn fail_coach_job(&mut self, job_ref: &str, attempt: u32, error: Value, now: &str) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if job.status != CoachJobStatus::Running || job.attempt != attempt { return Err(conflict("教练失败不属于当前运行尝试。")); }
        close_attempt(&mut job, CoachJobStatus::Failed, Some(error), now);
        save_job(&tx, &self.workspace_scope, &job)?;
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    pub fn retry_coach_job(&mut self, job_ref: &str, now: &str) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if !matches!(job.status, CoachJobStatus::Failed | CoachJobStatus::Interrupted) { return Err(conflict("只有失败或中断的教练任务可重试。")); }
        job.status = CoachJobStatus::Pending;
        job.updated_at = now.into();
        job.error = None;
        save_job(&tx, &self.workspace_scope, &job)?;
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    /// Only original, host-verified source reads may supplement the frozen
    /// input. All learner facts, help conditions and paid attempts stay intact.
    pub fn freeze_coach_sources(&mut self, job_ref: &str, attempt: u32, ranges: &[read_tools::EvidenceRange], bindings: Value) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if job.attempt != attempt || matches!(job.status, CoachJobStatus::Ready | CoachJobStatus::Cancelled) {
            return Err(conflict("原文读取补充不属于可处理的教练尝试。"));
        }
        job.input.context["verified_read_ranges"] = serde_json::to_value(ranges).map_err(storage)?;
        job.input.context["source_bindings"] = bindings;
        save_job(&tx, &self.workspace_scope, &job)?;
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    pub fn cancel_coach_job(&mut self, job_ref: &str, now: &str) -> Result<CoachJob, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut job = required_job(&tx, &self.workspace_scope, job_ref)?;
        if matches!(job.status, CoachJobStatus::Pending | CoachJobStatus::Running) {
            close_attempt(&mut job, CoachJobStatus::Cancelled, None, now);
            save_job(&tx, &self.workspace_scope, &job)?;
        }
        tx.commit().map_err(storage)?;
        Ok(job)
    }

    /// Run once when recovering the owner, never on ordinary per-request open.
    /// A call with an unknown outcome must not be replayed as an unstarted job.
    pub fn recover_interrupted_coach_jobs(&mut self, now: &str) -> Result<usize, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let jobs = {
            let mut query = tx.prepare("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND status='running'").map_err(storage)?;
            let rows = query.query_map([&self.workspace_scope], |row|row.get::<_,String>(0)).map_err(storage)?;
            rows.map(|row|serde_json::from_str::<CoachJob>(&row.map_err(storage)?).map_err(storage)).collect::<Result<Vec<_>,_>>()?
        };
        for mut job in jobs.iter().cloned() {
            close_attempt(&mut job, CoachJobStatus::Interrupted, Some(serde_json::json!({"reason":"owner_restarted","message":"教练调用结果未知，请显式重试。"})), now);
            save_job(&tx, &self.workspace_scope, &job)?;
        }
        tx.commit().map_err(storage)?;
        Ok(jobs.len())
    }

    pub fn ensure_coach_turn_dependency(&mut self, chat: &str, turn: &str, job_ref: Option<&str>, now: &str) -> Result<CoachTurnDependency, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        if let Some(existing) = get_turn(&tx, &self.workspace_scope, chat, turn)? { return Ok(existing); }
        if let Some(job_ref) = job_ref { required_job(&tx, &self.workspace_scope, job_ref)?; }
        let context = CoachTurnDependency { chat_session_id: chat.into(), turn_id: turn.into(), job_ref: job_ref.map(str::to_owned),
            context: None, created_at: now.into(), frozen_at: None };
        tx.execute("INSERT INTO teaching_coach_turns(learning_workspace_id,chat_session_id,turn_id,context) VALUES(?,?,?,?)",
            params![self.workspace_scope,chat,turn,serde_json::to_string(&context).map_err(storage)?]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(context)
    }

    pub fn coach_turn_dependency(&self, chat: &str, turn: &str) -> Result<Option<CoachTurnDependency>, ToolError> {
        get_turn(&self.connection, &self.workspace_scope, chat, turn)
    }

    pub fn coach_turn_context(&self, chat: &str, turn: &str) -> Result<Option<CoachTurnDependency>, ToolError> {
        self.coach_turn_dependency(chat, turn)
    }

    pub fn freeze_coach_turn_context(&mut self, chat: &str, turn: &str, consumed: Value, now: &str) -> Result<CoachTurnDependency, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let mut context = get_turn(&tx, &self.workspace_scope, chat, turn)?.ok_or_else(||conflict("需要先固定本轮教练任务依赖。"))?;
        if context.context.is_some() { return Ok(context); }
        context.context = Some(consumed);
        context.frozen_at = Some(now.into());
        tx.execute("UPDATE teaching_coach_turns SET context=? WHERE learning_workspace_id=? AND chat_session_id=? AND turn_id=?",
            params![serde_json::to_string(&context).map_err(storage)?,self.workspace_scope,chat,turn]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input(job: &str, session: &str, source: &str) -> CoachJobInput {
        CoachJobInput {
            job_ref: job.into(), origin: CoachJobOrigin::AfterTurn, source_ref: source.into(),
            workspace_goal_revision: Some(4), context: json!({"user":"原始表达","delivery":"已交付内容"}),
            binding: TeachingBinding { tutor_session_id: session.into(), session_revision: 1, control_revision: 1,
                source_id: "book".into(), source_revision: "v1".into(), map_revision: None,
                chat_session_id: "chat".into(), turn_id: source.into() },
        }
    }

    #[test]
    fn coach_jobs_keep_one_frozen_source_input_and_isolate_workspaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut first = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace-a"));
        let original = first.ensure_coach_job(&input("job-a", "stage", "turn"), "t0").unwrap();
        let mut duplicate = input("other-id", "stage", "turn");
        duplicate.context = json!({"late_state":"must not replace source snapshot"});
        assert_eq!(first.ensure_coach_job(&duplicate, "t1").unwrap(), original);
        let claimed = first.claim_coach_job("job-a", "t2").unwrap().unwrap();
        assert_eq!(claimed.attempt, 1);
        assert!(first.claim_coach_job("job-a", "t3").unwrap().is_none());
        let mut second = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace-b"));
        assert!(second.coach_job("job-a").unwrap().is_none());
        second.ensure_coach_job(&input("job-a", "stage", "turn"), "t4").unwrap();
        assert_eq!(second.list_coach_jobs(None, 20).unwrap().len(), 1);
        let all = second.list_coach_jobs_all_scopes(None, 20).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].learning_workspace_id.as_deref(), Some("workspace-a"));
        assert_eq!(all[1].learning_workspace_id.as_deref(), Some("workspace-b"));
        assert_eq!(second.coach_job_for_source("stage", CoachJobOrigin::AfterTurn, "turn").unwrap().unwrap().status, CoachJobStatus::Pending);
    }

    #[test]
    fn coach_guidance_and_ready_status_commit_together_and_retry_saving_without_regeneration() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&dir.path().join("learning.db"), false).unwrap();
        let input = input("job", "stage", "delivered");
        store.append_teaching(&TeachingEvent { event_id: "delivered".into(), binding: input.binding.clone(),
            kind: TeachingFact::MessageDelivered, causal_refs: Vec::new(), payload: json!({"text":"已展示"}), occurred_at: "t0".into() }).unwrap();
        store.ensure_coach_job(&input, "t0").unwrap();
        store.claim_coach_job("job", "t1").unwrap();
        store.connection.execute_batch("CREATE TRIGGER coach_save_failure BEFORE UPDATE ON teaching_coach_jobs WHEN NEW.status='ready' BEGIN SELECT RAISE(ABORT,'write failed'); END;").unwrap();
        let guidance = json!({"reason":"下一步等待真实回答","next_step":{"focus":"条件辨析"}});
        assert!(store.finish_coach_job("job", 1, guidance.clone(), "t2").is_err());
        assert!(store.teaching_event("job:guidance").is_err());
        assert_eq!(store.coach_job("job").unwrap().unwrap().status, CoachJobStatus::Running);
        store.connection.execute_batch("DROP TRIGGER coach_save_failure;").unwrap();
        let ready = store.finish_coach_job("job", 1, guidance.clone(), "t3").unwrap();
        assert_eq!(ready.status, CoachJobStatus::Ready);
        assert_eq!(store.finish_coach_job("job", 1, guidance, "t4").unwrap(), ready);
        assert_eq!(store.teaching_event("job:guidance").unwrap().causal_refs, vec!["delivered"]);
        assert_eq!(store.teaching_event("job:guidance").unwrap().kind, TeachingFact::CoachGuided);
        assert!(store.current_teaching_design("stage", "book", "v1").unwrap().is_none());
        assert!(store.pending_response_invitation("stage", "book", "v1").unwrap().is_none());
    }

    #[test]
    fn coach_restart_does_not_replay_unknown_calls_and_explicit_retry_tracks_attempts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        for (job, source) in [("pending", "t1"), ("running", "t2"), ("ready", "t3")] {
            store.ensure_coach_job(&input(job, "stage", source), "t0").unwrap();
        }
        store.claim_coach_job("running", "t1").unwrap();
        store.claim_coach_job("ready", "t1").unwrap();
        store.finish_coach_job("ready", 1, json!({"next":"continue"}), "t2").unwrap();
        drop(store);
        let mut store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.recover_interrupted_coach_jobs("restart").unwrap(), 1);
        assert_eq!(store.recover_interrupted_coach_jobs("restart").unwrap(), 0);
        assert_eq!(store.coach_job("pending").unwrap().unwrap().status, CoachJobStatus::Pending);
        assert_eq!(store.coach_job("ready").unwrap().unwrap().status, CoachJobStatus::Ready);
        assert!(store.claim_coach_job("running", "t3").unwrap().is_none());
        assert!(store.finish_coach_job("running", 1, json!({"late":true}), "t3").is_err());
        store.retry_coach_job("running", "t4").unwrap();
        let attempt = store.claim_coach_job("running", "t5").unwrap().unwrap();
        assert_eq!(attempt.attempt, 2);
        assert_eq!(attempt.attempts[0].status, CoachJobStatus::Interrupted);
        assert!(store.fail_coach_job("running", 1, json!({"stale":true}), "t6").is_err());
        let failed = store.fail_coach_job("running", 2, json!({"message":"provider failed"}), "t6").unwrap();
        assert_eq!(failed.attempts[1].status, CoachJobStatus::Failed);
        store.retry_coach_job("running", "t7").unwrap();
        store.cancel_coach_job("running", "t8").unwrap();
        assert!(store.claim_coach_job("running", "t9").unwrap().is_none());
    }

    #[test]
    fn coach_dependency_and_consumed_guidance_freeze_independently_of_late_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        store.ensure_coach_job(&input("job", "stage", "previous"), "t0").unwrap();
        let dependency = store.ensure_coach_turn_dependency("chat", "next", Some("job"), "t1").unwrap();
        assert!(dependency.context.is_none());
        assert_eq!(store.ensure_coach_turn_dependency("chat", "next", None, "t2").unwrap(), dependency);
        let consumed = json!({"status":"unavailable","source_turn_ref":"previous","reason":"interrupted"});
        let frozen = store.freeze_coach_turn_context("chat", "next", consumed, "t3").unwrap();
        assert_eq!(store.freeze_coach_turn_context("chat", "next", json!({"status":"ready","guidance":"late"}), "t4").unwrap(), frozen);
        let none = store.ensure_coach_turn_dependency("chat", "ordinary", None, "t5").unwrap();
        assert!(none.job_ref.is_none());
        assert_eq!(store.ensure_coach_turn_dependency("chat", "ordinary", Some("job"), "t6").unwrap(), none);
        drop(store);
        let store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.coach_turn_dependency("chat", "next").unwrap(), Some(frozen));
        assert!(store.coach_turn_dependency("chat", "never-admitted").unwrap().is_none());
    }

    #[test]
    fn coach_latest_after_turn_uses_original_binding_order_when_repair_inserts_jobs_out_of_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        for (workspace, binding_order, expected) in [
            ("workspace-a", ["old", "new"], "new"),
            ("workspace-b", ["new", "old"], "old"),
        ] {
            let mut store = LearningStore::open(&path, false).unwrap().with_workspace(Some(workspace));
            for source in binding_order {
                let job = input(source, "stage", source);
                store.append_teaching(&TeachingEvent { event_id: format!("binding:{source}"),
                    binding: job.binding, kind: TeachingFact::TurnBound, causal_refs: Vec::new(),
                    payload: json!({"status":"active"}), occurred_at: "same-time".into() }).unwrap();
            }
            // The older A turn is repaired after the newer turn's task exists.
            store.ensure_coach_job(&input("new", "stage", "new"), "same-time").unwrap();
            store.ensure_coach_job(&input("old", "stage", "old"), "same-time").unwrap();
            assert_eq!(store.latest_coach_job("stage", CoachJobOrigin::AfterTurn).unwrap().unwrap().input.job_ref, expected);
            for source in ["new", "old"] {
                let mut start = input(&format!("start-{source}"), "stage", source);
                start.origin = CoachJobOrigin::StageStart;
                store.ensure_coach_job(&start, "same-time").unwrap();
            }
            assert_eq!(store.latest_coach_job("stage", CoachJobOrigin::StageStart).unwrap().unwrap().input.job_ref, "start-old");
        }
        // Another workspace's later binding rows cannot reorder A's history.
        let store = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace-a"));
        assert_eq!(store.latest_coach_job("stage", CoachJobOrigin::AfterTurn).unwrap().unwrap().input.job_ref, "new");
    }

    #[test]
    fn coach_ready_guidance_uses_original_turn_order_after_older_job_is_recovered_late() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&dir.path().join("learning.db"), false).unwrap()
            .with_workspace(Some("workspace-a"));
        for source in ["old", "new"] {
            store.append_teaching(&TeachingEvent { event_id:format!("binding:{source}"),
                binding:input(source,"stage",source).binding, kind:TeachingFact::TurnBound,
                causal_refs:Vec::new(), payload:json!({"status":"active"}), occurred_at:"same-time".into() }).unwrap();
        }
        // Both tasks concern the same stage, source revision and goal. Recovery
        // registers and completes old last, but it must not displace new guidance.
        for source in ["new", "old"] {
            store.ensure_coach_job(&input(source,"stage",source),"same-time").unwrap();
            store.claim_coach_job(source,"same-time").unwrap().unwrap();
            store.finish_coach_job(source,1,json!({"focus":source}),"same-time").unwrap();
        }
        let current = input("current","stage","current");
        let ready = store.ready_coach_for_binding(&current.binding,current.workspace_goal_revision).unwrap().unwrap();
        assert_eq!(ready.input.job_ref,"new");
        assert_eq!(ready.guidance.unwrap()["focus"],"new");
    }
}
