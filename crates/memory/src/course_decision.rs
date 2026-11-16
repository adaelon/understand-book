//! A prepared decision task continues through real learner replies and a fresh application.
use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseDecisionTeaching {
    pub goals: Vec<CourseDecisionGoal>,
    pub experiment: CourseResourceState,
    pub transfer_question: String,
    pub transfer_solution: String,
    pub transfer_experiment: CourseResourceState,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseDecisionGoal {
    pub id: String,
    pub title: String,
    pub criterion: String,
    pub support: String,
    pub attention: String,
}
impl CourseDecisionTeaching {
    pub fn validate(&self) -> Result<(), ToolError> {
        let ids:BTreeSet<_> = self.goals.iter().map(|g|&g.id).collect();
        if self.goals.is_empty() || ids.len()!=self.goals.len() || self.goals.iter().any(|g| [&g.id,&g.title,&g.criterion,&g.support,&g.attention].iter().any(|s|s.trim().is_empty()))
            || self.transfer_question.trim().is_empty() || self.transfer_solution.trim().is_empty() { return Err(invalid("决策教学需要目标、支撑与新情境任务")); }
        self.experiment.validate()?; self.transfer_experiment.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseEvidenceStatus { Demonstrated, NeedsSupport, Unconfirmed }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseGoalEvidence {
    pub goal_id: String,
    pub status: CourseEvidenceStatus,
    pub fact_ref: Option<String>,
    pub quote: Option<String>,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseDecisionJudgment {
    pub goals: Vec<CourseGoalEvidence>,
    pub focus_goal: Option<String>,
    pub ready: bool,
}
impl CourseDecisionJudgment {
    /// An evaluator must point to real expressions, never the teacher's solution.
    pub fn validate_for(&self, teaching: &CourseDecisionTeaching, facts: &[(String, String)]) -> Result<(), ToolError> {
        let ids: BTreeSet<_> = self.goals.iter().map(|g| &g.goal_id).collect();
        if ids.len() != self.goals.len() || ids != teaching.goals.iter().map(|g| &g.id).collect() {
            return Err(invalid("本题理解判断须对应已准备的目标"));
        }
        for g in &self.goals {
            if g.reason.trim().is_empty() { return Err(invalid("理解判断缺少具体依据")); }
            if g.status == CourseEvidenceStatus::Demonstrated && !facts.iter().any(|(id, text)|
                g.fact_ref.as_ref() == Some(id) && g.quote.as_ref().is_some_and(|q| !q.trim().is_empty() && text.contains(q))) {
                return Err(invalid("理解依据必须引用学习者的真实表达"));
            }
        }
        if self.ready {
            if self.goals.iter().any(|g| g.status != CourseEvidenceStatus::Demonstrated) || self.focus_goal.is_some() {
                return Err(invalid("本题仍有待确认的理解目标"));
            }
        } else if !self.focus_goal.as_ref().is_some_and(|id| teaching.goals.iter().any(|g| &g.id == id)) {
            return Err(invalid("请选择本轮需要帮助的目标"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseDecisionPhase { Decision, Transfer }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseReplyIntent { Answer, Question, Explain }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseDecisionState {
    pub phase: CourseDecisionPhase,
    pub latest_job: Option<String>,
    pub intent: CourseReplyIntent,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseDecisionView {
    pub phase: CourseDecisionPhase,
    pub latest_job: Option<String>,
    pub question: String,
    pub can_continue: bool,
    pub focus_title: Option<String>,
    pub support: Option<String>,
    pub attention: Option<String>,
}

pub(super) fn decision_facts(run: &CourseRun) -> Vec<(String, String)> {
    let phase = run.decision.as_ref().map(|d| d.phase);
    run.expressions.iter().filter(|e| match phase {
        Some(CourseDecisionPhase::Decision) => e.activity == "task" || e.activity == "decision",
        Some(CourseDecisionPhase::Transfer) => e.activity == "transfer",
        None => false,
    }).map(|e| (e.fact_ref.clone(), e.response.clone())).collect()
}
pub(super) fn decision_view(run: &CourseRun) -> Option<CourseDecisionView> {
    let state = run.decision.as_ref()?;
    let teaching = run.lesson.decision_teaching.as_ref()?;
    Some(CourseDecisionView { phase:state.phase, latest_job:state.latest_job.clone(),
        question:if state.phase == CourseDecisionPhase::Transfer { teaching.transfer_question.clone() } else { run.lesson.task.question.clone() },
        can_continue:false, focus_title:None, support:None, attention:None })
}
pub(super) fn decision_guidance(db: &Connection, scope: &str, run: &CourseRun) -> Result<Option<(CourseFeedback, Value)>, ToolError> {
    let Some(state) = &run.decision else { return Ok(None) };
    let Some(id) = &state.latest_job else { return Ok(None) };
    let raw:Option<String> = db.query_row("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND job_ref=?",params![scope,id],|r|r.get(0)).optional().map_err(storage)?;
    let Some(raw) = raw else { return Ok(None) };
    let job:crate::tutor_coach::CoachJob = serde_json::from_str(&raw).map_err(storage)?;
    if job.status != crate::tutor_coach::CoachJobStatus::Ready { return Ok(None) }
    let Some(guidance) = job.guidance else { return Ok(None) };
    let feedback:CourseFeedback = serde_json::from_value(guidance["course_feedback"].clone()).map_err(storage)?;
    let Some(judgment) = &feedback.decision else { return Ok(None) };
    judgment.validate_for(run.lesson.decision_teaching.as_ref().unwrap(), &decision_facts(run))?;
    Ok(Some((feedback, guidance["next_step"].clone())))
}
pub(super) fn decision_ready(db: &Connection, scope: &str, run: &CourseRun) -> Result<bool, ToolError> {
    let Some(state) = &run.decision else { return Ok(false) };
    if state.intent != CourseReplyIntent::Answer || !state.latest_job.as_ref().is_some_and(|id|run.displayed_feedback.contains(id)) { return Ok(false) }
    Ok(decision_guidance(db,scope,run)?.is_some_and(|(f,_)| f.decision.is_some_and(|d|d.ready)))
}
