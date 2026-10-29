//! Unresolved review work within one teaching run. Only a normalized review
//! can close an issue; author edits and source reads supply evidence, not status.

use std::collections::{BTreeMap, BTreeSet};

use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Default)]
pub(crate) struct TeachingReviewIssues {
    open: BTreeMap<String, Value>,
}

impl TeachingReviewIssues {
    pub(crate) fn branch_open_issues(&self, branch: &str) -> Value {
        json!(self.open.values().filter(|issue| issue["branch"] == branch).collect::<Vec<_>>())
    }

    pub(crate) fn all_open_issues(&self) -> Value {
        json!(self.open.values().collect::<Vec<_>>())
    }

    /// Call only after normalization with this branch's actual open issues.
    pub(crate) fn record_branch(&mut self, branch: &str, round: usize, review: &Value) {
        self.apply_updates(review, Some(branch));
        self.record_new_issues(branch, round, review, false);
    }

    /// A visual or assessment review owns the whole delivery, including an
    /// explanation issue left open while the delivery was still plain text.
    pub(crate) fn record_combined(&mut self, round: usize, review: &Value) {
        self.apply_updates(review, None);
        self.record_new_issues("factual", round, review, true);
    }

    fn apply_updates(&mut self, review: &Value, branch: Option<&str>) {
        for update in review["issue_updates"].as_array().into_iter().flatten() {
            if let Some(reference) = update["issue_ref"].as_str() {
                if self.open.get(reference).is_some_and(|issue|
                    branch.is_none_or(|branch| issue["branch"] == branch)) {
                    if update["status"] == "resolved" {
                        self.open.remove(reference);
                    } else if let Some(issue) = self.open.get_mut(reference) {
                        // Reviewers supply the complete current repair direction.
                        // Keep identity and original diagnosis, not stale actions.
                        if issue["kind"] == "finding" {
                            issue["finding"]["required_change"] = update["reason"].clone();
                        } else if issue["kind"] == "request" {
                            issue["request_resolution"]["reason"] = update["reason"].clone();
                        }
                    }
                }
            }
        }
    }

    fn record_new_issues(&mut self, branch: &str, round: usize, review: &Value, combined: bool) {
        // Effective findings also contain carried issues. Allocate identities
        // only for findings first reported by this review.
        for (index, finding) in review["new_findings"].as_array().into_iter().flatten().enumerate() {
            let reference = format!("{branch}:{round}:{index}");
            self.open.insert(reference.clone(), json!({"issue_ref":reference,"branch":branch,
                "kind":"finding","finding":finding}));
        }
        if review["request_resolution"]["status"] == "unresolved" {
            if let Some(issue) = self.open.values_mut().find(|issue| issue["kind"] == "request"
                && (combined || issue["branch"] == branch)) {
                // A review can discover another request gap while the old one
                // stays open. The normalized resolution carries both the new
                // gap and current unresolved updates under the stable identity.
                issue["request_resolution"] = review["request_resolution"].clone();
            } else {
                let reference = format!("{branch}:{round}:request");
                self.open.insert(reference.clone(), json!({"issue_ref":reference,"branch":branch,
                    "kind":"request","request_resolution":review["request_resolution"]}));
            }
        }
    }
}

