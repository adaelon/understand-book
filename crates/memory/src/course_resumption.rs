//! Read the location of a saved course without loading its complete frozen lesson.
use crate::{
    course::{CourseDecisionPhase, CourseLessonSummary, CourseProgress},
    learning::{storage, LearningStore},
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CourseResumption {
    pub progress: CourseProgress,
    pub summary: CourseLessonSummary,
    pub round_index: u32,
    pub decision_phase: Option<CourseDecisionPhase>,
    pub question: String,
}

impl LearningStore {
    pub fn course_resumption(&self, id: &str) -> Result<Option<CourseResumption>, ToolError> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT json_object(
                'progress',json_extract(state,'$.progress'),
                'summary',json_extract(state,'$.lesson.summary'),
                'round_index',COALESCE(json_extract(state,'$.round_index'),0),
                'decision_phase',json_extract(state,'$.decision.phase'),
                'question',COALESCE(json_extract(state,'$.support_visit.branch.return_prompt'),
                    CASE json_extract(state,'$.progress.stage')
                    WHEN 'prediction' THEN CASE WHEN COALESCE(json_extract(state,'$.round_index'),0)=0
                        THEN json_extract(state,'$.lesson.prediction.question')
                        ELSE json_extract(state,'$.lesson.follow_up_rounds[' || (json_extract(state,'$.round_index')-1) || '].prediction.question') END
                    WHEN 'explanation' THEN CASE WHEN COALESCE(json_extract(state,'$.round_index'),0)=0
                        THEN json_extract(state,'$.lesson.reflection_prompt')
                        ELSE json_extract(state,'$.lesson.follow_up_rounds[' || (json_extract(state,'$.round_index')-1) || '].reflection_prompt') END
                    WHEN 'task' THEN CASE WHEN json_extract(state,'$.decision.phase')='transfer'
                        THEN json_extract(state,'$.lesson.decision_teaching.transfer_question') ELSE json_extract(state,'$.lesson.task.question') END
                    WHEN 'feedback' THEN CASE WHEN json_extract(state,'$.decision.phase')='transfer'
                        THEN json_extract(state,'$.lesson.decision_teaching.transfer_question') ELSE json_extract(state,'$.lesson.task.question') END
                    END, json_extract(state,'$.lesson.summary.objective')))
             FROM course_runs WHERE learning_workspace_id=? AND run_id=?",
            params![self.workspace_scope, id], |row| row.get(0),
        ).optional().map_err(storage)?;
        raw.map(|value| serde_json::from_str(&value).map_err(storage))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::{CourseAction, CourseMutation, InquiryCourse};

    #[test]
    fn learning_task_course_summary_reads_original_scope_and_revealed_question() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("learning.db");
        let mut store = LearningStore::open(&file, false)
            .unwrap()
            .with_workspace(Some("original"));
        let course: InquiryCourse = serde_json::from_str(include_str!(
            "../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
        ))
        .unwrap();
        let lesson = &course.lessons[0];
        let started = store
            .mutate_course(
                &CourseMutation {
                    operation_id: "resumption".into(),
                    run_id: None,
                    expected_revision: 0,
                    release_version: 1,
                    action: CourseAction::Start {
                        lesson_id: lesson.summary.id.clone(),
                    },
                },
                Some(("release", 1, lesson)),
                1,
                "now",
                None,
            )
            .unwrap();
        let initial = store
            .course_resumption(&started.progress.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(initial.question, lesson.summary.objective);
        assert_eq!(initial.summary.source, lesson.summary.source);
        assert!(LearningStore::open(&file, false)
            .unwrap()
            .with_workspace(Some("another"))
            .course_resumption(&started.progress.run_id)
            .unwrap()
            .is_none());
        let advanced = store
            .mutate_course(
                &CourseMutation {
                    operation_id: "resumption-advance".into(),
                    run_id: Some(started.progress.run_id.clone()),
                    expected_revision: started.progress.revision,
                    release_version: 1,
                    action: CourseAction::Advance,
                },
                None,
                1,
                "later",
                None,
            )
            .unwrap();
        let resumed = store
            .course_resumption(&started.progress.run_id)
            .unwrap()
            .unwrap();
        assert_eq!(resumed.progress, advanced.progress);
        assert_eq!(resumed.question, lesson.prediction.question);
        assert_ne!(resumed.question, lesson.task.question);
    }
}
