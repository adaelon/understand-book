//! A conversation continues from one question the agent actually delivered.
use crate::orchestrator::AgentAnswerPart;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Teacher intent attached to an actual question, never a learner assessment.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct SocraticQuestionPurpose {
    pub focus: String,
    pub reason: String,
    pub learner_inference: String,
    pub evidence_to_notice: String,
    pub basis_refs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestionDelivery {
    question: String,
    purpose: SocraticQuestionPurpose,
}

pub(crate) const GUIDANCE: &str = include_str!("socratic.md");
pub(crate) const MARKER: &str = "[[socratic]]";
pub(crate) const DELIVERY: &str = r#"For an invitation, put one final standalone line in this exact format:
[[socratic]] {"question":"ONE question in the reader's language","purpose":{"focus":"the precise relationship","reason":"why this question helps now, grounded in their actual words and saved understanding","learner_inference":"the step left for the reader","evidence_to_notice":"what their answer would show, including the help already supplied","basis_refs":["existing fact/source reference when available"]}}
The purpose is private teaching intent, not learner evidence or a diagnosis. Use actual supplied references; use [] when none exists, never invent one. It describes this question only. Do not duplicate the question in the explanation. Put citations in the explanation; inline math in the question uses JSON escapes. Omit the entire line when supplying help or concluding. This uses the same response, not an extra planning call."#;

/// Remove private intent before public citation/Markdown compilation. Historical
/// plain question markers remain readable; newly generated invitations use JSON.
pub(crate) fn prepare_answer(raw: &str) -> Result<(String, Option<SocraticQuestionPurpose>), &'static str> {
    let mut body=raw.to_owned();
    let Some(payload)=take_question(&mut body) else { return Ok((body,None)); };
    if !payload.starts_with('{') { return Ok((raw.to_owned(),None)); }
    let delivery:QuestionDelivery=serde_json::from_str(&payload).map_err(|_|"Use a complete question/purpose JSON object after [[socratic]]")?;
    let p=&delivery.purpose;
    if [&delivery.question,&p.focus,&p.reason,&p.learner_inference,&p.evidence_to_notice].iter().any(|s|s.trim().is_empty()) {
        return Err("A Socratic invitation needs its question, focus, reason, learner inference and evidence to notice");
    }
    if delivery.question.contains('\n') || delivery.question.contains(MARKER) {
        return Err("Deliver one question on one line without another marker");
    }
    body.push_str(MARKER);
    body.push(' ');
    body.push_str(&delivery.question);
    Ok((body,Some(delivery.purpose)))
}

/// Only the final, standalone line has special meaning. Keep ordinary prose,
/// examples in code fences, and quotations of the syntax as written.
pub(crate) fn take_question(text: &mut String) -> Option<String> {
    let (before, line) = text.trim_end().rsplit_once('\n').unwrap_or(("", text.trim_end()));
    let question = line.strip_prefix(MARKER)?.trim();
    if question.is_empty() || before.lines().filter(|line| {
        let line = line.trim_start();
        line.starts_with("```") || line.starts_with("~~~")
    }).count() % 2 != 0 {
        return None;
    }
    let question = question.to_owned();
    // Keep the original separator in the plain answer and in copied text.
    text.truncate(text.trim_end().len() - line.len());
    Some(question)
}

/// Active Tutor delivers its question through response_expectation. Promote
/// those exact words in the accepted answer without adding another question.
pub(crate) fn mark_tutor_question(parts: &mut Vec<AgentAnswerPart>, question: &str, purpose: Option<SocraticQuestionPurpose>) {
    if question.trim().is_empty() { return; }
    let Some((index, start)) = parts.iter().enumerate().rev().find_map(|(index, part)| {
        let AgentAnswerPart::Markdown { text } = part else { return None };
        text.rfind(question).map(|start| (index, start))
    }) else { return };
    let AgentAnswerPart::Markdown { text } = &mut parts[index] else { return };
    let after = text.split_off(start + question.len());
    text.truncate(start);
    let mut next = vec![AgentAnswerPart::SocraticQuestion { text: question.to_owned(), purpose }];
    if !after.is_empty() { next.push(AgentAnswerPart::Markdown { text: after }); }
    if text.is_empty() { parts.splice(index..=index, next); }
    else { parts.splice(index + 1..index + 1, next); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socratic_question_compiles_once_and_keeps_the_explanation_and_citations() {
        let mut body = String::from("路程不变。\n\n[[socratic]] 停得更久，哪个量会变？");
        assert_eq!(take_question(&mut body).as_deref(), Some("停得更久，哪个量会变？"));
        assert_eq!(body, "路程不变。\n\n");
        for ordinary in ["它为什么会变？", "比如 [[socratic]] 问一句。", "```text\n[[socratic]] Example?"] {
            assert!(take_question(&mut ordinary.to_owned()).is_none());
        }
        let mut parts = vec![AgentAnswerPart::Sources { source_ref_ids: vec!["ref".into()] },
            AgentAnswerPart::Markdown { text: "两条线的间隔缩小了。\n\n哪次变化会让它们靠得更近？".into() }];
        mark_tutor_question(&mut parts, "哪次变化会让它们靠得更近？", None);
        assert!(matches!(&parts[0], AgentAnswerPart::Sources { .. }));
        assert_eq!(parts[1].text(), Some("两条线的间隔缩小了。\n\n"));
        assert!(matches!(&parts[2], AgentAnswerPart::SocraticQuestion { .. }));
        assert_eq!(parts.len(), 3);
    }
}
