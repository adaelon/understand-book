//! Teacher-owned work and review history. The event log is authoritative; this
//! bounded view is disposable and never contributes to the learner projection.
use crate::{learning::{storage, LearningStore}, teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact}};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

const RECENT_TURNS: u32 = 4;

fn excerpt(value: &Value, limit: usize) -> Value {
    let text = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
    json!({"text":text.chars().take(limit).collect::<String>(),"truncated":text.chars().count()>limit})
}

fn change_summary(value: &Value) -> Value {
    json!(value.as_array().into_iter().flatten().take(6).map(|change| json!({
        "path":change["path"],"before":excerpt(&change["before"],400),"after":excerpt(&change["after"],400)
    })).collect::<Vec<_>>())
}

impl LearningStore {
    /// Called only for a structurally valid review. Full source passages,
    /// candidate text and verdict stay available by their exact event reference.
    pub fn append_agent_work_review(&mut self, binding: &TeachingBinding, binding_ref: &str,
        record: Value, now: &str) -> Result<TeachingEvent, ToolError> {
        record["round"].as_u64().filter(|round| *round > 0)
            .ok_or_else(|| invalid("Agent review needs its actual positive round"))?;
        let review = &record["review"];
        if !(review["decision"] == "accept" && review.get("revision").is_some_and(Value::is_null)
            || review["decision"] == "revise" && review["revision"].as_str().is_some_and(|text| !text.trim().is_empty())) {
            return Err(invalid("Agent work requires a valid review decision"));
        }
        if !record["original_request"].is_string() || !record["proposed_delivery"].is_object()
            || !record["source_evidence"].is_array() {
            return Err(invalid("Agent work needs the actual request, proposed delivery and read evidence"));
        }
        // A resumed run can restart its local review round. Allocate the event
        // ordinal from persisted history, while an exact retry reuses its receipt.
        let previous:Option<String>=self.connection.query_row("SELECT event FROM teaching_trace
            WHERE learning_workspace_id=?1 AND session_id=?2 AND json_extract(event,'$.kind')='agent_work_reviewed'
              AND json_extract(event,'$.binding.turn_id')=?3 ORDER BY seq DESC LIMIT 1",
            params![self.workspace_scope,binding.tutor_session_id,binding.turn_id],|row|row.get(0)).optional().map_err(storage)?;
        let previous=previous.map(|text|serde_json::from_str::<TeachingEvent>(&text).map_err(storage)).transpose()?;
        if let Some(previous)=&previous {
            if previous.binding==*binding && previous.payload==record { return Ok(previous.clone()); }
        }
        let ordinal=previous.as_ref().and_then(|event|event.event_id.rsplit(':').next()?.parse::<u64>().ok()).unwrap_or(0)+1;
        self.append_teaching(&TeachingEvent {
            event_id:format!("agent-work:{}:{ordinal}",binding.turn_id),binding:binding.clone(),
            kind:TeachingFact::AgentWorkReviewed,causal_refs:vec![binding_ref.to_owned()],payload:record,occurred_at:now.into(),
        })
    }

