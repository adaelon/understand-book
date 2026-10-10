use serde::{Deserialize, Serialize};

/// The user task belongs to a Resident chat, not to a single model run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidentGoal {
    pub id: String,
    pub revision: u64,
    pub origin_turn_id: String,
    #[serde(default)]
    pub related_goal_id: Option<String>,
    pub user_message_refs: Vec<String>,
    pub interpretation: String,
    pub requirements: Vec<GoalRequirement>,
    #[serde(default)]
    pub working: GoalWorkingState,
    #[serde(default)]
    pub result_refs: Vec<String>,
    pub status: GoalStatus,
    #[serde(default)]
    pub last_stop_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalRequirement {
    pub id: String,
    pub description: String,
    pub basis_turn_id: String,
    pub verification: GoalVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalVerification {
    Content,
    PresentationDelivery,
    ReaderAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Open,
    Completed,
    Cancelled,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalWorkItem {
    pub id: String,
    pub description: String,
    pub status: GoalWorkItemStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalWorkItemStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalWorkingState {
    pub focus: String,
    pub open_questions: Vec<String>,
    pub next_move: String,
    #[serde(default)]
    pub items: Vec<GoalWorkItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum GoalUpdate {
    Working {
        focus: String,
        #[serde(default)]
        open_questions: Vec<String>,
        next_move: String,
        #[serde(default)]
        items: Option<Vec<GoalWorkItem>>,
    },
    Refine {
        interpretation: String,
        requirements: Vec<GoalRequirement>,
    },
    Revise {
        basis_turn_id: String,
        basis_quote: String,
        interpretation: String,
        requirements: Vec<GoalRequirement>,
    },
}

fn explicit_text_only(request: &str) -> bool {
    let request = request.to_lowercase();
    ["只要文字", "只用文字", "改成文字", "不需要演示", "不要演示", "不用网页", "无需页面", "text only", "no page"]
        .iter().any(|phrase| request.contains(phrase))
}

impl ResidentGoal {
    pub fn objective_gap(&self, delivered_presentations: usize, reader_effects: usize) -> Option<&'static str> {
        if self.status != GoalStatus::Open { return None; }
        if self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery)
            && delivered_presentations == 0 {
            return Some("The requested presentation has not been delivered. A draft, preview, or prose answer does not satisfy it.");
        }
        if self.requirements.iter().any(|r| r.verification == GoalVerification::ReaderAction)
            && reader_effects == 0 {
            return Some("The requested host Reader action has not been performed. reader_action requires an actual book navigation or Reader UI effect. Authored page controls (dragging, playback, pause, seeking or scene restoration) are content requirements. If you misclassified those controls, correct their verification with goal.update using the existing refine/revise rules while retaining every requested feature; otherwise perform the requested host Reader action.");
        }
        None
    }

    pub fn new(id: String, turn_id: String, user_request: String) -> Self {
        let request = user_request.to_lowercase();
        let text_only = explicit_text_only(&request);
        let presentation = !text_only && ["富文本演示", "演示页", "网页", "页面", "html", "web page", "interactive page"]
            .iter().any(|term| request.contains(term))
            && ["做", "制作", "生成", "创建", "演示", "展示", "修改", "make", "create", "show", "present", "edit"]
                .iter().any(|verb| request.contains(verb));
        Self {
            id,
            revision: 1,
            origin_turn_id: turn_id.clone(),
            related_goal_id: None,
            user_message_refs: vec![turn_id.clone()],
            interpretation: user_request.clone(),
            requirements: vec![GoalRequirement {
                id: format!("{turn_id}:{}", if presentation { "presentation" } else { "content" }),
                description: user_request,
                basis_turn_id: turn_id,
                verification: if presentation { GoalVerification::PresentationDelivery } else { GoalVerification::Content },
            }],
            working: GoalWorkingState::default(),
            result_refs: Vec::new(),
            status: GoalStatus::Open,
            last_stop_reason: None,
        }
    }

    pub fn apply_update(&mut self, update: GoalUpdate, current_turn_id: &str, current_user: &str) -> Result<bool, String> {
        if self.status != GoalStatus::Open {
            return Err("only an open goal can be updated".into());
        }
        let previous = self.clone();
        match update {
            GoalUpdate::Working { focus, open_questions, next_move, items } => {
                if let Some(items) = items {
                    if items.iter().any(|item| item.id.trim().is_empty() || item.description.trim().is_empty()) {
                        return Err("goal work items need non-empty IDs and descriptions".into());
                    }
                    let mut ids = std::collections::HashSet::new();
                    if items.iter().any(|item| !ids.insert(&item.id)) {
                        return Err("goal work item IDs must be unique".into());
                    }
                    self.working.items = items;
                }
                self.working.focus = focus;
                self.working.open_questions = open_questions;
                self.working.next_move = next_move;
            }
            GoalUpdate::Refine { interpretation, requirements } => {
                if self.origin_turn_id != current_turn_id {
                    return Err("refine is limited to the goal's origin turn".into());
                }
                if self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery)
                    && !requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery) {
                    return Err("refine cannot remove the user's explicit presentation request".into());
                }
                self.replace_requirements(interpretation, requirements, current_turn_id)?;
            }
            GoalUpdate::Revise { basis_turn_id, basis_quote, interpretation, requirements } => {
                if basis_turn_id != current_turn_id || basis_quote.trim().is_empty() || !current_user.contains(&basis_quote) {
                    return Err("revision must quote the current user message".into());
                }
                if self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery)
                    && !requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery)
                    && !explicit_text_only(current_user) {
                    return Err("removing presentation delivery needs an explicit text-only user request".into());
                }
                self.replace_requirements(interpretation, requirements, current_turn_id)?;
                if !self.user_message_refs.contains(&basis_turn_id) {
                    self.user_message_refs.push(basis_turn_id);
                }
            }
        }
        if *self != previous {
            self.revision += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn replace_requirements(&mut self, interpretation: String, requirements: Vec<GoalRequirement>, current_turn_id: &str) -> Result<(), String> {
        if interpretation.trim().is_empty() || requirements.is_empty() || requirements.iter().any(|r| r.id.trim().is_empty() || r.description.trim().is_empty() || (r.basis_turn_id != self.origin_turn_id && r.basis_turn_id != current_turn_id)) {
            return Err("goal interpretation and requirements need a valid user-turn basis".into());
        }
        let mut ids = std::collections::HashSet::new();
        if requirements.iter().any(|r| !ids.insert(&r.id)) {
            return Err("goal requirement IDs must be unique".into());
        }
        self.interpretation = interpretation;
        self.requirements = requirements;
        Ok(())
    }

    pub fn projection(&self, observed_passages: usize, pending_candidates: usize, delivered_presentations: usize) -> String {
        let delivery_required = self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery);
        format!(
            "resident_goal.v1\nid={} revision={} status={:?}\nOrigin turn: {}. Current request turn: {}.\nUser task: {}\nRequirements: {}\nActual results in this run: observed passages={observed_passages}, uncommitted presentation candidates={pending_candidates}, delivered presentations={delivered_presentations}.\nSaved result references: {}\nRemaining delivery: {}\nWorking focus: {}\nOpen questions: {}\nNext move: {}\nWork items (method/progress only): {}\nThis current Goal revision is authoritative over historical active_goal summaries. Work items may change without changing requirements; completed items do not prove delivery or content completeness. A source receipt or candidate is not a delivered page. An old tool activation or candidate cannot be reused in a new run.",
            self.id, self.revision, self.status, self.origin_turn_id, self.user_message_refs.last().map(String::as_str).unwrap_or(&self.origin_turn_id), self.interpretation,
            self.requirements.iter().map(|r| format!("{} [{:?}]", r.description, r.verification)).collect::<Vec<_>>().join("; "),
            self.result_refs.join(", "),
            if delivery_required && delivered_presentations == 0 { "presentation delivery still required" } else { "no objective presentation gap recorded" },
            self.working.focus, self.working.open_questions.join("; "), self.working.next_move,
            serde_json::to_string(&self.working.items).expect("goal work items serialize"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, basis: &str) -> GoalRequirement {
        GoalRequirement { id: id.into(), description: "Deliver a chapter overview page".into(), basis_turn_id: basis.into(), verification: GoalVerification::PresentationDelivery }
    }

    #[test]
    fn original_request_can_be_refined_and_working_updates_do_not_erase_delivery() {
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "把这一章富文本演示给我看".into());
        assert!(goal.apply_update(GoalUpdate::Working { focus: "read chapter".into(), open_questions: vec![], next_move: "author".into(), items: None }, "t1", "把这一章富文本演示给我看").unwrap());
        let revision = goal.revision;
        assert!(!goal.apply_update(GoalUpdate::Working { focus: "read chapter".into(), open_questions: vec![], next_move: "author".into(), items: None }, "t1", "把这一章富文本演示给我看").unwrap());
        assert_eq!(goal.revision, revision);
        goal.apply_update(GoalUpdate::Refine { interpretation: "Show the chapter as a rich page".into(), requirements: vec![page("page", "t1")] }, "t1", "把这一章富文本演示给我看").unwrap();
        assert!(goal.projection(2, 0, 0).contains("presentation delivery still required"));
        assert!(goal.projection(2, 0, 0).contains("observed passages=2"));
        assert_eq!(goal.requirements[0].verification, GoalVerification::PresentationDelivery);
    }

    #[test]
    fn requirement_revision_needs_current_user_quote() {
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        goal.apply_update(GoalUpdate::Refine { interpretation: "Make a page".into(), requirements: vec![page("page", "t1")] }, "t1", "做网页").unwrap();
        goal.user_message_refs.push("t2".into());
        goal.apply_update(GoalUpdate::Working { focus: "只修引用".into(), open_questions: vec![], next_move: "preview again".into(), items: None }, "t2", "请只修引用").unwrap();
        assert_eq!(goal.requirements[0].verification, GoalVerification::PresentationDelivery);
        let no_basis = GoalUpdate::Revise { basis_turn_id: "t2".into(), basis_quote: "改成文字即可".into(), interpretation: "Text only".into(), requirements: vec![GoalRequirement { id: "text".into(), description: "Text".into(), basis_turn_id: "t2".into(), verification: GoalVerification::Content }] };
        assert!(goal.apply_update(no_basis, "t2", "请只修引用").is_err());
        assert_eq!(goal.requirements[0].verification, GoalVerification::PresentationDelivery);
        let explicit = GoalUpdate::Revise { basis_turn_id: "t2".into(), basis_quote: "改成文字即可".into(), interpretation: "Text only".into(), requirements: vec![GoalRequirement { id: "text".into(), description: "Text".into(), basis_turn_id: "t2".into(), verification: GoalVerification::Content }] };
        assert!(goal.apply_update(explicit, "t2", "改成文字即可").unwrap());
        assert_eq!(goal.requirements[0].verification, GoalVerification::Content);
    }

    #[test]
    fn projection_uses_run_facts_and_never_treats_candidate_as_delivery() {
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        goal.requirements = vec![page("page", "t1")];
        assert!(goal.projection(5, 1, 0).contains("presentation delivery still required"));
        assert!(goal.projection(5, 1, 1).contains("no objective presentation gap recorded"));
        let restored: ResidentGoal = serde_json::from_str(&serde_json::to_string(&goal).unwrap()).unwrap();
        assert_eq!(restored.requirements, goal.requirements);
    }

    fn working(items: Option<serde_json::Value>) -> GoalUpdate {
        let mut value = serde_json::json!({"operation":"working", "focus":"check the whole page",
            "open_questions":["Does the example cover the remaining case?"], "next_move":"preview"});
        if let Some(items) = items { value["items"] = items; }
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn ex13_work_plan_replacement_omission_clear_and_noop_preserve_requirements() {
        use serde_json::json;
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        let requirements = goal.requirements.clone();
        let plan = json!([
            {"id":"prototype","description":"Try the key relationship","status":"completed"},
            {"id":"expand","description":"Cover the full scope","status":"in_progress"},
            {"id":"review","description":"Review and deliver","status":"pending"}
        ]);
        assert!(goal.apply_update(working(Some(plan.clone())), "t1", "做网页").unwrap());
        assert_eq!(goal.revision, 2);
        assert_eq!(serde_json::to_value(&goal.working.items).unwrap(), plan);
        assert!(!goal.apply_update(working(Some(plan.clone())), "t1", "做网页").unwrap());
        assert!(!goal.apply_update(working(None), "t1", "做网页").unwrap());
        assert_eq!(goal.revision, 2);
        // A normal focus/next-move update must not erase the multi-step plan.
        let update = serde_json::from_value(json!({"operation":"working", "focus":"rethink the example", "next_move":"change the representation"})).unwrap();
        assert!(goal.apply_update(update, "t2", "继续").unwrap());
        assert_eq!(serde_json::to_value(&goal.working.items).unwrap(), plan);
        let revised = json!([
            {"id":"review","description":"Recheck the remaining case","status":"in_progress"},
            {"id":"prototype","description":"Replace the example","status":"in_progress"}
        ]);
        assert!(goal.apply_update(working(Some(revised.clone())), "t2", "继续").unwrap());
        assert_eq!(serde_json::to_value(&goal.working.items).unwrap(), revised);
        assert_eq!(goal.requirements, requirements);
        assert_eq!(goal.interpretation, "做网页");
        assert!(goal.apply_update(working(Some(json!([]))), "t2", "继续").unwrap());
        assert!(goal.working.items.is_empty());
        assert!(!goal.apply_update(working(Some(json!([]))), "t2", "继续").unwrap());
        assert_eq!(goal.revision, 5);
    }

    #[test]
    fn ex13_invalid_work_plan_is_rejected_without_partial_update() {
        use serde_json::json;
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        let valid = json!({"id":"review","description":"Review the page","status":"pending"});
        goal.apply_update(working(Some(json!([valid]))), "t1", "做网页").unwrap();
        let before = goal.clone();
        for items in [
            json!([{"id":" ","description":"Review","status":"pending"}]),
            json!([{"id":"review","description":"\n ","status":"pending"}]),
            json!([valid, valid]),
        ] {
            let update = serde_json::from_value(json!({"operation":"working", "focus":"must not replace focus", "next_move":"must not replace next move", "items":items})).unwrap();
            assert!(goal.apply_update(update, "t1", "做网页").is_err());
            assert_eq!(goal, before);
        }
        for invalid in [
            json!({"id":"review","description":"Review","status":"done"}),
            json!({"id":"review","status":"pending"}),
        ] {
            assert!(serde_json::from_value::<GoalUpdate>(json!({"operation":"working","focus":"review","next_move":"preview","items":[invalid]})).is_err());
        }
    }

    #[test]
    fn ex13_legacy_goal_defaults_and_work_plan_roundtrip() {
        use serde_json::json;
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        let mut old = serde_json::to_value(&goal).unwrap();
        old["working"].as_object_mut().unwrap().remove("items");
        assert_eq!(serde_json::from_value::<ResidentGoal>(old.clone()).unwrap(), goal);
        old.as_object_mut().unwrap().remove("working");
        assert_eq!(serde_json::from_value::<ResidentGoal>(old).unwrap(), goal);
        goal.apply_update(working(Some(json!([{"id":"review","description":"Review","status":"in_progress"}]))), "t1", "做网页").unwrap();
        assert_eq!(serde_json::from_str::<ResidentGoal>(&serde_json::to_string(&goal).unwrap()).unwrap(), goal);
    }

    #[test]
    fn ex13_completed_items_do_not_complete_goal_or_satisfy_delivery() {
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "做网页".into());
        let update = working(Some(serde_json::json!([{"id":"all","description":"Make the page","status":"completed"}])));
        goal.apply_update(update.clone(), "t1", "做网页").unwrap();
        assert_eq!(goal.status, GoalStatus::Open);
        assert!(goal.objective_gap(0, 0).is_some());
        assert!(goal.objective_gap(1, 0).is_none());
        let projection = goal.projection(2, 1, 0);
        assert!(projection.contains("\"id\":\"all\""));
        assert!(projection.contains("\"status\":\"completed\""));
        assert!(projection.contains("Does the example cover the remaining case?"));
        assert!(projection.contains("presentation delivery still required"));
        for status in [GoalStatus::Completed, GoalStatus::Cancelled, GoalStatus::Superseded] {
            goal.status = status;
            let before = goal.clone();
            assert!(goal.apply_update(update.clone(), "t2", "继续").is_err());
            assert_eq!(goal, before);
        }
    }

    #[test]
    fn explicit_page_request_and_text_only_request_start_with_their_actual_delivery_type() {
        let page = ResidentGoal::new("g1".into(), "t1".into(), "可以把这一章的内容富文本演示给我看吗".into());
        assert_eq!(page.requirements[0].verification, GoalVerification::PresentationDelivery);
        let text = ResidentGoal::new("g2".into(), "t2".into(), "只用文字概述这一节，不需要演示页".into());
        assert_eq!(text.requirements[0].verification, GoalVerification::Content);
        let mut page = page;
        let content = GoalRequirement { id: "text".into(), description: "只回答文字".into(), basis_turn_id: "t1".into(), verification: GoalVerification::Content };
        assert!(page.apply_update(GoalUpdate::Refine { interpretation: "Text".into(), requirements: vec![content.clone()] }, "t1", "可以把这一章的内容富文本演示给我看吗").is_err());
        page.user_message_refs.push("t3".into());
        assert!(page.apply_update(GoalUpdate::Revise { basis_turn_id: "t3".into(), basis_quote: "只修引用".into(), interpretation: "Text".into(), requirements: vec![content.clone()] }, "t3", "请只修引用").is_err());
        let mut content = content;
        content.basis_turn_id = "t3".into();
        assert!(page.apply_update(GoalUpdate::Revise { basis_turn_id: "t3".into(), basis_quote: "改成文字即可".into(), interpretation: "Text".into(), requirements: vec![content] }, "t3", "改成文字即可").unwrap());
    }
}
