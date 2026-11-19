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
    /// An output selection is authoritative; wording alone is only an initial inference.
    #[serde(default)]
    pub presentation_selection_turn_id: Option<String>,
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

pub(crate) const PLANNED_WORK_GUIDANCE: &str = "Use the complete current request to determine the delivery scope. When the request has several deliverables, a broad comparison or substantial authoring work, begin by recording a few concrete outcomes through goal.update operation=working; do this before substantive work, in the same tool batch as the first independent read when possible. Include the completion basis in important item descriptions. An agent.progress message or a teaching design does not save this execution plan. Do not use generic search/analyze/answer placeholders. An ordinary explanation or local follow-up needs no planning call, even if it requires a search and several passages. The host already tracks tool activity, evidence counts and delivery status; do not repeat those facts through goal.update. Before final selection of a task with an existing work plan, update the completed outcomes and any real remaining gap; retain supported results and stable item IDs at meaningful intermediate stages; combine independent reads and do not add a planning round for every tool action. Record useful conclusions in focus or item descriptions, remaining conditions in open_questions, and the specific next action in next_move. New evidence may reorder, split, merge or reopen work without changing the user's requirements.\nDistinguish source uncertainty from unknown learner understanding. Missing knowledge requires the relevant original text, conditions or calculation; search hits and a plan are not evidence. Unknown understanding requires support appropriate to the actual expression and help already received, not more source reading or a compulsory quiz. Invite a response only when it affects the next teaching decision and fits the request. TeachingDesign organizes learning relationships; Goal items execute the reads, comparisons, calculations, authoring, observations and delivery. Use only ready, applicable coach advice; the current request and new evidence take priority.\nBefore selecting delivery, establish the required facts and conditions, explain the necessary connections and inspect actual visual results. Preserve the complete requested scope, including a whole-book explanation when requested. When work is ready, deliver the exact result through the host's selection and completion contract; unresolved work remains explicit. Completed work items, teacher explanations and successful tools never establish learner mastery.";

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

const PRESENTATION_TERMS: &[&str] = &[
    "富文本演示", "演示页", "网页", "页面", "交互页", "html", "web page", "webpage", "interactive page",
];
const PRESENTATION_VERBS: &[&str] = &[
    "做", "制作", "生成", "创建", "演示", "展示", "修改", "make", "create", "build", "generate", "show", "present", "edit",
];

fn request_clauses(request: &str) -> impl Iterator<Item = &str> {
    request.split(['，', ',', '。', '.', '；', ';', '！', '!', '？', '?', '\n'])
        .flat_map(|clause| clause.split(" but "))
        .flat_map(|clause| clause.split(" and "))
        .flat_map(|clause| clause.split('并'))
        .flat_map(|clause| clause.split("而是"))
}

fn first_phrase(request: &str, phrases: &[&str]) -> Option<usize> {
    phrases.iter().filter_map(|phrase| {
        request.match_indices(*phrase).find(|(index, _)| {
            !phrase.is_ascii()
                || (request[..*index].chars().next_back().map_or(true, |c| !c.is_ascii_alphanumeric())
                    && request[*index + phrase.len()..].chars().next().map_or(true, |c| !c.is_ascii_alphanumeric()))
        }).map(|(index, _)| index)
    }).min()
}

fn text_only_clause(request: &str) -> bool {
    if first_phrase(request, &["只要文字", "只用文字", "改成文字", "不需要演示", "不要演示", "text only", "no page"]).is_some() {
        return true;
    }
    let Some(object) = first_phrase(request, PRESENTATION_TERMS) else { return false; };
    let action = first_phrase(request, PRESENTATION_VERBS).unwrap_or(object);
    first_phrase(&request[..object.max(action)], &[
        "不要", "不需要", "不用", "无需", "不必", "别", "不想", "不要再",
        "do not", "don't", "don’t", "not", "no", "without", "avoid", "skip", "never",
    ]).is_some()
}

