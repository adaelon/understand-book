//! Exam settings join the accepted goal revision; evidence and teaching remain in existing stores.
use crate::{control_store::ControlStore, learning_workspace, user_storage_paths::error};
use base_schema::WorkspaceGoalInput;
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn invalid() -> ToolError { error("EXAM_SPRINT_INVALID", "validation", "请填写有效考试日期、复习范围和每天可用时间。") }
fn storage() -> ToolError { error("EXAM_SPRINT_UNAVAILABLE", "unavailable", "考试安排暂时无法读取或保存。") }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SprintInput {
    pub exam_date: String,
    #[serde(default)]
    pub syllabus: String,
    pub daily_minutes: Option<u32>,
    /// Calendar boundaries are explicitly the student's local offset, in minutes.
    pub utc_offset_minutes: i32,
    pub entry_mode: String,
}

impl SprintInput {
    fn date(&self) -> Result<time::Date, ToolError> {
        let format = time::format_description::parse("[year]-[month]-[day]").map_err(|_| invalid())?;
        time::Date::parse(&self.exam_date, &format).map_err(|_| invalid())
    }
    pub(crate) fn validate(&self) -> Result<(), ToolError> {
        self.date()?;
        if !matches!(self.entry_mode.as_str(), "diagnostic" | "question")
            || self.daily_minutes.is_some_and(|minutes| minutes == 0 || minutes > 1440)
            || !(-720..=840).contains(&self.utc_offset_minutes) { return Err(invalid()); }
        Ok(())
    }
    pub(crate) fn constraints(&self) -> Vec<String> {
        let mut constraints = vec![format!("考试日期：{}（UTC{:+} 分钟）；按当前日期安排复习。", self.exam_date, self.utc_offset_minutes)];
        constraints.push(if self.syllabus.trim().is_empty() { "考试范围暂未提供；先依据课程注明的适用范围，不猜测学校考纲。".into() }
            else { format!("已确认考试范围：{}", self.syllabus.trim()) });
        if let Some(minutes) = self.daily_minutes { constraints.push(format!("每天可用复习时间：{minutes} 分钟。")); }
        constraints.push(if self.entry_mode == "diagnostic" { "开始方式：先做短诊断，根据实际作答调整；观看视频不代表已掌握。".into() }
            else { "开始方式：直接处理我提出的问题，无需先做诊断；根据实际作答调整。".into() });
        constraints
    }
    fn remaining_days(&self, now: i64) -> Result<i64, ToolError> {
        let offset = time::UtcOffset::from_whole_seconds(self.utc_offset_minutes * 60).map_err(|_| invalid())?;
        let today = time::OffsetDateTime::from_unix_timestamp(now).map_err(|_| invalid())?.to_offset(offset).date();
        Ok((self.date()? - today).whole_days())
    }
    pub(crate) fn calendar(&self, now: i64, goal_revision: u32) -> Result<Value, ToolError> {
        let offset = time::UtcOffset::from_whole_seconds(self.utc_offset_minutes * 60).map_err(|_| invalid())?;
        let today = time::OffsetDateTime::from_unix_timestamp(now).map_err(|_| invalid())?.to_offset(offset).date();
        Ok(json!({"exam_date":self.exam_date,"current_local_date":today.to_string(),
            "utc_offset_minutes":self.utc_offset_minutes,"remaining_days":self.remaining_days(now)?,
            "workspace_goal_revision":goal_revision,"as_of":now}))
    }
}

pub(crate) fn settings(db: &Connection, owner: &str, workspace: &str) -> Result<Option<SprintInput>, ToolError> {
    let row: Option<String> = db.query_row("SELECT settings_json FROM workspace_exam_sprints WHERE owner_user_id=? AND learning_workspace_id=?",
        params![owner,workspace],|r|r.get(0)).optional().map_err(|_|storage())?;
    row.map(|row|serde_json::from_str(&row).map_err(|_|storage())).transpose()
}