    /// Scope is the same private teaching session and source version as the
    /// next turn. Review acceptance remains distinct from actual delivery.
    pub fn agent_working_state(&self, binding: &TeachingBinding) -> Result<Value, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace
            WHERE learning_workspace_id=?1 AND session_id=?2
              AND json_extract(event,'$.kind')='agent_work_reviewed'
              AND json_extract(event,'$.binding.source_id')=?3
              AND json_extract(event,'$.binding.source_revision')=?4
              AND json_extract(event,'$.binding.turn_id') IN (
                SELECT json_extract(event,'$.binding.turn_id') FROM teaching_trace
                WHERE learning_workspace_id=?1 AND session_id=?2
                  AND json_extract(event,'$.kind')='agent_work_reviewed'
                  AND json_extract(event,'$.binding.source_id')=?3
                  AND json_extract(event,'$.binding.source_revision')=?4
                GROUP BY json_extract(event,'$.binding.turn_id') ORDER BY max(seq) DESC LIMIT ?5)
            ORDER BY seq").map_err(storage)?;
        let events:Vec<TeachingEvent> = query.query_map(params![self.workspace_scope,binding.tutor_session_id,
            binding.source_id,binding.source_revision,RECENT_TURNS],|row|row.get::<_,String>(0)).map_err(storage)?
            .map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect::<Result<_,_>>()?;
        let mut latest_per_turn = Vec::<&TeachingEvent>::new();
        for event in &events {
            latest_per_turn.retain(|previous|previous.binding.turn_id != event.binding.turn_id);
            latest_per_turn.push(event);
        }
        let mut work = Vec::new();
        for event in &latest_per_turn {
            let deliveries = self.teaching_deliveries(&event.binding.chat_session_id,&event.binding.turn_id)?;
            work.push(json!({"review_ref":event.event_id,"turn_id":event.binding.turn_id,"round":event.payload["round"],
                "request":excerpt(&event.payload["original_request"],600),
                "design_ref":event.payload["current_design"]["design_ref"],
                "review_status":if event.payload["review"]["decision"]=="accept" {"accepted_by_review"} else {"revision_requested"},
                "request_resolution":event.payload["review"]["request_resolution"],
                "agent_work":event.payload["review"]["agent_work"],
                "proposed_text":excerpt(&event.payload["proposed_delivery"]["text"],1000),
                "delivered_refs":deliveries.iter().filter(|delivery|delivery.binding.tutor_session_id==binding.tutor_session_id
                    && delivery.binding.source_id==binding.source_id && delivery.binding.source_revision==binding.source_revision)
                    .map(|delivery|&delivery.event_id).collect::<Vec<_>>() }));
        }
        let mut corrections = Vec::new();
        for (index,event) in events.iter().enumerate().filter(|(_,event)|event.payload["review"]["decision"]=="revise") {
            let later:Vec<_>=events[index+1..].iter().filter(|next|next.binding.turn_id==event.binding.turn_id).collect();
            let accepted=later.iter().rev().find(|next|next.payload["review"]["decision"]=="accept");
            corrections.push(json!({"review_ref":event.event_id,"turn_id":event.binding.turn_id,
                "revision":excerpt(&event.payload["review"]["revision"],1400),
                "rules":event.payload["review"]["findings"].as_array().into_iter().flatten()
                    .filter_map(|finding|finding["rule_id"].as_str()).collect::<Vec<_>>(),
                "request_resolution":event.payload["review"]["request_resolution"],
                "review_status":if accepted.is_some(){"accepted_by_review"}else{"revision_requested"},
                "accepted_review_ref":accepted.map(|next|&next.event_id),
                "follow_up_changes":later.iter().filter(|next|next.payload["changes_since_previous_review"].as_array().is_some_and(|changes|!changes.is_empty()))
                    .take(3).map(|next|json!({"review_ref":next.event_id,"changes":change_summary(&next.payload["changes_since_previous_review"])})).collect::<Vec<_>>() }));
        }
        if corrections.len()>8 { corrections.drain(..corrections.len()-8); }
        let current_work=work.pop().unwrap_or(Value::Null);
        Ok(json!({"current_work":current_work,"previous_work":work,"corrections":corrections,
            "meaning":"Teacher work and review observations only. accepted_by_review means that review accepted the candidate; it is not proof of factual truth, actual delivery, or learner understanding. Full records and evidence are readable with tutor.step trace using review_ref. Older records may be outside this bounded view."}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> TeachingBinding {
        TeachingBinding {tutor_session_id:"tutor".into(),session_revision:1,control_revision:1,source_id:"book".into(),
            source_revision:"v1".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()}
    }
    fn start(store:&mut LearningStore,binding:&TeachingBinding) {
        store.append_teaching(&TeachingEvent{event_id:format!("binding:{}",binding.turn_id),binding:binding.clone(),
            kind:TeachingFact::TurnBound,causal_refs:vec![],payload:json!({}),occurred_at:"now".into()}).unwrap();
    }
    fn record(round:u64,accept:bool) -> Value {
        json!({"round":round,"original_request":"How does stopped time affect average speed?",
            "proposed_delivery":{"text":if accept {"Use all elapsed time."} else {"Use moving time only."},"moves":[]},
            "source_evidence":[{"lid":"1.1","text":"Average speed divides distance by all elapsed time."}],
            "review":{"decision":if accept {"accept"}else{"revise"},"revision":if accept {Value::Null}else{json!("Stopped time is missing from the denominator.")},
                "request_resolution":{"status":if accept {"answered"}else{"unresolved"},
                    "question_quote":"How does stopped time affect average speed?",
                    "reason":if accept {"The explanation includes stopped time in all elapsed time."}else{"The explanation omits stopped time; include it in the denominator."}},
                "findings":if accept {json!([])}else{json!([{"rule_id":"R2","location":"delivery.text",
                    "evidence":"The draft uses moving time only, but the source divides distance by all elapsed time.",
                    "required_change":"Include stopped time in the elapsed-time denominator."}])},
                "agent_work":{"understanding":[{"statement":"Stops count in elapsed time.","sources":[{"lid":"1.1","quote":"all elapsed time"}]}],"open_questions":[]}},
            "changes_since_previous_review":if accept {json!([{"path":"delivery.text","before":"Use moving time only.","after":"Use all elapsed time."}])}else{json!([])}})
    }

    #[test]
    fn agent_work_restart_keeps_correction_and_actual_changes_without_learner_evidence() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("learning.db");let binding=binding();
        let mut store=LearningStore::open(&path,false).unwrap();start(&mut store,&binding);
        let rejected=store.append_agent_work_review(&binding,"binding:turn",record(1,false),"now").unwrap();
        assert!(rejected.learner_expression().is_none());
        assert!(store.learner_expression_fact(&binding,&rejected.event_id).is_err());
        assert_eq!(store.agent_working_state(&binding).unwrap()["corrections"][0]["review_status"],"revision_requested");
        let accepted=store.append_agent_work_review(&binding,"binding:turn",record(2,true),"later").unwrap();
        assert_eq!(store.append_agent_work_review(&binding,"binding:turn",record(2,true),"retry").unwrap(),accepted);
        store.rebuild().unwrap();drop(store);
        let mut store=LearningStore::open(&path,false).unwrap();let state=store.agent_working_state(&binding).unwrap();
        assert_eq!(state["current_work"]["review_status"],"accepted_by_review");
        assert_eq!(state["current_work"]["request_resolution"]["status"],"answered");
        assert_eq!(state["current_work"]["delivered_refs"],json!([]));
        assert_eq!(state["corrections"][0]["review_ref"],rejected.event_id);
        assert_eq!(state["corrections"][0]["rules"],json!(["R2"]));
        assert_eq!(state["corrections"][0]["request_resolution"]["status"],"unresolved");
        assert_eq!(state["corrections"][0]["accepted_review_ref"],accepted.event_id);
        assert_eq!(state["corrections"][0]["follow_up_changes"][0]["changes"][0]["after"]["text"],"Use all elapsed time.");
        assert_eq!(store.teaching_event(&rejected.event_id).unwrap().payload,record(1,false));
        assert_eq!(store.learner_projection("book").unwrap().1,0);
        assert!(store.teaching_recent("tutor",20).unwrap().is_empty());
        let resumed=store.append_agent_work_review(&binding,"binding:turn",record(1,false),"resumed").unwrap();
        assert_eq!(resumed.event_id,"agent-work:turn:3","a resumed run's local round cannot overwrite prior work");
        assert_eq!(store.agent_working_state(&binding).unwrap()["current_work"]["review_status"],"revision_requested");
    }

    #[test]
    fn agent_work_stays_in_workspace_session_and_source_revision() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("learning.db");let binding=binding();
        let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("A"));start(&mut store,&binding);
        store.append_agent_work_review(&binding,"binding:turn",record(1,false),"now").unwrap();
        assert!(!store.agent_working_state(&binding).unwrap()["current_work"].is_null());
        for field in ["session","source","revision"] {
            let mut other=binding.clone();match field {"session"=>other.tutor_session_id="other".into(),"source"=>other.source_id="other".into(),_=>other.source_revision="v2".into()};
            assert!(store.agent_working_state(&other).unwrap()["current_work"].is_null());
        }
        let other=LearningStore::open(&path,false).unwrap().with_workspace(Some("B"));
        assert!(other.agent_working_state(&binding).unwrap()["current_work"].is_null());
    }
}