fn presentation_requested(request: &str) -> bool {
    // Scope a refusal to its clause so a separate positive request remains deliverable.
    request_clauses(request).any(|clause| {
        let Some(action) = first_phrase(clause, PRESENTATION_VERBS) else { return false; };
        let explanation = first_phrase(clause, &[
            "为什么", "为何", "如何", "怎样", "怎么", "解释", "说明", "讲解", "介绍",
            "演示如何", "展示如何", "演示怎么", "展示怎么",
            "why", "how", "what", "explain", "describe", "tell me", "show me how", "show how",
        ]);
        !text_only_clause(clause) && first_phrase(clause, PRESENTATION_TERMS).is_some()
            && explanation.map_or(true, |explanation| explanation > action)
    })
}

fn explicit_text_only(request: &str) -> bool {
    let request = request.to_lowercase();
    !presentation_requested(&request) && request_clauses(&request).any(text_only_clause)
}

impl ResidentGoal {
    /// Record the reader's explicit output selection, independently of wording.
    pub fn require_presentation(&mut self, turn_id: &str) {
        let already_required = self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery);
        if already_required && self.presentation_selection_turn_id.as_deref() == Some(turn_id) {
            return;
        }
        self.presentation_selection_turn_id = Some(turn_id.into());
        if !already_required {
            self.requirements.push(GoalRequirement {
                id: format!("{turn_id}:presentation"),
                description: "制作并交付本次问题的演示页".into(),
                basis_turn_id: turn_id.into(),
                verification: GoalVerification::PresentationDelivery,
            });
        }
        self.revision += 1;
    }

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
        let presentation = presentation_requested(&request);
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
            presentation_selection_turn_id: None,
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
                if self.presentation_selection_turn_id.is_some()
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
                    && !requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery) {
                    if self.presentation_selection_turn_id.as_deref() == Some(current_turn_id) {
                        return Err("revise cannot remove the current turn's explicit presentation selection".into());
                    }
                    if !explicit_text_only(current_user) {
                        return Err("removing presentation delivery needs an explicit text-only user request".into());
                    }
                }
                self.replace_requirements(interpretation, requirements, current_turn_id)?;
                if !self.requirements.iter().any(|r| r.verification == GoalVerification::PresentationDelivery) {
                    self.presentation_selection_turn_id = None;
                }
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
            "resident_goal.v1\nid={} revision={} status={:?}\nOrigin turn: {}. Current request turn: {}.\nUser task: {}\nRequirements: {}\nActual results in this run: observed passages={observed_passages}, uncommitted presentation candidates={pending_candidates}, delivered presentations={delivered_presentations}.\nSaved result references: {}\nRemaining delivery: {}\nDelivery classification: {}\nWorking focus: {}\nOpen questions: {}\nNext move: {}\nWork items (method/progress only): {}\nThis current Goal revision is authoritative over historical active_goal summaries. Work items may change without changing requirements; completed items do not prove delivery or content completeness. A source receipt or candidate is not a delivered page. An old tool activation or candidate cannot be reused in a new run. Work completion never proves learner understanding.",
            self.id, self.revision, self.status, self.origin_turn_id, self.user_message_refs.last().map(String::as_str).unwrap_or(&self.origin_turn_id), self.interpretation,
            self.requirements.iter().map(|r| format!("{} [{:?}]", r.description, r.verification)).collect::<Vec<_>>().join("; "),
            self.result_refs.join(", "),
            if delivery_required && delivered_presentations == 0 { "presentation delivery still required" } else { "no objective presentation gap recorded" },
            if self.presentation_selection_turn_id.is_some() { "explicit reader output selection; refine must retain presentation delivery" }
                else { "initially inferred from wording; origin-turn refine may correct its verification while preserving every requested outcome" },
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
        old.as_object_mut().unwrap().remove("presentation_selection_turn_id");
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
        page.require_presentation("t1");
        let content = GoalRequirement { id: "text".into(), description: "只回答文字".into(), basis_turn_id: "t1".into(), verification: GoalVerification::Content };
        assert!(page.apply_update(GoalUpdate::Refine { interpretation: "Text".into(), requirements: vec![content.clone()] }, "t1", "可以把这一章的内容富文本演示给我看吗").is_err());
        page.user_message_refs.push("t3".into());
        assert!(page.apply_update(GoalUpdate::Revise { basis_turn_id: "t3".into(), basis_quote: "只修引用".into(), interpretation: "Text".into(), requirements: vec![content.clone()] }, "t3", "请只修引用").is_err());
        let mut content = content;
        content.basis_turn_id = "t3".into();
        assert!(page.apply_update(GoalUpdate::Revise { basis_turn_id: "t3".into(), basis_quote: "改成文字即可".into(), interpretation: "Text".into(), requirements: vec![content] }, "t3", "改成文字即可").unwrap());
    }

    #[test]
    fn exam_requests_that_decline_interactive_pages_have_no_presentation_gap() {
        for request in [
            "根据工作区已经确认的考试日期和范围，给我今天30分钟的复习安排。不要先做诊断，不要制作交互页面；说明先练什么以及为什么。用中文简洁回答。",
            "我刚修改了工作区的考试日期和范围。请按当前设置重新安排今天30分钟，只安排考试范围内的内容。不要先做诊断，不要制作交互页面。",
            "我现在不会这道题，请直接讲这题，不要先给我诊断或先修测验：样例视频有两秒，我的陈述只涉及第一秒，为什么不能把整段视频当作精确证据窗口？请直接用这道题解释。不要制作交互页面。",
            "请实际查这个工作区的原创讲义和样例视频字幕，用中文回答：讲义为什么要求保留原始来源引用？视频字幕说动态图案持续多久，应回看哪个时间窗口？分别给能跳回讲义原文和视频时段的来源引用。只依据文字和字幕，不需要分析画面，也不要制作交互页面。",
            "请直接讲题，不需要生成页面。",
            "无需创建网页，讲清楚计算步骤。",
        ] {
            let goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
            assert_eq!(goal.requirements[0].verification, GoalVerification::Content, "{request}");
            assert_eq!(goal.objective_gap(0, 0), None, "{request}");
        }
    }

    #[test]
    fn declining_one_page_does_not_erase_a_separate_positive_page_request() {
        for request in [
            "请制作交互页面解释这道题。",
            "不要先做诊断，请制作交互页面解释这道题。",
            "不要制作交互页面，改为制作静态网页。",
            "不要演示，给我创建一个网页。",
            "Please create a web page explaining why the text is bold.",
            "Can you build an HTML page showing how this works?",
            "Don't create an interactive page, create a static web page instead.",
            "Do not make an interactive page but create a static web page.",
            "Explain how HTML works and create a web page with an example.",
            "解释 HTML 的用途并创建一个示例网页。",
        ] {
            let goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
            assert_eq!(goal.requirements[0].verification, GoalVerification::PresentationDelivery, "{request}");
            assert!(goal.objective_gap(0, 0).is_some(), "{request}");
            assert!(!explicit_text_only(request), "{request}");
        }
    }

    #[test]
    fn page_negations_and_questions_about_authoring_are_content_requests() {
        for request in [
            "do not create a web page",
            "Don't create a web page; just explain it.",
            "I do not want you to build an HTML page.",
            "Please avoid creating an interactive page.",
            "不要给我创建一个网页。",
            "不必制作任何交互页面。",
            "Why does HTML make this text bold?",
            "How do I create an HTML page?",
            "Please explain how to build a web page.",
            "Show me how to make an interactive page.",
            "解释书里如何创建网页",
            "请说明如何生成 HTML 页面。",
            "网页应该如何创建？",
            "展示如何创建网页的步骤。",
        ] {
            let goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
            assert_eq!(goal.requirements[0].verification, GoalVerification::Content, "{request}");
            assert_eq!(goal.objective_gap(0, 0), None, "{request}");
        }
    }

    fn content_refinement(request: &str, turn: &str) -> GoalUpdate {
        GoalUpdate::Refine {
            interpretation: request.into(),
            requirements: vec![GoalRequirement {
                id: "content".into(), description: request.into(), basis_turn_id: turn.into(),
                verification: GoalVerification::Content,
            }],
        }
    }

    #[test]
    fn wording_inference_can_be_corrected_without_removing_the_requested_content() {
        let request = "Show the HTML that makes this text bold";
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
        assert!(goal.objective_gap(0, 0).is_some(), "the initial wording classification is ambiguous");
        assert!(goal.projection(0, 0, 0).contains("origin-turn refine may correct"));
        assert!(goal.apply_update(content_refinement(request, "t1"), "t1", request).unwrap());
        assert_eq!(goal.requirements[0].description, request);
        assert_eq!(goal.objective_gap(0, 0), None);
        assert!(!goal.apply_update(content_refinement(request, "t1"), "t1", request).unwrap());
        assert!(goal.apply_update(content_refinement(request, "t2"), "t2", request).is_err());
    }

    #[test]
    fn restored_keyword_misclassification_can_be_refined() {
        for request in ["do not create a web page", "Why does HTML make this text bold?", "解释书里如何创建网页"] {
            let goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
            // Persisted goals from the old keyword classifier have no output-selection source.
            let mut stored = serde_json::to_value(goal).unwrap();
            stored.as_object_mut().unwrap().remove("presentation_selection_turn_id");
            stored["requirements"][0]["verification"] = serde_json::json!("presentation_delivery");
            let mut restored: ResidentGoal = serde_json::from_value(stored).unwrap();
            assert!(restored.objective_gap(0, 0).is_some());
            assert!(restored.apply_update(content_refinement(request, "t1"), "t1", request).unwrap());
            assert_eq!(restored.objective_gap(0, 0), None, "{request}");
            assert_eq!(restored.requirements[0].description, request);
        }
    }

    #[test]
    fn explicit_output_selection_survives_storage_and_cannot_be_refined_away() {
        for request in ["解释一下这里", "Please create a web page", "do not create a web page"] {
            let mut goal = ResidentGoal::new("g".into(), "t1".into(), request.into());
            let initial_revision = goal.revision;
            goal.require_presentation("t1");
            assert_eq!(goal.revision, initial_revision + 1);
            let mut goal: ResidentGoal = serde_json::from_value(serde_json::to_value(goal).unwrap()).unwrap();
            let before = goal.clone();
            goal.require_presentation("t1");
            assert_eq!(goal, before, "repeating the same output selection is a no-op");
            assert!(goal.apply_update(content_refinement(request, "t1"), "t1", request).is_err());
            assert_eq!(goal, before);
            assert!(goal.projection(0, 0, 0).contains("explicit reader output selection"));
            let GoalUpdate::Refine { interpretation, requirements } = content_refinement(request, "t1") else { unreachable!() };
            let revise = GoalUpdate::Revise {
                basis_turn_id: "t1".into(), basis_quote: request.into(), interpretation, requirements,
            };
            assert!(goal.apply_update(revise, "t1", request).is_err());
            assert_eq!(goal, before);
            let refusal = "Do not create a web page; text only.";
            let GoalUpdate::Refine { interpretation, requirements } = content_refinement(refusal, "t2") else { unreachable!() };
            let revise = GoalUpdate::Revise {
                basis_turn_id: "t2".into(), basis_quote: refusal.into(), interpretation, requirements,
            };
            assert!(goal.apply_update(revise, "t2", refusal).unwrap());
            assert_eq!(goal.presentation_selection_turn_id, None);
            assert_eq!(goal.objective_gap(0, 0), None);
        }
    }

    #[test]
    fn explicit_page_refusal_can_revise_an_existing_presentation_goal() {
        let mut goal = ResidentGoal::new("g".into(), "t1".into(), "请制作交互页面".into());
        let request = "不要制作交互页面，请直接讲这道题。";
        let update = GoalUpdate::Revise {
            basis_turn_id: "t2".into(),
            basis_quote: "不要制作交互页面".into(),
            interpretation: "直接讲题".into(),
            requirements: vec![GoalRequirement {
                id: "content".into(), description: "讲清楚这道题".into(),
                basis_turn_id: "t2".into(), verification: GoalVerification::Content,
            }],
        };
        assert!(goal.apply_update(update.clone(), "t2", "不要制作交互页面，改为制作静态网页。").is_err());
        assert_eq!(goal.requirements[0].verification, GoalVerification::PresentationDelivery);
        assert!(goal.apply_update(update, "t2", request).unwrap());
        assert_eq!(goal.requirements[0].verification, GoalVerification::Content);
        assert_eq!(goal.objective_gap(0, 0), None);
    }
}