pub(crate) fn branch_issues(issues: &Value, branch: &str) -> Value {
    json!(issues.as_array().into_iter().flatten()
        .filter(|issue| issue["branch"] == branch).collect::<Vec<_>>())
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum IssueStatus { Resolved, Unresolved }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueUpdate {
    issue_ref: String,
    status: IssueStatus,
    reason: String,
    #[serde(default)]
    answer_quotes: Vec<String>,
    #[serde(default)]
    source_quotes: Vec<Value>,
}

/// Preserve newly reported findings separately, then carry unresolved findings
/// into the ordinary decision normalizer. Quotation validation uses the same
/// actual delivery and source checks as the other review fields.
pub(crate) fn normalize_issue_updates(
    review: &mut Value,
    packet: &Value,
    answer_texts: &[&str],
    source_supported: impl Fn(&Value) -> bool,
) -> Result<(), ToolError> {
    let open = match packet.get("open_issues") {
        None | Some(Value::Null) => &[][..],
        Some(Value::Array(issues)) => issues.as_slice(),
        _ => return Err(memory::teaching::invalid("open_issues 必须是宿主提供的问题数组")),
    };
    let updates: Vec<IssueUpdate> = match review.get("issue_updates") {
        None if open.is_empty() => Vec::new(),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|error| memory::teaching::invalid(format!("issue_updates 需要有效的问题处置：{error}")))?,
        None => return Err(memory::teaching::invalid("必须用 issue_updates 逐一处置每个 open_issues，不能省略旧问题")),
    };
    let mut seen = BTreeSet::new();
    let mut unresolved = Vec::new();
    for update in &updates {
        let Some(issue) = open.iter().find(|issue| issue["issue_ref"].as_str() == Some(&update.issue_ref)) else {
            return Err(memory::teaching::invalid(format!("issue_updates 引用了未知问题：{}", update.issue_ref)));
        };
        if !seen.insert(update.issue_ref.as_str()) {
            return Err(memory::teaching::invalid(format!("issue_updates 重复处置问题：{}", update.issue_ref)));
        }
        if update.reason.trim().is_empty() {
            return Err(memory::teaching::invalid(format!("问题 {} 的处置必须说明具体理由", update.issue_ref)));
        }
        if update.answer_quotes.iter().any(|quote| quote.trim().is_empty()
            || !answer_texts.iter().any(|text| text.contains(quote))) {
            return Err(memory::teaching::invalid("issue_updates.answer_quotes 必须逐字来自当前实际正文或本轮已交付正文"));
        }
        if update.source_quotes.iter().any(|citation| !source_supported(citation)) {
            return Err(memory::teaching::invalid("issue_updates.source_quotes 必须引用指定来源版本与位置的实际已读原文"));
        }
        if matches!(update.status, IssueStatus::Unresolved) {
            unresolved.push((issue, update));
        }
    }
    let missing: Vec<_> = open.iter().filter_map(|issue| issue["issue_ref"].as_str())
        .filter(|reference| !seen.contains(reference)).collect();
    if !missing.is_empty() {
        return Err(memory::teaching::invalid(format!("issue_updates 尚未处置这些旧问题：{}", missing.join(", "))));
    }
    let findings = review["findings"].as_array().ok_or_else(|| memory::teaching::invalid("教学审阅需要 findings 数组"))?.clone();
    review["new_findings"] = json!(findings);
    if review.get("issue_updates").is_none() { review["issue_updates"] = json!([]); }
    for (issue, update) in unresolved {
        if issue["kind"] == "finding" {
            let mut finding = issue["finding"].clone();
            finding["required_change"] = json!(update.reason);
            review["findings"].as_array_mut().unwrap().push(finding);
        } else if issue["kind"] == "request" {
            review["request_resolution"]["status"] = json!("unresolved");
            let current = review["request_resolution"]["reason"].as_str().unwrap_or_default();
            review["request_resolution"]["reason"] = json!(format!("{current}\n此前必要连接仍未解决（{}）：{}",
                update.issue_ref, update.reason));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding() -> Value {
        json!({"rule_id":"R3","location":"The delay is five minutes.",
            "evidence":"The delay passage has not been read.","required_change":"Read the delay passage."})
    }

    fn review() -> Value {
        json!({"decision":"accept","revision":null,"request_resolution":{"status":"answered",
            "question_quote":"Explain the delay.","requested_relation":"How delayed feedback affects adjustment.",
            "delivered_relation":"The response takes five minutes.","answer_quotes":["The delay is five minutes."],
            "reason":"The delay is explained."},"findings":[],"learner_claims":[]})
    }

    fn packet() -> Value {
        json!({"original_request":"Explain the delay.","proposed_delivery":{"text":"The delay is five minutes."},
            "open_issues":[{"issue_ref":"factual:1:0","branch":"factual","kind":"finding","finding":finding()}],
            "current_source_evidence":[{"source_id":"book","source_revision":"v1", "segments":[
                {"lid":"1.1","text":"The response is delayed by five minutes."}]}]})
    }

    #[test]
    fn issue_updates_require_each_known_issue_once() {
        let update = json!({"issue_ref":"factual:1:0","status":"resolved","reason":"The missing source was read."});
        for updates in [Value::Null, json!([]), json!([update.clone(), update.clone()]),
            json!([{"issue_ref":"factual:9:0","status":"resolved","reason":"Unknown."}]),
            json!([{"issue_ref":"factual:1:0","status":"resolved","reason":" "}])] {
            let mut value = review();
            if !updates.is_null() { value["issue_updates"] = updates; }
            assert!(crate::tutor::normalize_delivery_review(&mut value, &packet()).is_err());
        }
    }

    #[test]
    fn unchanged_text_can_close_an_issue_with_actual_source_quotes() {
        let mut value = review();
        value["issue_updates"] = json!([{"issue_ref":"factual:1:0","status":"resolved",
            "reason":"The newly read source supports the unchanged sentence.",
            "answer_quotes":["The delay is five minutes."],"source_quotes":[{"source_id":"book",
                "source_revision":"v1","lid":"1.1","quote":"delayed by five minutes"}]}]);
        crate::tutor::normalize_delivery_review(&mut value, &packet()).unwrap();
        assert_eq!(value["decision"], "accept");
        assert_eq!(value["new_findings"], json!([]));
        for field in ["answer_quotes", "source_quotes"] {
            let mut invalid = review();
            invalid["issue_updates"] = value["issue_updates"].clone();
            if field == "answer_quotes" { invalid["issue_updates"][0][field] = json!(["Invented public explanation"]); }
            else { invalid["issue_updates"][0][field][0]["quote"] = json!("Invented source"); }
            assert!(crate::tutor::normalize_delivery_review(&mut invalid, &packet()).is_err());
        }
    }

    #[test]
    fn unresolved_findings_force_revision_without_becoming_new_issues() {
        let mut value = review();
        value["issue_updates"] = json!([{"issue_ref":"factual:1:0","status":"unresolved","reason":"Still unread."}]);
        crate::tutor::normalize_delivery_review(&mut value, &packet()).unwrap();
        assert_eq!(value["decision"], "revise");
        assert_eq!(value["new_findings"], json!([]));
        assert_eq!(value["findings"][0]["location"], finding()["location"]);
        let mut ledger = TeachingReviewIssues::default();
        ledger.record_branch("factual", 1, &json!({"new_findings":[finding()]}));
        ledger.record_branch("factual", 2, &value);
        assert_eq!(ledger.branch_open_issues("factual").as_array().unwrap().len(), 1);
        assert_eq!(ledger.all_open_issues()[0]["issue_ref"], "factual:1:0");
    }

    #[test]
    fn unresolved_updates_replace_obsolete_actions_without_losing_other_issues_or_history() {
        let mut ledger = TeachingReviewIssues::default();
        let original = json!({"new_findings":[finding()]});
        ledger.record_branch("factual", 1, &original);
        let mut explanation = review();
        explanation["request_resolution"]["status"] = json!("unresolved");
        explanation["request_resolution"]["reason"] = json!("Keep the causal connection and explain the delayed observation.");
        ledger.record_branch("explanation", 1, &explanation);
        let untouched = ledger.branch_open_issues("explanation");
        for (round, direction) in [
            "The duration is now supported. Read 1.2 through 1.4 to verify the complete response sequence.",
            "The response sequence is now supported. Read the continuation at 1.4 to establish the remaining adjustment condition.",
        ].into_iter().enumerate() {
            let mut input = packet();
            input["open_issues"] = ledger.branch_open_issues("factual");
            let mut value = review();
            value["issue_updates"] = json!([{"issue_ref":"factual:1:0","status":"unresolved","reason":direction}]);
            crate::tutor::normalize_delivery_review(&mut value, &input).unwrap();
            assert_eq!(value["decision"], "revise");
            assert_eq!(value["findings"][0]["required_change"], direction);
            ledger.record_branch("factual", round + 2, &value);
            let current = ledger.branch_open_issues("factual");
            assert_eq!(current.as_array().unwrap().len(), 1);
            assert_eq!(current[0]["issue_ref"], "factual:1:0");
            assert_eq!(current[0]["finding"]["required_change"], direction);
            assert_eq!(current[0]["finding"]["evidence"], finding()["evidence"]);
            assert!(current[0].get("last_reason").is_none());
            assert_eq!(ledger.branch_open_issues("explanation"), untouched, "a skipped branch keeps its complete remaining work");
        }
        assert_eq!(original["new_findings"][0], finding(), "historical review input remains intact");
    }

    #[test]
    fn request_issues_stay_stable_and_an_unexecuted_branch_stays_open() {
        let mut ledger = TeachingReviewIssues::default();
        let mut explanation = review();
        explanation["request_resolution"]["status"] = json!("unresolved");
        explanation["new_findings"] = json!([]);
        explanation["request_resolution"]["reason"] = json!("OLD_REQUEST_ACTION: explain the original timing example.");
        ledger.record_branch("explanation", 1, &explanation);
        ledger.record_branch("factual", 2, &json!({"new_findings":[finding()]}));
        let mut input = packet();
        input["open_issues"] = ledger.branch_open_issues("explanation");
        let mut value = review();
        value["issue_updates"] = json!([{"issue_ref":"explanation:1:request","status":"unresolved",
            "reason":"Deleting the example has not supplied the required connection."}]);
        crate::tutor::normalize_delivery_review(&mut value, &input).unwrap();
        assert_eq!(value["decision"], "revise");
        assert!(value["findings"].as_array().unwrap().is_empty());
        ledger.record_branch("explanation", 3, &value);
        assert_eq!(ledger.branch_open_issues("explanation")[0]["issue_ref"], "explanation:1:request");
        assert_eq!(ledger.branch_open_issues("explanation")[0]["request_resolution"], value["request_resolution"]);
        assert!(!value["request_resolution"]["reason"].as_str().unwrap().contains("OLD_REQUEST_ACTION"));
        let mut factual = review();
        factual["issue_updates"] = json!([{"issue_ref":"factual:2:0","status":"resolved","reason":"Read."}]);
        input["open_issues"] = ledger.branch_open_issues("factual");
        crate::tutor::normalize_delivery_review(&mut factual, &input).unwrap();
        ledger.record_branch("factual", 4, &factual);
        assert_eq!(ledger.all_open_issues().as_array().unwrap().len(), 1);
        assert_eq!(ledger.all_open_issues()[0]["branch"], "explanation");
    }

    #[test]
    fn new_request_gap_joins_the_stable_unresolved_issue_without_old_action_text() {
        let mut ledger = TeachingReviewIssues::default();
        let mut first = review();
        first["request_resolution"]["status"] = json!("unresolved");
        first["request_resolution"]["reason"] = json!("OLD_ACTION: explain the delay example.");
        ledger.record_branch("explanation", 1, &first);
        let mut input = packet();
        input["open_issues"] = ledger.branch_open_issues("explanation");
        let mut next = review();
        next["request_resolution"]["status"] = json!("unresolved");
        next["request_resolution"]["reason"] = json!("NEW_GAP: also restore how the full chapter sequence develops the method.");
        next["issue_updates"] = json!([{"issue_ref":"explanation:1:request","status":"unresolved",
            "reason":"CURRENT_GAP: explain why delayed observation makes the next adjustment excessive."}]);
        crate::tutor::normalize_delivery_review(&mut next, &input).unwrap();
        ledger.record_branch("explanation", 2, &next);
        let open = ledger.branch_open_issues("explanation");
        assert_eq!(open.as_array().unwrap().len(), 1);
        assert_eq!(open[0]["issue_ref"], "explanation:1:request");
        assert_eq!(open[0]["request_resolution"], next["request_resolution"]);
        let reason = open[0]["request_resolution"]["reason"].as_str().unwrap();
        assert!(reason.contains("NEW_GAP") && reason.contains("CURRENT_GAP"));
        assert!(!reason.contains("OLD_ACTION"));
        assert_eq!(first["request_resolution"]["reason"], "OLD_ACTION: explain the delay example.");
    }

    #[test]
    fn combined_review_resolves_prior_explanation_issues_after_a_mode_change() {
        let mut ledger = TeachingReviewIssues::default();
        let mut explanation = review();
        explanation["request_resolution"]["status"] = json!("unresolved");
        explanation["new_findings"] = json!([]);
        ledger.record_branch("explanation", 1, &explanation);
        ledger.record_branch("factual", 1, &json!({"new_findings":[finding()]}));

        let mut combined = review();
        combined["issue_updates"] = json!([
            {"issue_ref":"explanation:1:request","status":"unresolved","reason":"The page still omits the connection."},
            {"issue_ref":"factual:1:0","status":"resolved","reason":"The source was read."}
        ]);
        let mut input = packet();
        input["open_issues"] = ledger.all_open_issues();
        crate::tutor::normalize_delivery_review(&mut combined, &input).unwrap();
        ledger.record_combined(2, &combined);
        assert_eq!(combined["decision"], "revise");
        assert_eq!(ledger.all_open_issues().as_array().unwrap().len(), 1);
        assert_eq!(ledger.all_open_issues()[0]["issue_ref"], "explanation:1:request");

        let mut repaired = review();
        repaired["findings"] = json!([finding()]);
        repaired["issue_updates"] = json!([{"issue_ref":"explanation:1:request","status":"resolved",
            "reason":"The actual page now explains the connection."}]);
        input["open_issues"] = ledger.all_open_issues();
        crate::tutor::normalize_delivery_review(&mut repaired, &input).unwrap();
        ledger.record_combined(3, &repaired);
        assert!(ledger.branch_open_issues("explanation").as_array().unwrap().is_empty());
        assert_eq!(ledger.all_open_issues().as_array().unwrap().len(), 1);
        assert_eq!(ledger.all_open_issues()[0]["issue_ref"], "factual:3:0");
    }
}