pub(crate) fn save(db: &Connection, owner: &str, workspace: &str, revision: u32, input: &SprintInput, now: i64) -> Result<(), ToolError> {
    db.execute("INSERT INTO workspace_exam_sprints(owner_user_id,learning_workspace_id,goal_revision,settings_json,updated_at) VALUES(?,?,?,?,?)
        ON CONFLICT(owner_user_id,learning_workspace_id) DO UPDATE SET goal_revision=excluded.goal_revision,settings_json=excluded.settings_json,updated_at=excluded.updated_at",
        params![owner,workspace,revision,serde_json::to_string(input).map_err(|_|storage())?,now]).map_err(|_|storage())?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviseSprint {
    pub expected_goal_revision: u32,
    pub goal_text: String,
    pub sprint: SprintInput,
}

impl ControlStore {
    pub(crate) fn exam_sprint(&self, owner: &str, workspace: &str, now: i64) -> Result<Value, ToolError> {
        let learning = self.learning_workspace(owner, workspace)?;
        let row: Option<(u32,String,i64)> = self.connection.query_row(
            "SELECT goal_revision,settings_json,updated_at FROM workspace_exam_sprints WHERE owner_user_id=? AND learning_workspace_id=?",
            params![owner,workspace], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).optional().map_err(|_|storage())?;
        let sprint = row.map(|(revision,settings,updated)| {
            let settings: SprintInput = serde_json::from_str(&settings).map_err(|_|storage())?;
            Ok::<Value, ToolError>(json!({"settings":settings,"goal_revision":revision,"current_goal_revision":learning.goal.revision,
                "remaining_days":settings.remaining_days(now)?,"updated_at":updated,"as_of":now}))
        }).transpose()?;
        Ok(json!({"sprint":sprint}))
    }

    pub(crate) fn revise_exam_sprint(&mut self, owner: &str, workspace: &str, input: ReviseSprint, now: i64) -> Result<Value, ToolError> {
        input.sprint.validate()?;
        let current = self.learning_workspace(owner, workspace)?;
        let mut constraints = current.goal.constraints.clone();
        let old: Option<String> = self.connection.query_row(
            "SELECT settings_json FROM workspace_exam_sprints WHERE owner_user_id=? AND learning_workspace_id=?",
            params![owner,workspace],|r|r.get(0)).optional().map_err(|_|storage())?;
        if let Some(old) = old {
            let old: SprintInput = serde_json::from_str(&old).map_err(|_|storage())?;
            constraints.retain(|constraint| !old.constraints().contains(constraint));
        }
        constraints.extend(input.sprint.constraints());
        let goal = WorkspaceGoalInput { text: input.goal_text.trim().into(), constraints };
        learning_workspace::validate_goal(&goal)?;
        let tx = self.connection.transaction().map_err(|_|storage())?;
        learning_workspace::require_active(&tx,owner,workspace)?;
        let revision: u32 = tx.query_row("SELECT MAX(revision) FROM learning_workspace_goals WHERE owner_user_id=? AND learning_workspace_id=?",params![owner,workspace],|r|r.get(0)).map_err(|_|storage())?;
        if revision != input.expected_goal_revision {
            return Err(error("EXAM_GOAL_VERSION_CONFLICT","conflict","目标已更新，请重新读取后保存考试安排。"));
        }
        learning_workspace::insert_goal(&tx,owner,workspace,revision+1,&goal,now)?;
        save(&tx,owner,workspace,revision+1,&input.sprint,now)?;
        tx.execute("UPDATE learning_workspaces SET updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?",params![now,owner,workspace]).map_err(|_|storage())?;
        tx.commit().map_err(|_|storage())?;
        Ok(json!({"workspace":self.learning_workspace(owner,workspace)?,"sprint":self.exam_sprint(owner,workspace,now)?["sprint"]}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_countdown_advances_at_students_midnight() {
        let sprint = SprintInput { exam_date:"2026-10-23".into(), syllabus:String::new(), daily_minutes:Some(90), utc_offset_minutes:480, entry_mode:"question".into() };
        let now = time::OffsetDateTime::parse("2026-10-20T15:59:00Z",&time::format_description::well_known::Rfc3339).unwrap().unix_timestamp();
        assert_eq!(sprint.remaining_days(now).unwrap(),3);
        assert_eq!(sprint.remaining_days(now+120).unwrap(),2);
        let calendar = sprint.calendar(now+120,2).unwrap();
        assert_eq!(calendar["current_local_date"],"2026-10-21");
        assert_eq!(calendar["workspace_goal_revision"],2);
        assert!(sprint.constraints().iter().any(|constraint|constraint.contains("无需先做诊断")));
        let mut invalid = sprint; invalid.exam_date="2026-02-30".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn revision_is_atomic_private_and_invalidates_the_old_plan_without_rewriting_history() {
        let root = tempfile::tempdir().unwrap();
        let writer = crate::control_store::ServiceWriter::acquire(root.path()).unwrap();
        let mut store = ControlStore::open(writer).unwrap();
        store.create_user("A").unwrap(); store.create_user("B").unwrap();
        let workspace = store.create_learning_workspace("A", WorkspaceGoalInput { text:"通过这门考试".into(), constraints:vec!["保留手写笔记".into()] },vec![],1).unwrap();
        let settings = SprintInput { exam_date:"2026-10-23".into(), syllabus:"只考前三章".into(), daily_minutes:Some(60), utc_offset_minutes:480, entry_mode:"question".into() };
        let now = time::OffsetDateTime::parse("2026-10-20T15:59:00Z",&time::format_description::well_known::Rfc3339).unwrap().unix_timestamp();
        let id = &workspace.learning_workspace_id;
        let result = store.revise_exam_sprint("A",id,ReviseSprint { expected_goal_revision:1, goal_text:"通过这门考试".into(), sprint:settings.clone() },now).unwrap();
        assert_eq!(result["workspace"]["goal"]["revision"],2);
        assert_eq!(result["sprint"]["remaining_days"],3);
        assert_eq!(store.exam_sprint("A",id,now+120).unwrap()["sprint"]["remaining_days"],2);
        assert!(store.exam_sprint("B",id,now).is_err());
        let old_goal = result["workspace"]["goal"].clone();
        let changed = SprintInput { exam_date:"2026-10-25".into(), syllabus:"改考第四章".into(), ..settings.clone() };
        assert_eq!(store.revise_exam_sprint("A",id,ReviseSprint { expected_goal_revision:1,goal_text:"更新目标".into(),sprint:changed.clone() },now).unwrap_err().error_code,"EXAM_GOAL_VERSION_CONFLICT");
        let result = store.revise_exam_sprint("A",id,ReviseSprint { expected_goal_revision:2,goal_text:"更新目标".into(),sprint:changed },now).unwrap();
        assert_eq!(result["workspace"]["goal_history"][1],old_goal);
        assert_eq!(result["workspace"]["goal"]["revision"],3);
        let constraints = result["workspace"]["goal"]["constraints"].to_string();
        assert!(constraints.contains("保留手写笔记"));
        assert!(constraints.contains("改考第四章"));
        assert!(!constraints.contains("只考前三章"));
        assert_eq!(result["sprint"]["goal_revision"],3);
    }
}
