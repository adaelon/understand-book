//! Reuse an actually delivered independent attempt as a referenced course reply.
use super::*;
use crate::closed_book::{ClosedBookCourseOrigin, ClosedBookPractice};

pub(super) struct ClosedBookCourseResult {
    pub origin: ClosedBookCourseOrigin,
    pub response: String,
    pub expression_refs: Vec<String>,
    pub context: Value,
}

fn event(db: &Connection, scope: &str, id: &str) -> Result<TeachingEvent, ToolError> {
    let raw: Option<String> = db
        .query_row(
            "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
            params![scope, id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    serde_json::from_str(&raw.ok_or_else(|| invalid("闭卷结果引用不存在"))?).map_err(storage)
}

pub(super) fn read_closed_book_result(
    db: &Connection,
    scope: &str,
    result_ref: &str,
    run: &CourseRun,
) -> Result<ClosedBookCourseResult, ToolError> {
    let delivered = event(db, scope, result_ref)?;
    if delivered.kind != TeachingFact::ClosedBookChanged {
        return Err(invalid("请使用已经交付的闭卷完成结果"));
    }
    let practice: ClosedBookPractice =
        serde_json::from_value(delivered.payload).map_err(storage)?;
    if practice.result_ref.as_deref() != Some(result_ref)
        || practice.status != "complete"
        || practice.assisted
        || practice.needs_retry
        || practice.completion_reason.trim().is_empty()
        || practice.learner_quote.trim().is_empty()
        || practice.visible_turn_ids.len() < 2
    {
        return Err(invalid("本轮尚无已交付的独立完成依据"));
    }
    if !practice.course
        || practice.owner_id != run.progress.run_id
        || practice.source_id != run.lesson.summary.source.publication.book_id
        || practice.source_revision != run.lesson.summary.source.source_revision
        || practice
            .goal_revision
            .is_some_and(|revision| revision != run.progress.goal_revision)
    {
        return Err(invalid("闭卷结果不属于当前课程任务"));
    }
    let origin = practice
        .course_origin
        .clone()
        .ok_or_else(|| invalid("这份练习未关联具体课内任务，请回到原任务继续"))?;
    let mut responses = Vec::new();
    let mut expression_refs = Vec::new();
    let mut exchanges = Vec::new();
    for (index, turn) in practice.visible_turn_ids.iter().enumerate() {
        let receipt = event(db, scope, &format!("closed-book:delivery:{turn}"))?;
        let proposal = event(db, scope, &format!("closed-book-proposal:{turn}"))?;
        if receipt.kind != TeachingFact::ClosedBookChanged
            || receipt.payload["id"] != practice.id
            || receipt.payload["attempt"] != practice.attempt
            || receipt.binding.chat_session_id != practice.chat_session_id
            || proposal.kind != TeachingFact::ClosedBookProposed
            || proposal.payload["practice_id"] != practice.id
            || proposal.binding.chat_session_id != practice.chat_session_id
        {
            return Err(invalid("闭卷原话与当前独立尝试不一致"));
        }
        let decision = &proposal.payload["decision"];
        if decision["provides_help"] == true || decision["action"] == "help" {
            return Err(invalid("本次尝试获得过帮助，请先完成新的独立尝试"));
        }
        let response = proposal.payload["response"].as_str().unwrap_or("");
        // The first input requested this question; only later inputs answer it.
        if index > 0 && !response.trim().is_empty() {
            responses.push(response.to_owned());
            expression_refs.push(proposal.event_id.clone());
        }
        exchanges.push(
            json!({"delivery_ref":receipt.event_id,"expression_ref":proposal.event_id,
            "response":if index > 0 { response } else { "" },"teacher":decision["text"],
            "action":decision["action"]}),
        );
    }
    let response = responses.join("\n\n");
    if !response.contains(&practice.learner_quote) || response.trim().is_empty() {
        return Err(invalid("闭卷完成依据没有对应的真实回答"));
    }
    let context = json!({"result_ref":result_ref,"practice_id":practice.id,"attempt":practice.attempt,
        "course_origin":origin,"prior_support_delivery_refs":practice.support_delivery_refs,"target":practice.target,"criteria":practice.criteria,
        "completion_reason":practice.completion_reason,"learner_quote":practice.learner_quote,
        "assisted":practice.assisted,"exchanges":exchanges,"source_expression_refs":expression_refs,
        "assistance_scope":"Only the displayed current attempt is independent. Earlier teaching and earlier attempts remain historical context, not help given during this attempt. Compare this actual problem and reasoning against the original course task before accepting it."});
    Ok(ClosedBookCourseResult {
        origin,
        response,
        expression_refs,
        context,
    })
}
