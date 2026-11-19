//! Prepared inquiry lessons. Public content is released by stage; private runs
//! retain their exact lesson, responses and delivered support in LearningStore.
use crate::{
    assessment::{
        assess_closed, AssessmentContract, AssessmentStatus, FeedbackPolicy, ResponseAssessment, ScoringRule,
    },
    learning::{storage, LearningStore},
    teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact},
};
use base_schema::PublishedBookRef;
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "course_tests.rs"]
mod tests;
#[path = "course_decision.rs"]
mod decision;
pub use decision::*;
#[path = "course_math.rs"]
mod math;
pub use math::*;
use decision::{decision_facts, decision_view, decision_guidance, decision_ready};
#[path = "course_closed_book.rs"]
mod closed_book;
use closed_book::read_closed_book_result;

pub(crate) const SCHEMA: &str = "CREATE TABLE course_runs (
 learning_workspace_id TEXT NOT NULL, run_id TEXT NOT NULL, release_id TEXT NOT NULL,
 release_version INTEGER NOT NULL, lesson_id TEXT NOT NULL, state TEXT NOT NULL,
 PRIMARY KEY(learning_workspace_id,run_id));
 CREATE TABLE course_operations (learning_workspace_id TEXT NOT NULL, operation_id TEXT NOT NULL,
 request TEXT NOT NULL, run_id TEXT NOT NULL, result TEXT NOT NULL,
 PRIMARY KEY(learning_workspace_id,operation_id));";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseSource {
    pub publication: PublishedBookRef,
    pub source_revision: String,
    pub start_lid: String,
    pub end_lid: String,
    pub label: String,
    pub structure_refs: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseOption {
    pub id: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CoursePrompt {
    pub question: String,
    pub options: Vec<CourseOption>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum CourseExperiment {
    Inquiry {
        activity: CourseInquiryKind,
        instruction: String,
        material: String,
        response_prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        visualization: Option<CourseMathState>,
    },
    ResourceBounds {
        instruction: String,
        #[serde(deserialize_with = "decimal")]
        work_gflops: f64,
        #[serde(deserialize_with = "decimal")]
        read_gb: f64,
        #[serde(deserialize_with = "decimal")]
        compute_tflops: f64,
        #[serde(deserialize_with = "decimal")]
        bandwidth_tb_s: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        batch_size: Option<u32>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseInquiryKind { Reading, Reasoning, Practice }
impl CourseExperiment {
    fn batch_size(&self) -> Option<u32> { match self { Self::ResourceBounds { batch_size, .. } => *batch_size, _ => None } }
    fn presets(&self) -> Vec<CoursePreset> {
        if matches!(self, Self::Inquiry { .. }) { vec![] }
        else if self.batch_size().is_some() { vec![CoursePreset::BatchOne,CoursePreset::BatchFour,CoursePreset::BatchEight] }
        else { vec![CoursePreset::Baseline,CoursePreset::DoubleCompute,CoursePreset::DoubleBandwidth] }
    }
}
// serde's tagged-enum buffer represents arbitrary-precision JSON decimals as
// a map. Decode through Value, as the existing media/board contracts do.
fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    serde_json::Value::deserialize(d)?
        .as_f64()
        .ok_or_else(|| serde::de::Error::custom("expected a finite decimal"))
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseLessonSummary {
    pub id: String,
    pub unit_id: String,
    pub question: String,
    pub objective: String,
    pub prerequisites: String,
    pub estimated_minutes: u32,
    pub duration_basis: String,
    pub source: CourseSource,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseLesson {
    pub summary: CourseLessonSummary,
    pub introduction: String,
    pub prediction: CoursePrompt,
    pub experiment: CourseExperiment,
    pub explanation: String,
    pub task: CoursePrompt,
    pub hint: String,
    pub assessment: AssessmentContract,
    pub solution: String,
    #[serde(default)]
    pub concepts: Vec<CourseConcept>,
    #[serde(default)]
    pub reflection_prompt: Option<String>,
    #[serde(default)]
    pub lab: Option<CourseResourceState>,
    #[serde(default)]
    pub reading: Option<CourseReadingGuide>,
    #[serde(default)]
    pub round_title: String,
    #[serde(default)]
    pub follow_up_rounds: Vec<CourseRound>,
    #[serde(default)]
    pub decision_teaching: Option<CourseDecisionTeaching>,
}

/// Further investigations of the same main question, before the final task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseRound {
    pub title: String,
    pub prediction: CoursePrompt,
    pub experiment: CourseExperiment,
    pub explanation: String,
    pub reflection_prompt: String,
    #[serde(default)]
    pub lab: Option<CourseResourceState>,
    pub reading: CourseReadingGuide,
}

impl CourseLesson {
    fn round_titles(&self) -> Vec<String> {
        std::iter::once(if self.round_title.is_empty() { self.summary.question.clone() } else { self.round_title.clone() })
            .chain(self.follow_up_rounds.iter().map(|r| r.title.clone())).collect()
    }
    /// The original lesson is the first investigation. Existing frozen runs
    /// retain that investigation; subsequent rounds are authored explicitly.
    pub(crate) fn round_content(&self, index: usize) -> Self {
        let mut content = self.clone();
        if index > 0 {
            let round = &self.follow_up_rounds[index - 1];
            content.prediction = round.prediction.clone();
            content.experiment = round.experiment.clone();
            content.explanation = round.explanation.clone();
            content.reflection_prompt = Some(round.reflection_prompt.clone());
            content.lab = round.lab.clone();
            content.reading = Some(round.reading.clone());
            content.round_title = round.title.clone();
            content.concepts.clear();
        }
        content.follow_up_rounds.clear();
        content
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseRoundRecord {
    #[serde(default)]
    pub math_exploration: Option<CourseMathState>,
    pub title: String,
    pub prediction: CoursePrompt,
    pub response: CourseResponse,
    pub exploration: Option<CourseResourceState>,
    pub reflection: Option<String>,
    pub explanation: String,
    #[serde(default)]
    pub activity_response: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseReadingGuide { pub before_experiment: String, pub after_experiment: String }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseConcept { pub title: String, pub explanation: String, pub relation: String }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseResourceState {
    #[serde(deserialize_with = "decimal")]
    pub work_gflops: f64,
    #[serde(deserialize_with = "decimal")]
    pub read_gb: f64,
    #[serde(deserialize_with = "decimal")]
    pub compute_tflops: f64,
    #[serde(deserialize_with = "decimal")]
    pub bandwidth_tb_s: f64,
    #[serde(deserialize_with = "decimal")]
    pub target_ms: f64,
    /// Work and read quantities remain totals for the batch. This records its output count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub batch_size: Option<u32>,
}
impl CourseResourceState {
    pub fn validate(&self) -> Result<(), ToolError> {
        if [self.work_gflops, self.read_gb, self.compute_tflops, self.bandwidth_tb_s, self.target_ms]
            .iter().any(|n| !n.is_finite() || *n <= 0.0) || self.batch_size.is_some_and(|b| b == 0 || b > 256) {
            return Err(invalid("实验参数必须是大于零的有限数值"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseFeedback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub course_reuse: Option<CourseReuseJudgment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub task_assessment: Option<Vec<CourseGoalEvidence>>,
    pub response_quote: String,
    pub acknowledgement: String,
    pub guidance: String,
    pub follow_up: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub decision: Option<CourseDecisionJudgment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub support_branch: Option<CourseSupportBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub question_guide: Option<CourseQuestionGuide>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseReuseJudgment {
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseSupportBranch {
    /// 当前求助所需的一个关系，保留原问题的条件。
    pub focus: String,
    /// 一段短解释，与原文和当前题设一致，不把下界当成实际总时间。
    pub explanation: String,
    /// 一个自洽的小例子；名称、条件和数值与 explanation 一致。
    pub example: String,
    /// 接回用户刚提出的疑问或当前原题，不能跳到课件中的后续题目。
    pub return_prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseQuestionGuide {
    /// 一句可以直接提交的完整问题，保留原疑问；不能包含“把这句话改成”等编辑指令或介绍语。
    pub suggested_question: String,
    /// 说明这个问法补充了什么条件或比较对象，不猜测用户已经采纳了什么观点。
    pub why: String,
    /// 当前材料或界面支持的一次观察、比较或查证，也可以明确建议在纸上画图。
    pub try_next: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseSupportVisit {
    pub job_ref: String,
    pub original_response: String,
    pub branch: CourseSupportBranch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseExpression {
    pub fact_ref: String, pub job_ref: String, pub activity: String, pub response: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub closed_book_result_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intent: Option<CourseReplyIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub round_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub round_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseUnderstanding {
    pub evidence_ref: String, pub interpretation: String,
    pub demonstrated: Vec<String>, pub difficulties: Vec<String>, pub unconfirmed: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub learner_summary: Option<crate::understanding::LearnerUnderstandingSummary>,
    #[serde(default)]
    #[ts(optional)]
    pub assistance: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseReview {
    pub expression: CourseExpression, pub status: String, pub displayed: bool,
    pub feedback: Option<CourseFeedback>, pub understanding: Vec<CourseUnderstanding>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseUnit {
    pub id: String,
    pub title: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseRoute {
    pub id: String,
    pub title: String,
    pub question: String,
    pub structure_ref: String,
    pub lesson_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InquiryCourse {
    pub language: String,
    pub units: Vec<CourseUnit>,
    pub routes: Vec<CourseRoute>,
    pub lessons: Vec<CourseLesson>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseOverview {
    pub language: String,
    pub units: Vec<CourseUnit>,
    pub routes: Vec<CourseRoute>,
    pub lessons: Vec<CourseLessonSummary>,
}
impl InquiryCourse {
    pub fn overview(&self) -> CourseOverview {
        CourseOverview {
            language: self.language.clone(),
            units: self.units.clone(),
            routes: self.routes.clone(),
            lessons: self.lessons.iter().map(|l| l.summary.clone()).collect(),
        }
    }
    pub fn validate(&self) -> Result<(), ToolError> {
        let ids: BTreeSet<_> = self.lessons.iter().map(|l| &l.summary.id).collect();
        let units: BTreeSet<_> = self.units.iter().map(|u| &u.id).collect();
        if self.language.trim().is_empty()
            || self.lessons.is_empty()
            || ids.len() != self.lessons.len()
            || units.len() != self.units.len()
            || self
                .units
                .iter()
                .any(|u| u.id.is_empty() || u.title.trim().is_empty())
            || self
                .routes
                .iter()
                .map(|r| &r.id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.routes.len()
            || self.routes.iter().any(|r| {
                r.id.is_empty()
                    || r.title.trim().is_empty()
                    || r.question.trim().is_empty()
                    || r.structure_ref.is_empty()
                    || r.lesson_ids.is_empty()
                    || r.lesson_ids.iter().any(|id| !ids.contains(id))
                    || r.lesson_ids.iter().collect::<BTreeSet<_>>().len() != r.lesson_ids.len()
            })
        {
            return Err(invalid("课程需要明确单元、唯一的课及有效路线"));
        }
        for lesson in &self.lessons {
            if let Some(teaching)=&lesson.decision_teaching { teaching.validate()?; }
            if let Some(lab) = &lesson.lab { lab.validate()?; }
            if lesson.reading.as_ref().is_some_and(|r| r.before_experiment.trim().is_empty() || r.after_experiment.trim().is_empty()) {
                return Err(invalid("原文阅读活动需要明确的问题"));
            }
            if lesson.concepts.iter().any(|c| [&c.title,&c.explanation,&c.relation].iter().any(|s| s.trim().is_empty()))
                || lesson.reflection_prompt.as_ref().is_some_and(|s| s.trim().is_empty()) {
                return Err(invalid("概念讲解与自我解释需要完整内容"));
            }
            let s = &lesson.summary;
            if !units.contains(&s.unit_id)
                || !(5..=30).contains(&s.estimated_minutes)
                || [
                    &s.id,
                    &s.question,
                    &s.objective,
                    &s.prerequisites,
                    &s.duration_basis,
                    &s.source.source_revision,
                    &s.source.start_lid,
                    &s.source.end_lid,
                    &s.source.label,
                    &lesson.introduction,
                    &lesson.explanation,
                    &lesson.hint,
                    &lesson.solution,
                ]
                .iter()
                .any(|s| s.trim().is_empty())
                || s.source.structure_refs.is_empty()
                || s.source.structure_refs.iter().any(|r| r.trim().is_empty())
            {
                return Err(invalid(
                    "每课需要来源、学习目标、必要支持和不超过 30 分钟的时长依据",
                ));
            }
            for prompt in [&lesson.prediction, &lesson.task] {
                if prompt.question.trim().is_empty()
                    || prompt.options.len() == 1
                    || prompt
                        .options
                        .iter()
                        .any(|o| o.id.trim().is_empty() || o.text.trim().is_empty())
                    || prompt
                        .options
                        .iter()
                        .map(|o| &o.id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != prompt.options.len()
                {
                    return Err(invalid("题面或选项缺失、重复"));
                }
            }
            lesson.assessment.validate()?;
            let task_matches = match &lesson.assessment.rule {
                ScoringRule::ChoiceSet { choices, correct } => choices.iter().collect::<BTreeSet<_>>()
                    == lesson.task.options.iter().map(|o| &o.id).collect() && correct.len() == 1,
                ScoringRule::Open { .. } => lesson.task.options.is_empty() && lesson.decision_teaching.is_none(),
            };
            if !task_matches || !lesson.assessment.source_sufficient
                || lesson.assessment.feedback != FeedbackPolicy::AfterSubmit
                || !lesson.assessment.source_ref_ids.is_empty()
            {
                return Err(invalid(
                    "课程题面、判断依据和来源必须一致，反馈在提交后交付",
                ));
            }
            match &lesson.experiment {
                CourseExperiment::Inquiry { instruction, material, response_prompt, visualization, .. } => {
                    if let Some(state) = visualization { state.validate()?; }
                    if [instruction, material, response_prompt].iter().any(|s| s.trim().is_empty())
                        || lesson.lab.is_some() || lesson.decision_teaching.is_some() {
                        return Err(invalid("探究活动需要材料、操作说明和表达任务；资源参数仅用于资源实验"));
                    }
                }
                CourseExperiment::ResourceBounds {
                    instruction,
                    work_gflops,
                    read_gb,
                    compute_tflops,
                    bandwidth_tb_s,
                    batch_size,
                } => {
                    if instruction.trim().is_empty() || batch_size.is_some_and(|b| b == 0 || b > 256)
                        || [work_gflops, read_gb, compute_tflops, bandwidth_tb_s]
                            .iter()
                            .any(|v| !v.is_finite() || **v <= 0.0)
                        || !(*work_gflops / *compute_tflops).is_finite()
                        || !(*read_gb / *bandwidth_tb_s).is_finite()
                    {
                        return Err(invalid("资源实验需要有限的正参数及操作说明"));
                    }
                }
            }
            if !lesson.follow_up_rounds.is_empty() {
                if lesson.round_title.trim().is_empty() || lesson.reflection_prompt.is_none() {
                    return Err(invalid("多轮探究需要首轮标题和解释任务"));
                }
                for (index, round) in lesson.follow_up_rounds.iter().enumerate() {
                    if round.title.trim().is_empty() || round.reflection_prompt.trim().is_empty() {
                        return Err(invalid("每轮探究需要标题和解释任务"));
                    }
                    // Apply the same prompt, experiment, source and content
                    // rules to every authored round, without recursive rounds.
                    InquiryCourse { language:self.language.clone(), units:self.units.clone(), routes:vec![],
                        lessons:vec![lesson.round_content(index + 1)] }.validate()?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseStage {
    Introduction,
    Prediction,
    Experiment,
    Explanation,
    Task,
    Feedback,
    Complete,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CourseStatus {
    Active,
    Paused,
    Completed,
    Skipped,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CoursePreset {
    Baseline,
    DoubleCompute,
    DoubleBandwidth,
    BatchOne,
    BatchFour,
    BatchEight,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseResponse {
    pub choice: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum CourseAction {
    Start { lesson_id: String },
    Advance,
    Predict { response: CourseResponse },
    Observe { preset: CoursePreset },
    Explore { state: CourseResourceState },
    Investigate { response: String, #[serde(default)] visualization: Option<CourseMathState> },
    Reflect { response: String },
    UseClosedBook { result_ref: String },
    FeedbackReply { job_ref: String, response: String, intent: CourseReplyIntent },
    AskQuestion { response: String },
    ResolveQuestion { fact_ref: String, resolved: bool },
    OpenSupport { job_ref: String },
    ReturnFromSupport,
    DecisionReply { job_ref: Option<String>, response: String, intent: CourseReplyIntent },
    ReadFeedback { job_ref: String },
    RetryFeedback { job_ref: String },
    Answer { response: CourseResponse },
    Hint,
    Source,
    Pause,
    Resume,
    Skip,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct CourseMutation {
    pub operation_id: String,
    pub run_id: Option<String>,
    pub expected_revision: u32,
    pub release_version: u32,
    pub action: CourseAction,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseProgress {
    pub run_id: String,
    pub release_id: String,
    pub release_version: u32,
    pub lesson_id: String,
    pub revision: u32,
    pub goal_revision: u32,
    pub stage: CourseStage,
    pub status: CourseStatus,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct CourseLessonView {
    #[serde(default)]
    pub math_exploration: Option<CourseMathState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub support_visit: Option<CourseSupportVisit>,
    #[serde(default)]
    pub resolved_questions: Vec<String>,
    pub progress: CourseProgress,
    pub summary: CourseLessonSummary,
    pub introduction: String,
    pub prediction: CoursePrompt,
    pub prediction_response: Option<CourseResponse>,
    pub experiment: Option<CourseExperiment>,
    pub preset: Option<CoursePreset>,
    pub observed: Vec<CoursePreset>,
    pub explanation: Option<String>,
    pub task: Option<CoursePrompt>,
    pub answer: Option<CourseResponse>,
    pub hint: Option<String>,
    pub solution: Option<String>,
    pub assessment_status: Option<String>,
    pub support_delivered: Vec<String>,
    pub source_excerpt: Option<String>,
    #[serde(default)]
    pub concepts: Vec<CourseConcept>,
    #[serde(default)]
    pub reflection_prompt: Option<String>,
    #[serde(default)]
    pub lab: Option<CourseResourceState>,
    #[serde(default)]
    pub exploration: Option<CourseResourceState>,
    #[serde(default)]
    pub expressions: Vec<CourseExpression>,
    #[serde(default)]
    pub reviews: Vec<CourseReview>,
    #[serde(default)]
    pub reading: Option<CourseReadingGuide>,
    #[serde(default)]
    pub round_index: u32,
    #[serde(default)]
    pub round_titles: Vec<String>,
    #[serde(default)]
    pub round_history: Vec<CourseRoundRecord>,
    #[serde(default)]
    pub decision: Option<CourseDecisionView>,
    #[serde(default)]
    pub activity_response: Option<String>,
    #[serde(default)]
    pub inquiry_kind: Option<CourseInquiryKind>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseRun {
    #[serde(default)]
    pub math_exploration: Option<CourseMathState>,
    #[serde(default)]
    pub support_visit: Option<CourseSupportVisit>,
    #[serde(default)]
    pub resolved_questions: Vec<String>,
    pub progress: CourseProgress,
    pub lesson: CourseLesson,
    pub prediction: Option<CourseResponse>,
    pub answer: Option<CourseResponse>,
    pub preset: Option<CoursePreset>,
    pub observed: Vec<CoursePreset>,
    pub source_excerpt: Option<String>,
    pub support_refs: Vec<String>,
    pub hint: bool,
    pub assessment: Option<ResponseAssessment>,
    #[serde(default)]
    pub exploration: Option<CourseResourceState>,
    #[serde(default)]
    pub expressions: Vec<CourseExpression>,
    #[serde(default)]
    pub displayed_feedback: Vec<String>,
    #[serde(default)]
    pub round_index: u32,
    #[serde(default)]
    pub round_history: Vec<CourseRoundRecord>,
    #[serde(default)]
    pub reflection: Option<String>,
    #[serde(default)]
    pub decision: Option<CourseDecisionState>,
    #[serde(default)]
    pub activity_response: Option<String>,
}
impl CourseRun {
    pub fn view(&self) -> CourseLessonView {
        let stage = self.progress.stage;
        let content = self.lesson.round_content(self.round_index as usize);
        let after_prediction = self.prediction.is_some();
        let explanation = matches!(
            stage,
            CourseStage::Explanation
                | CourseStage::Task
                | CourseStage::Feedback
                | CourseStage::Complete
        );
        let task = matches!(
            stage,
            CourseStage::Task | CourseStage::Feedback | CourseStage::Complete
        );
        let decision_base = self.decision.as_ref().and_then(|state| self.lesson.decision_teaching.as_ref().map(|teaching|
            if state.phase == CourseDecisionPhase::Decision { &teaching.experiment } else { &teaching.transfer_experiment }));
        CourseLessonView {
            math_exploration: self.math_exploration.clone(),
            support_visit: self.support_visit.clone(),
            resolved_questions: self.resolved_questions.clone(),
            progress: self.progress.clone(),
            summary: self.lesson.summary.clone(),
            introduction: self.lesson.introduction.clone(),
            prediction: content.prediction.clone(),
            prediction_response: self.prediction.clone(),
            experiment: decision_base.map(|base| CourseExperiment::ResourceBounds {instruction:"对照当前问题，比较计算与读取的时间。".into(),work_gflops:base.work_gflops,read_gb:base.read_gb,compute_tflops:base.compute_tflops,bandwidth_tb_s:base.bandwidth_tb_s,batch_size:base.batch_size}).or_else(||after_prediction.then(|| content.experiment.clone())),
            preset: self.preset,
            observed: self.observed.clone(),
            explanation: explanation.then(|| content.explanation.clone()),
            task: task.then(|| self.lesson.task.clone()),
            answer: self.answer.clone(),
            hint: self.hint.then(|| self.lesson.hint.clone()),
            solution: self
                .assessment
                .as_ref()
                .filter(|_| self.decision.is_none() || self.progress.stage == CourseStage::Complete)
                .map(|_| self.lesson.solution.clone()),
            assessment_status: self.assessment.as_ref().map(|a| {
                serde_json::to_value(&a.status)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            }),
            support_delivered: self.support_refs.clone(),
            source_excerpt: self.source_excerpt.clone(),
            concepts: self.lesson.concepts.clone(),
            reading: content.reading.clone(),
            reflection_prompt: explanation.then(|| content.reflection_prompt.clone()).flatten(),
            lab: decision_base.cloned().or_else(||after_prediction.then(|| content.lab.clone()).flatten()),
            exploration: self.exploration.clone(),
            expressions: self.expressions.clone(),
            reviews: vec![],
            round_index: self.round_index,
            round_titles: self.lesson.round_titles(),
            round_history: self.round_history.clone(),
            decision: decision_view(self),
            activity_response: self.activity_response.clone(),
            inquiry_kind: match &content.experiment { CourseExperiment::Inquiry { activity, .. } => Some(activity.clone()), _ => None },
        }
    }
}

fn read_run(db: &Connection, scope: &str, id: &str) -> Result<CourseRun, ToolError> {
    let raw: Option<String> = db
        .query_row(
            "SELECT state FROM course_runs WHERE learning_workspace_id=? AND run_id=?",
            params![scope, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    serde_json::from_str(&raw.ok_or_else(|| invalid("此课程记录不存在"))?).map_err(storage)
}
fn receipt(
    db: &Connection,
    scope: &str,
    request: &CourseMutation,
) -> Result<Option<CourseLessonView>, ToolError> {
    let old:Option<(String,String)>=db.query_row("SELECT request,result FROM course_operations WHERE learning_workspace_id=? AND operation_id=?",params![scope,request.operation_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    if let Some((raw, result)) = old {
        if serde_json::from_str::<CourseMutation>(&raw).map_err(storage)? != *request {
            return Err(invalid("同一课程操作不能改写内容"));
        }
        return serde_json::from_str(&result).map(Some).map_err(storage);
    }
    Ok(None)
}
fn conflict() -> ToolError {
    ToolError {
        error_code: "COURSE_REVISION_CONFLICT".into(),
        category: "conflict".into(),
        message: "课程进度已变化，请重新读取后继续".into(),
    }
}

fn enqueue_response(db: &Connection, scope: &str, run: &mut CourseRun, event: &TeachingEvent, now: &str,
    prior_understanding: &[crate::understanding::UnderstandingSummary]) -> Result<(), ToolError> {
    use crate::tutor_coach::{CoachJobInput, CoachJobOrigin};
    let refs:Vec<String> = serde_json::from_value(event.payload["support_delivery_refs"].clone()).map_err(storage)?;
    let mut help = Vec::new();
    for id in &refs {
        let raw:String = db.query_row("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",params![scope,id],|r|r.get(0)).map_err(storage)?;
        let saved:TeachingEvent = serde_json::from_str(&raw).map_err(storage)?;
        help.push(json!({"event_id":saved.event_id,"binding":saved.binding,"kind":saved.kind,"text":saved.payload.get("text").or_else(||saved.payload.get("answer")),"occurred_at":saved.occurred_at}));
    }
    let response_round=event.payload["round_index"].as_u64().map(|index|index as u32).unwrap_or(run.round_index);
    let expression = CourseExpression { fact_ref:event.event_id.clone(),job_ref:format!("course-review:{}",event.event_id),
        activity:event.payload["course_activity"].as_str().unwrap_or("reflection").into(),response:event.learner_expression().unwrap().into(),
        closed_book_result_ref:event.payload["closed_book_result_ref"].as_str().map(str::to_owned),
        intent:serde_json::from_value(event.payload["intent"].clone()).map_err(storage)?,
        reply_to:event.payload["responds_to"].as_str().map(str::to_owned),
        round_index:Some(response_round),round_title:Some(run.lesson.round_titles()[response_round as usize].clone()) };
    let teaching = run.decision.as_ref().filter(|_| matches!(expression.activity.as_str(), "task" | "decision" | "transfer")).and(run.lesson.decision_teaching.as_ref());
    let mut evidence_facts = decision_facts(run);
    evidence_facts.push((expression.fact_ref.clone(), expression.response.clone()));
    if teaching.is_some() { run.decision.as_mut().unwrap().latest_job = Some(expression.job_ref.clone()); }
    let source = &run.lesson.summary.source;
    let task_criteria = if matches!(expression.activity.as_str(), "task" | "revision")
        && expression.intent.as_ref().is_none_or(|intent| *intent == CourseReplyIntent::Answer)
        && matches!(&run.lesson.experiment, CourseExperiment::Inquiry { visualization: Some(_), .. }) {
        match &run.lesson.assessment.rule { ScoringRule::Open { items } => Some(items), _ => None }
    } else { None };
    let teaching_context = json!({"status":"course","binding":event.binding,"publication":source.publication,
        "course_run":run.progress.run_id,"course_progress":run.progress,"user_intent":run.lesson.summary.objective,
        "learner_context":{"current_understanding":prior_understanding,"unknown_when_absent":true}});
    for (id,payload) in [(format!("assets:{}",event.binding.turn_id),json!({})),
        (format!("binding:{}",event.binding.turn_id),teaching_context.clone())] {
        crate::teaching::append_in(db,scope,&TeachingEvent {event_id:id,binding:event.binding.clone(),kind:TeachingFact::TurnBound,
            causal_refs:vec![],payload,occurred_at:now.into()})?;
    }
    let mut q = db.prepare("SELECT target FROM understanding_focus WHERE learning_workspace_id=? AND source_id=? AND source_revision=? ORDER BY rowid DESC LIMIT 24").map_err(storage)?;
    let known_focuses = q.query_map(params![scope,event.binding.source_id,event.binding.source_revision],|r|r.get::<_,String>(0)).map_err(storage)?
        .map(|r| serde_json::from_str::<Value>(&r.map_err(storage)?).map_err(storage)).collect::<Result<Vec<_>,_>>()?;
    crate::tutor_coach::ensure_in(db,scope,&CoachJobInput {
        job_ref:expression.job_ref.clone(),origin:CoachJobOrigin::ObservationBackfill,binding:event.binding.clone(),
        source_ref:event.event_id.clone(),workspace_goal_revision:Some(run.progress.goal_revision),
        context:json!({"trigger":"course_response","observation_only":false,
            "course_teaching":teaching.map(|contract|json!({"contract":contract,"phase":run.decision.as_ref().unwrap().phase,"intent":run.decision.as_ref().unwrap().intent,"evidence_facts":evidence_facts})),"publication":source.publication,
            "source_bindings":[],"known_focuses":known_focuses,"teaching_context":teaching_context,
            "course_response":{"fact_ref":expression.fact_ref,"activity":expression.activity,"response":expression.response,"choice":event.payload["choice"],
                "intent":expression.intent,"reply_to":event.payload["reply_to"],"task_criteria":task_criteria,
                "closed_book_result":event.payload["closed_book_result"],
                "selected_option":event.payload["selected_option"],"question":event.payload["question"],
                "lesson":run.lesson.round_content(response_round as usize),"round_index":response_round,
                "round_history":run.round_history,"math_exploration":run.math_exploration,"exploration":event.payload.get("exploration").cloned().unwrap_or_else(||json!(run.exploration)),"earlier_expressions":run.expressions},
            "observation_facts":[{"fact_ref":event.event_id,"fact_refs":[event.event_id],"user_expression":expression.response,
                "binding":event.binding,"occurred_at":event.occurred_at,"assistance_refs":refs,"prior_help":help,
                "already_interpreted":false,"help_before_expression":if refs.is_empty(){"no_recorded_prior_help"}else{"prior_help_recorded"}}]})
    },now)?;
    run.expressions.push(expression);
    Ok(())
}

fn learner_feedback(value: &Value, intent: Option<CourseReplyIntent>) -> Value {
    let mut feedback=value.clone();
    // The original question is already shown verbatim. Start with help rather
    // than a second model interpretation of what the learner might believe.
    if intent==Some(CourseReplyIntent::Question) {
        if let Some(fields)=feedback.as_object_mut() { fields.insert("acknowledgement".into(),json!("")); }
    }
    feedback
}

// Freeze the question that was actually shown, together with its teaching purpose.
// Optional support branches are not delivered help until the learner opens them.
fn feedback_reply_context(db: &Connection, scope: &str, job_ref: &str, previous: Option<&CourseExpression>) -> Result<Value, ToolError> {
    let raw:String=db.query_row("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND job_ref=?",params![scope,job_ref],|r|r.get(0)).map_err(storage)?;
    let job:crate::tutor_coach::CoachJob=serde_json::from_str(&raw).map_err(storage)?;
    let mut feedback=job.guidance.as_ref().map(|g|learner_feedback(&g["course_feedback"],previous.and_then(|e|e.intent)));
    if let Some(Value::Object(fields))=&mut feedback { fields.remove("support_branch"); }
    Ok(json!({"job_ref":job_ref,"expression":previous,
        "question":feedback.as_ref().and_then(|f|f.get("follow_up")),"feedback":feedback,
        "next_step":job.guidance.as_ref().and_then(|g|g.get("next_step"))}))
}

impl LearningStore {
    pub fn course_view(&self, id: &str) -> Result<CourseLessonView, ToolError> {
        let run = self.course_run(id)?;
        let mut view = run.view();
        for expression in &run.expressions {
            let Some(job) = self.coach_job(&expression.job_ref)? else { continue; };
            let displayed = run.displayed_feedback.contains(&expression.job_ref);
            let guidance = job.guidance.as_ref();
            let feedback = guidance.and_then(|g| g.get("course_feedback"))
                .filter(|g| !g.is_null()).map(|g| serde_json::from_value(learner_feedback(g,expression.intent)).map_err(storage)).transpose()?;
            let understanding = self.teaching_expression_interpretations(&expression.fact_ref)?.into_iter()
                .map(|e| CourseUnderstanding { evidence_ref:e.evidence_id, interpretation:e.interpretation,
                    learner_summary:e.understanding.learner_summary,
                    assistance:Some(if e.assistance_refs.is_empty() && e.support_delivery_refs.is_empty(){"本次未记录提示或先前讲解"}else{"在已提供帮助下"}.into()),
                    demonstrated:e.understanding.demonstrated, difficulties:e.understanding.difficulties,
                    unconfirmed:e.understanding.unconfirmed }).collect();
            view.reviews.push(CourseReview { expression:expression.clone(),
                status:serde_json::to_value(job.status).unwrap().as_str().unwrap().into(), displayed,
                feedback:if displayed { feedback } else { None }, understanding:if displayed { understanding } else { vec![] },
                error:job.error.as_ref().and_then(|e| e["message"].as_str()).map(str::to_owned) });
        }
        if run.decision.is_some() {
            view.decision.as_mut().unwrap().can_continue = decision_ready(&self.connection,&self.workspace_scope,&run)?;
            if let Some((feedback,step)) = decision_guidance(&self.connection,&self.workspace_scope,&run)? {
                let displayed = run.decision.as_ref().unwrap().latest_job.as_ref().is_some_and(|id|run.displayed_feedback.contains(id));
                if displayed {
                    if let Some(goal) = feedback.decision.as_ref().and_then(|d|d.focus_goal.as_ref()).and_then(|id|run.lesson.decision_teaching.as_ref().unwrap().goals.iter().find(|g|&g.id==id)) {
                        let d=view.decision.as_mut().unwrap(); d.focus_title=Some(goal.title.clone()); d.attention=Some(goal.attention.clone());
                        if step["participation"]["choice"] == "support" || run.decision.as_ref().unwrap().intent == CourseReplyIntent::Explain { d.support=Some(goal.support.clone()); }
                    }
                }
            }
        }
        Ok(view)
    }
    pub fn course_run(&self, id: &str) -> Result<CourseRun, ToolError> {
        read_run(&self.connection, &self.workspace_scope, id)
    }
    pub fn course_receipt(
        &self,
        request: &CourseMutation,
    ) -> Result<Option<CourseLessonView>, ToolError> {
        receipt(&self.connection, &self.workspace_scope, request)
    }
    pub fn course_progress(&self) -> Result<Vec<CourseProgress>, ToolError> {
        let mut q=self.connection.prepare("SELECT json_extract(state,'$.progress') FROM course_runs WHERE learning_workspace_id=? ORDER BY rowid DESC").map_err(storage)?;
        let rows = q
            .query_map([&self.workspace_scope], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|r| serde_json::from_str::<CourseProgress>(&r.map_err(storage)?).map_err(storage))
            .collect()
    }
    /// One transaction owns progression, delivered content, frozen assessment and
    /// response facts. A retry returns the original receipt without duplicating them.
    pub fn mutate_course(
        &mut self,
        request: &CourseMutation,
        start: Option<(&str, u32, &CourseLesson)>,
        goal_revision: u32,
        now: &str,
        source_text: Option<&str>,
    ) -> Result<CourseLessonView, ToolError> {
        if request.operation_id.trim().is_empty() {
            return Err(invalid("课程操作缺少身份"));
        }
        // Freeze earlier understanding when the expression is accepted. A delayed
        // coach must not infer its starting point from evidence written later.
        let prior_understanding = if matches!(request.action, CourseAction::Predict { .. } | CourseAction::Investigate { .. }
            | CourseAction::Reflect { .. } | CourseAction::UseClosedBook { .. } | CourseAction::Answer { .. } | CourseAction::AskQuestion { .. } | CourseAction::FeedbackReply { .. } | CourseAction::DecisionReply { .. }) {
            if let Some(id) = &request.run_id {
                let prior_run = self.course_run(id)?;
                self.understanding_summaries(&prior_run.lesson.summary.source.publication.book_id,
                    &prior_run.lesson.summary.source.source_revision)?.into_iter()
                    .filter(|item|item.later_evidence_ref.is_none()).collect::<Vec<_>>()
            } else { vec![] }
        } else { vec![] };
        let scope = &self.workspace_scope;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(old) = receipt(&tx, scope, request)? {
            return Ok(old);
        }
        let mut run = if let CourseAction::Start { lesson_id } = &request.action {
            if request.run_id.is_some() || request.expected_revision != 0 {
                return Err(conflict());
            }
            let (release, version, lesson) = start.ok_or_else(|| invalid("请选择已发布课程"))?;
            if version != request.release_version {
                return Err(conflict());
            }
            if &lesson.summary.id != lesson_id {
                return Err(invalid("课程不存在"));
            }
            CourseRun {
                support_visit: None,
                resolved_questions: vec![],
                progress: CourseProgress {
                    run_id: format!("course:{}", request.operation_id),
                    release_id: release.into(),
                    release_version: version,
                    lesson_id: lesson_id.clone(),
                    revision: 0,
                    goal_revision,
                    stage: CourseStage::Introduction,
                    status: CourseStatus::Active,
                },
                lesson: lesson.clone(),
                prediction: None,
                answer: None,
                preset: None,
                observed: vec![],
                source_excerpt: None,
                support_refs: vec![],
                hint: false,
                assessment: None,
                exploration: None,
                expressions: vec![],
                displayed_feedback: vec![],
                round_index: 0,
                round_history: vec![],
                reflection: None,
                decision: None,
                activity_response: None,
                math_exploration: None,
            }
        } else {
            read_run(
                &tx,
                scope,
                request
                    .run_id
                    .as_deref()
                    .ok_or_else(|| invalid("请选择已开始的课程"))?,
            )?
        };
        if request.expected_revision != run.progress.revision
            || request.release_version != run.progress.release_version
        {
            return Err(conflict());
        }
        let review_action = matches!(request.action, CourseAction::ReadFeedback { .. } | CourseAction::RetryFeedback { .. });
        if run.progress.goal_revision != goal_revision
            && !matches!(request.action, CourseAction::Pause | CourseAction::Skip) && !review_action
        {
            return Err(invalid(
                "学习目标已变化。请保留或跳过这次记录，从课程目录重新开始。",
            ));
        }
        if !review_action && run.progress.status != CourseStatus::Active
            && !(run.progress.status == CourseStatus::Paused
                && matches!(request.action, CourseAction::Resume | CourseAction::Skip))
        {
            return Err(invalid("请先继续课程；已收束的记录不能改写"));
        }
        if run.support_visit.is_some() && !matches!(request.action,
            CourseAction::ReturnFromSupport | CourseAction::Pause | CourseAction::Resume | CourseAction::Skip
            | CourseAction::ReadFeedback { .. } | CourseAction::RetryFeedback { .. }) {
            return Err(invalid("请先回到原问题，再继续课程"));
        }
        let mut response_support = run.support_refs.clone();
        if matches!(request.action, CourseAction::Predict { .. } | CourseAction::Investigate { .. } | CourseAction::Reflect { .. } | CourseAction::Answer { .. } | CourseAction::AskQuestion { .. } | CourseAction::DecisionReply { .. } | CourseAction::FeedbackReply { .. }) {
            // Only completed answers from turns admitted for this exact course
            // precede the new expression. Reuse the original delivery facts.
            let mut q = tx.prepare("SELECT d.event_id FROM teaching_trace b JOIN teaching_trace d
                ON d.learning_workspace_id=b.learning_workspace_id
                AND d.event_id='usage:' || b.session_id || ':assistant:' || json_extract(b.event,'$.binding.turn_id')
                WHERE b.learning_workspace_id=?
                AND json_extract(b.event,'$.payload.inquiry_course.lesson.progress.run_id')=?
                AND json_extract(d.event,'$.payload.usage.kind')='assistant_delivery'
                ORDER BY d.seq").map_err(storage)?;
            let refs = q.query_map(params![scope,run.progress.run_id],|r|r.get::<_,String>(0)).map_err(storage)?
                .collect::<Result<Vec<_>,_>>().map_err(storage)?;
            for id in refs { if !response_support.contains(&id) { response_support.push(id); } }
        }
        let binding = TeachingBinding {
            tutor_session_id: run.progress.run_id.clone(),
            session_revision: run.progress.revision,
            control_revision: 0,
            source_id: run.lesson.summary.source.publication.book_id.clone(),
            source_revision: run.lesson.summary.source.source_revision.clone(),
            map_revision: None,
            chat_session_id: run.progress.run_id.clone(),
            turn_id: request.operation_id.clone(),
        };
        let event_id = format!("{}:{}", run.progress.run_id, request.operation_id);
        let mut facts: Vec<TeachingEvent> = vec![];
        let mut fact = |suffix: &str, kind: TeachingFact, payload: Value, causes: Vec<String>| {
            let id = format!("{event_id}:{suffix}");
            facts.push(TeachingEvent {
                event_id: id.clone(),
                binding: binding.clone(),
                kind,
                causal_refs: causes,
                payload,
                occurred_at: now.into(),
            });
            id
        };
        let content = run.lesson.round_content(run.round_index as usize);
        let delivered = match &request.action {
            CourseAction::Start { .. } => Some(json!({"introduction":run.lesson.introduction,"concepts":run.lesson.concepts})),
            CourseAction::Advance => match run.progress.stage {
                CourseStage::Introduction => {
                    run.progress.stage = CourseStage::Prediction;
                    Some(json!({"prediction":content.prediction}))
                }
                CourseStage::Experiment => {
                    if matches!(content.experiment, CourseExperiment::Inquiry { .. }) && run.activity_response.is_none() {
                        return Err(invalid("请先保存本轮取证、推理或练习的结果"));
                    }
                    if !content.experiment.presets().iter().all(|p| run.observed.contains(p)) {
                        return Err(invalid("请先比较本轮的三种实验配置"));
                    }
                    if content.lab.is_some() && run.exploration.is_none() {
                        return Err(invalid("请保存一次自由实验配置，再对照你的预测"));
                    }
                    run.progress.stage = CourseStage::Explanation;
                    Some(json!({"explanation":content.explanation}))
                }
                CourseStage::Explanation => {
                    if !run.lesson.follow_up_rounds.is_empty() && run.reflection.is_none() {
                        return Err(invalid("请先写下本轮观察怎样支持或修正你的预测，再继续下一段"));
                    }
                    run.round_history.push(CourseRoundRecord {
                        math_exploration: run.math_exploration.clone(),
                        title:run.lesson.round_titles()[run.round_index as usize].clone(),
                        prediction:content.prediction.clone(),response:run.prediction.clone().unwrap(),
                        exploration:run.exploration.clone(),reflection:run.reflection.clone(),
                        explanation:content.explanation.clone(),
                        activity_response:run.activity_response.clone(),
                    });
                    if (run.round_index as usize) < run.lesson.follow_up_rounds.len() {
                        run.round_index += 1;
                        run.prediction = None;
                        run.preset = None;
                        run.observed.clear();
                        run.exploration = None;
                        run.reflection = None;
                        run.activity_response = None;
                        run.math_exploration = None;
                        run.progress.stage = CourseStage::Prediction;
                        let next = run.lesson.round_content(run.round_index as usize);
                        Some(json!({"prediction":next.prediction,"reading":next.reading,"round_title":next.round_title}))
                    } else {
                    run.progress.stage = CourseStage::Task;
                    if let Some(teaching) = &run.lesson.decision_teaching {
                        run.decision = Some(CourseDecisionState { phase:CourseDecisionPhase::Decision, latest_job:None, intent:CourseReplyIntent::Answer });
                        run.exploration = Some(teaching.experiment.clone());
                    }
                    fact(
                        "contract",
                        TeachingFact::MoveSelected,
                        json!({"course_task":run.lesson.task,"assessment":run.lesson.assessment,"source":run.lesson.summary.source,"release_id":run.progress.release_id,"release_version":run.progress.release_version}),
                        vec![],
                    );
                    Some(json!({"task":run.lesson.task}))
                    }
                }
                CourseStage::Feedback => {
                    if let Some(state) = &run.decision {
                        if !decision_ready(&tx,scope,&run)? { return Err(invalid("请先回应当前教学问题；本题的理解还需要确认")); }
                        if state.phase == CourseDecisionPhase::Decision {
                            run.decision = Some(CourseDecisionState { phase:CourseDecisionPhase::Transfer, latest_job:None, intent:CourseReplyIntent::Answer });
                            run.exploration = Some(run.lesson.decision_teaching.as_ref().unwrap().transfer_experiment.clone());
                            Some(json!({"transfer_question":run.lesson.decision_teaching.as_ref().unwrap().transfer_question}))
                        } else {
                            run.progress.stage=CourseStage::Complete; run.progress.status=CourseStatus::Completed; None
                        }
                    } else {
                    run.progress.stage = CourseStage::Complete;
                    run.progress.status = CourseStatus::Completed;
                    None
                    }
                }
                _ => return Err(invalid("请先完成当前活动")),
            },
            CourseAction::Predict { response } => {
                if run.progress.stage != CourseStage::Prediction
                    || (content.prediction.options.is_empty() && (!response.choice.is_empty() || response.reason.trim().is_empty()))
                    || (!content.prediction.options.is_empty() && !content
                        .prediction
                        .options
                        .iter()
                        .any(|o| o.id == response.choice))
                {
                    return Err(invalid("请选择当前预测的一个选项"));
                }
                fact(
                    "prediction",
                    TeachingFact::LearnerAction,
                    json!({"response":response.reason,"choice":response.choice,"selected_option":content.prediction.options.iter().find(|o|o.id==response.choice),"question":content.prediction.question,"course_activity":"prediction","round_index":run.round_index,"support_delivery_refs":response_support}),
                    run.support_refs.clone(),
                );
                run.prediction = Some(response.clone());
                run.progress.stage = CourseStage::Experiment;
                Some(json!({"experiment":content.experiment}))
            }
            CourseAction::Observe { preset } => {
                if !matches!(run.progress.stage, CourseStage::Experiment | CourseStage::Explanation | CourseStage::Task | CourseStage::Feedback) {
                    return Err(invalid("当前不在实验活动"));
                }
                if !content.experiment.presets().contains(preset) { return Err(invalid("请选择本轮提供的实验配置")); }
                run.preset = Some(*preset);
                if !run.observed.contains(preset) {
                    run.observed.push(*preset);
                }
                Some(json!({"experiment":content.experiment,"preset":preset}))
            }
            CourseAction::Investigate { response, visualization } => {
                if run.progress.stage != CourseStage::Experiment || response.trim().is_empty()
                    || !matches!(content.experiment, CourseExperiment::Inquiry { .. }) {
                    return Err(invalid("请在当前探究活动中写下取证、推理或练习结果"));
                }
                let authored = match &content.experiment { CourseExperiment::Inquiry { visualization, .. } => visualization, _ => unreachable!() };
                match (authored, visualization) {
                    (Some(initial), Some(state)) if initial.same_experiment(state) => state.validate()?,
                    (None, None) => (),
                    _ => return Err(invalid("请保存当前数学实验的参数和观察")),
                }
                run.math_exploration = visualization.clone();
                fact("investigation", TeachingFact::LearnerAction,
                    json!({"response":response,"visualization":visualization,"course_activity":"investigation","round_index":run.round_index,
                        "support_delivery_refs":response_support}), run.support_refs.clone());
                run.activity_response = Some(response.clone());
                None
            }
            CourseAction::Explore { state } => {
                if !matches!(run.progress.stage, CourseStage::Experiment | CourseStage::Explanation | CourseStage::Task | CourseStage::Feedback) || content.lab.is_none() {
                    return Err(invalid("当前不在自由实验活动"));
                }
                state.validate()?;
                if state.batch_size.is_some() != content.experiment.batch_size().is_some() {
                    return Err(invalid("实验配置须保留本课的请求数量含义"));
                }
                run.exploration = Some(state.clone());
                Some(json!({"exploration":state,"compute_ms":state.work_gflops/state.compute_tflops,
                    "read_ms":state.read_gb/state.bandwidth_tb_s,"lower_bound_ms":(state.work_gflops/state.compute_tflops).max(state.read_gb/state.bandwidth_tb_s)}))
            }
            CourseAction::DecisionReply { job_ref, response, intent } => {
                let state=run.decision.as_ref().ok_or_else(||invalid("当前没有进行中的决策教学"))?;
                if run.progress.stage != CourseStage::Feedback || response.trim().is_empty() || job_ref != &state.latest_job { return Err(invalid("请回应当前的教学问题")); }
                if let Some(id)=job_ref { if !run.displayed_feedback.contains(id) || decision_guidance(&tx,scope,&run)?.is_none() { return Err(invalid("请先查看本轮反馈，再回应")); } }
                let activity=if state.phase == CourseDecisionPhase::Transfer {"transfer"} else {"decision"};
                let reply_to=job_ref.as_ref().map(|id|feedback_reply_context(&tx,scope,id,run.expressions.iter().find(|e|&e.job_ref==id))).transpose()?;
                fact("decision-reply",TeachingFact::LearnerAction,json!({"response":response,"course_activity":activity,"responds_to":job_ref,"reply_to":reply_to,"intent":intent,"exploration":run.exploration,"support_delivery_refs":response_support}),run.support_refs.clone());
                run.decision.as_mut().unwrap().intent=*intent;
                None
            }
            CourseAction::FeedbackReply { job_ref, response, intent } => {
                let previous=run.expressions.iter().find(|e|&e.job_ref==job_ref).ok_or_else(||invalid("当前没有可接续的课程反馈"))?.clone();
                let response_round=previous.round_index.unwrap_or(0);
                let question=previous.activity=="question";
                let earlier=(response_round as usize)<run.round_history.len() && matches!(previous.activity.as_str(),"prediction"|"reflection");
                let latest=run.expressions.iter().rev().find(|e|e.round_index.unwrap_or(0)==response_round && e.activity==previous.activity);
                let current_activity=match run.progress.stage {
                    CourseStage::Experiment => previous.activity=="prediction",
                    CourseStage::Explanation => matches!(previous.activity.as_str(),"reflection"|"prediction"),
                    CourseStage::Feedback => matches!(previous.activity.as_str(),"task"|"revision"),
                    _ => false,
                };
                if (!question && !earlier && (run.decision.is_some() || !current_activity || response_round!=run.round_index))
                    || (earlier && !matches!(previous.activity.as_str(),"prediction"|"reflection"))
                    || (question && run.expressions.iter().any(|e|e.reply_to.as_ref()==Some(job_ref)))
                    || (!question && latest.is_none_or(|e|&e.job_ref!=job_ref)) || response.trim().is_empty() {
                    return Err(invalid("请回应当前活动的最新反馈"));
                }
                if !run.displayed_feedback.contains(job_ref) { return Err(invalid("请先查看本轮反馈，再回应")); }
                let reply_to=feedback_reply_context(&tx,scope,job_ref,Some(&previous))?;
                // A rejected imported reflection stays on the same original task.
                // Once accepted, later questions use the ordinary feedback path.
                let closed_book_result_ref=previous.closed_book_result_ref.as_ref()
                    .filter(|_| !earlier && run.reflection.is_none());
                let closed_book_result=closed_book_result_ref.map(|id|
                    read_closed_book_result(&tx,scope,id,&run).map(|result|result.context)).transpose()?;
                fact("feedback-reply",TeachingFact::LearnerAction,
                    json!({"response":response,"course_activity":previous.activity,"responds_to":job_ref,"intent":intent,
                        "reply_to":reply_to,
                        "closed_book_result_ref":closed_book_result_ref,"closed_book_result":closed_book_result,
                        "round_index":response_round,"exploration":if earlier{run.round_history[response_round as usize].exploration.clone()}else{run.exploration.clone()},"support_delivery_refs":response_support}),run.support_refs.clone());
                if previous.activity=="reflection" && *intent==CourseReplyIntent::Answer && closed_book_result_ref.is_none() {
                    if earlier {run.round_history[response_round as usize].reflection=Some(response.clone());}
                    else {run.reflection=Some(response.clone());}
                }
                None
            }
            CourseAction::AskQuestion { response } => {
                if response.trim().is_empty() { return Err(invalid("先写下你的疑问")); }
                fact("question",TeachingFact::LearnerAction,
                    json!({"response":response,"course_activity":"question","intent":"question",
                        "round_index":run.round_index,"support_delivery_refs":response_support,"exploration":run.exploration}),run.support_refs.clone());
                None
            }
            CourseAction::ResolveQuestion { fact_ref, resolved } => {
                if !run.expressions.iter().any(|e|&e.fact_ref==fact_ref && e.intent==Some(CourseReplyIntent::Question)) {
                    return Err(invalid("请选中本课提出的疑问"));
                }
                run.resolved_questions.retain(|id|id!=fact_ref);
                if *resolved {run.resolved_questions.push(fact_ref.clone());}
                None
            }
            CourseAction::OpenSupport { job_ref } => {
                let expression=run.expressions.iter().find(|e|&e.job_ref==job_ref).ok_or_else(||invalid("补学不属于本课记录"))?;
                if !run.displayed_feedback.contains(job_ref) {return Err(invalid("请先查看当前反馈"));}
                let raw:String=tx.query_row("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND job_ref=?",params![scope,job_ref],|r|r.get(0)).map_err(storage)?;
                let job:crate::tutor_coach::CoachJob=serde_json::from_str(&raw).map_err(storage)?;
                let branch:CourseSupportBranch=serde_json::from_value(job.guidance.as_ref().and_then(|g|g.pointer("/course_feedback/support_branch")).cloned().ok_or_else(||invalid("本次反馈没有补学内容"))?).map_err(storage)?;
                run.support_visit=Some(CourseSupportVisit {job_ref:job_ref.clone(),original_response:expression.response.clone(),branch:branch.clone()});
                Some(json!({"support_branch":branch,"return_to":job_ref}))
            }
            CourseAction::ReturnFromSupport => {
                if run.support_visit.take().is_none() { return Err(invalid("当前没有进行中的补学")); }
                None
            }
            CourseAction::UseClosedBook { result_ref } => {
                let result = read_closed_book_result(&tx, scope, result_ref, &run)?;
                if run.expressions.iter().any(|expression| expression.closed_book_result_ref.as_ref() == Some(result_ref)) {
                    None
                } else {
                    if !result.origin.matches_run(&run) {
                        return Err(invalid("课程已进入另一项任务。请回到当前任务继续，原练习结果已保留。"));
                    }
                    let activity = if let Some(state) = &run.decision {
                        Some(if state.phase == CourseDecisionPhase::Transfer { "transfer" } else { "decision" })
                    } else if run.progress.stage == CourseStage::Explanation { Some("reflection") } else { None };
                    if let Some(activity) = activity {
                        let question = result.origin.question.clone();
                        fact("closed-book-response", TeachingFact::LearnerAction,
                            json!({"response":result.response,"course_activity":activity,"intent":"answer",
                                "question":question,"round_index":run.round_index,"exploration":run.exploration,
                                "closed_book_result_ref":result_ref,"closed_book_result":result.context,
                                "support_delivery_refs":[],"source_expression_refs":result.expression_refs}),
                            vec![result_ref.clone()]);
                        if let Some(state) = &mut run.decision {
                            state.intent = CourseReplyIntent::Answer;
                            run.progress.stage = CourseStage::Feedback;
                        }
                    }
                    None
                }
            }
            CourseAction::Reflect { response } => {
                if run.decision.is_some() { return Err(invalid("请在当前决策教学中回应")); }
                if !matches!(run.progress.stage, CourseStage::Explanation | CourseStage::Feedback) || response.trim().is_empty() {
                    return Err(invalid("请在解释或回看环节写下你的理解"));
                }
                fact("reflection", TeachingFact::LearnerAction,
                    json!({"response":response,"course_activity":if run.progress.stage == CourseStage::Feedback {"revision"} else {"reflection"},
                        "round_index":run.round_index,"support_delivery_refs":response_support,"exploration":run.exploration}), run.support_refs.clone());
                if run.progress.stage == CourseStage::Explanation { run.reflection = Some(response.clone()); }
                None
            }
            CourseAction::ReadFeedback { job_ref } | CourseAction::RetryFeedback { job_ref } => {
                let feedback_intent=run.expressions.iter().find(|e|&e.job_ref==job_ref).ok_or_else(||invalid("反馈不属于本课记录"))?.intent;
                let raw:String = tx.query_row("SELECT job FROM teaching_coach_jobs WHERE learning_workspace_id=? AND job_ref=?",
                    params![scope,job_ref],|r|r.get(0)).map_err(storage)?;
                let mut job:crate::tutor_coach::CoachJob = serde_json::from_str(&raw).map_err(storage)?;
                if matches!(request.action, CourseAction::ReadFeedback { .. }) {
                    if job.status != crate::tutor_coach::CoachJobStatus::Ready { return Err(invalid("反馈还未就绪")); }
                    if let Some(expression) = run.expressions.iter().find(|e| &e.job_ref == job_ref
                        && e.closed_book_result_ref.is_some() && e.activity == "reflection") {
                        let reuse: CourseReuseJudgment = serde_json::from_value(job.guidance.as_ref()
                            .and_then(|g|g.pointer("/course_feedback/course_reuse")).cloned()
                            .ok_or_else(||invalid("闭卷表达还未按原课程任务确认，请重试反馈"))?).map_err(storage)?;
                        if reuse.reason.trim().is_empty() { return Err(invalid("回课判断需要说明原任务已满足或尚缺什么")); }
                        if reuse.accepted && expression.intent == Some(CourseReplyIntent::Answer)
                            && expression.round_index == Some(run.round_index)
                            && run.progress.stage == CourseStage::Explanation
                            && run.expressions.iter().rev().find(|e| e.activity == "reflection"
                                && e.round_index == Some(run.round_index)).is_some_and(|e| &e.job_ref == job_ref) {
                            run.reflection = Some(expression.response.clone());
                        }
                    }
                    if run.displayed_feedback.contains(job_ref) { None } else {
                        run.displayed_feedback.push(job_ref.clone());
                        {
                            let step=job.guidance.as_ref().map(|g|&g["next_step"]);
                            let focus=job.guidance.as_ref().and_then(|g|g["course_feedback"]["decision"]["focus_goal"].as_str()).and_then(|id|run.lesson.decision_teaching.as_ref()?.goals.iter().find(|g|g.id==id));
                            let support=focus.filter(|_|step.is_some_and(|s|s["participation"]["choice"]=="support") || run.decision.as_ref().is_some_and(|d|d.intent==CourseReplyIntent::Explain)).map(|g|&g.support);
                            let mut feedback=job.guidance.as_ref().map(|g|learner_feedback(&g["course_feedback"],feedback_intent));
                            if let Some(Value::Object(value))=&mut feedback { value.remove("support_branch"); }
                            Some(json!({"personal_feedback":feedback,"job_ref":job_ref,"teaching_support":support}))
                        }
                    }
                } else {
                    if !matches!(job.status, crate::tutor_coach::CoachJobStatus::Failed | crate::tutor_coach::CoachJobStatus::Interrupted) {
                        return Err(invalid("只有失败或中断的反馈可以重试"));
                    }
                    job.status = crate::tutor_coach::CoachJobStatus::Pending;
                    job.updated_at = now.into(); job.error = None;
                    tx.execute("UPDATE teaching_coach_jobs SET status='pending',job=? WHERE learning_workspace_id=? AND job_ref=?",
                        params![serde_json::to_string(&job).map_err(storage)?,scope,job_ref]).map_err(storage)?;
                    None
                }
            }
            CourseAction::Source => {
                let text = source_text
                    .filter(|t| !t.trim().is_empty())
                    .ok_or_else(|| invalid("原文暂时无法读取"))?;
                run.source_excerpt = Some(text.into());
                Some(json!({"source_excerpt":text}))
            }
            CourseAction::Hint => {
                if run.progress.stage != CourseStage::Task || run.hint {
                    return Err(invalid("当前提示不可用"));
                }
                run.hint = true;
                Some(json!({"hint":run.lesson.hint}))
            }
            CourseAction::Answer { response } => {
                if run.progress.stage != CourseStage::Task
                    || (run.lesson.task.options.is_empty() && (!response.choice.is_empty() || response.reason.trim().is_empty()))
                    || (!run.lesson.task.options.is_empty() && !run
                        .lesson
                        .task
                        .options
                        .iter()
                        .any(|o| o.id == response.choice))
                {
                    return Err(invalid("请选择当前任务的一个选项"));
                }
                let contract:String=tx.query_row("SELECT event_id FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.payload.course_task') IS NOT NULL ORDER BY seq DESC LIMIT 1",params![scope,run.progress.run_id],|r|r.get(0)).map_err(storage)?;
                let mut causes = run.support_refs.clone();
                causes.push(contract.clone());
                let response_id = fact(
                    "answer",
                    TeachingFact::LearnerAction,
                    json!({"response":response.reason,"choice":response.choice,"selected_option":run.lesson.task.options.iter().find(|o|o.id==response.choice),"question":run.lesson.task.question,"course_activity":"task","assessment_contract_ref":contract,"support_delivery_refs":response_support,"reason_assessed":false}),
                    causes,
                );
                let assessment = ResponseAssessment {
                    action_ref: response_id,
                    contract_ref: contract,
                    evaluator_version: if run.lesson.task.options.is_empty() { "course-open-response/1" } else { "course-choice-set/1" }.into(),
                    status: if run.lesson.task.options.is_empty() { AssessmentStatus::Unassessed } else { assess_closed(&run.lesson.assessment, &response.choice)? },
                    items: vec![],
                };
                run.answer = Some(response.clone());
                run.assessment = Some(assessment);
                run.progress.stage = CourseStage::Feedback;
                if let Some(teaching)=&run.lesson.decision_teaching {
                    if run.decision.is_none() {
                        run.decision=Some(CourseDecisionState { phase:CourseDecisionPhase::Decision,latest_job:None,intent:CourseReplyIntent::Answer });
                        run.exploration=Some(teaching.experiment.clone());
                    }
                    None
                } else {
                    Some(json!({"solution":run.lesson.solution,"assessment_status":run.assessment.as_ref().map(|a|&a.status)}))
                }
            }
            CourseAction::Pause => {
                run.progress.status = CourseStatus::Paused;
                None
            }
            CourseAction::Resume => {
                run.progress.status = CourseStatus::Active;
                None
            }
            CourseAction::Skip => {
                run.progress.status = CourseStatus::Skipped;
                None
            }
        };
        if let Some(content) = delivered {
            let help = matches!(request.action, CourseAction::Hint | CourseAction::Source | CourseAction::OpenSupport { .. });
            let id = fact(
                "delivery",
                if help {
                    TeachingFact::HelpDisplayed
                } else {
                    TeachingFact::MessageDelivered
                },
                json!({"text":content.to_string(),"course_content":content,"course_stage":run.progress.stage,"round_index":run.round_index,"source":run.lesson.summary.source}),
                vec![],
            );
            run.support_refs.push(id);
        }
        run.progress.revision += 1;
        for event in &facts {
            crate::teaching::append_in(&tx, scope, event)?;
        }
        for event in facts.iter().filter(|e| e.learner_expression().is_some()) {
            enqueue_response(&tx, scope, &mut run, event, now, &prior_understanding)?;
        }
        if let Some(a) = &run.assessment {
            tx.execute("INSERT OR IGNORE INTO teaching_assessments(learning_workspace_id,action_ref,assessment) VALUES(?,?,?)",params![scope,a.action_ref,serde_json::to_string(a).map_err(storage)?]).map_err(storage)?;
        }
        tx.execute("INSERT INTO course_runs(learning_workspace_id,run_id,release_id,release_version,lesson_id,state) VALUES(?,?,?,?,?,?) ON CONFLICT(learning_workspace_id,run_id) DO UPDATE SET state=excluded.state",params![scope,run.progress.run_id,run.progress.release_id,run.progress.release_version,run.progress.lesson_id,serde_json::to_string(&run).map_err(storage)?]).map_err(storage)?;
        let view = run.view();
        tx.execute("INSERT INTO course_operations(learning_workspace_id,operation_id,request,run_id,result) VALUES(?,?,?,?,?)",params![scope,request.operation_id,serde_json::to_string(request).map_err(storage)?,run.progress.run_id,serde_json::to_string(&view).map_err(storage)?]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(view)
    }
}
