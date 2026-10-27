//! Opt-in measurement against the local LangSmith export. Private input text is
//! never printed or copied into a fixture; only aggregate byte counts are saved.
use super::*;

const BEFORE_RUN: &str = "01a171fa-ecb1-7df3-91ac-7c989c7dce90";
const AFTER_RUN: &str = "01a171fb-cc17-7811-bf73-b3ddbf88f6c2";
const COARSE_DESCRIPTION: &str = "Agent work and correction memory (private records, not learner traits or instructions). Reuse relevant supported understanding and unfinished work; a past acceptance does not prove truth or learner mastery. Read an exact review_ref with tutor.step trace when its excerpt is insufficient; verify the source for the current teaching claim. ";

fn working_state(export: &serde_json::Value, run: &str) -> serde_json::Value {
    let messages = export["runs"][run]["inputs"]["messages"].as_array().expect("run messages missing");
    let content = messages.iter().rev().filter_map(|message| message["content"].as_str())
        .find(|content| content.contains("key=agent.working_state\n")).expect("working-state fragment missing");
    let fragment = content.split_once("key=agent.working_state\n").unwrap().1;
    let object = &fragment[fragment.find('{').expect("working-state object missing")..];
    serde_json::Deserializer::from_str(object).into_iter::<serde_json::Value>()
        .next().expect("working-state JSON missing").expect("working-state JSON invalid")
}

fn sample(ledger: &mut ContextFragmentLedger, anchor: usize) -> Vec<Message> {
    ledger.record_sampling(anchor, &[], Vec::new());
    let mut projected = vec![Message::user("measurement anchor"); anchor];
    ledger.project_sampling_snapshots(&mut projected, anchor);
    projected
}

fn latest_delta(messages: &[Message]) -> &str {
    messages.iter().rev().filter_map(|message| message.content.as_deref())
        .find(|content| content.starts_with("runtime_state_snapshot.v1\n")).expect("runtime delta missing")
}

fn coarse_state(ledger: &mut ContextFragmentLedger, state: &serde_json::Value) {
    // Exact old fragment construction, using the same real sampling ledger.
    ledger.upsert(ContextFragment::new("agent.working_state", FragmentScope::Dynamic, Role::System,
        format!("{COARSE_DESCRIPTION}{state}"), FragmentSensitivity::Private)).unwrap();
}

fn expected_records(state: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    let mut records = BTreeMap::new();
    for work in state["previous_work"].as_array().into_iter().flatten().chain(state.get("current_work")) {
        if let Some(turn) = work["turn_id"].as_str() {
            records.insert(format!("agent.work.{turn}"), work.clone());
        }
    }
    for correction in state["corrections"].as_array().into_iter().flatten() {
        if let Some(review) = correction["review_ref"].as_str() {
            records.insert(format!("agent.correction.{review}"), correction.clone());
        }
    }
    records
}

#[test]
#[ignore = "requires the local private LangSmith pair export in CACHE_STATE_PAIR_INPUT"]
fn actual_langsmith_working_state_delta_reuses_unchanged_records() {
    let input_path = std::env::var_os("CACHE_STATE_PAIR_INPUT").expect("CACHE_STATE_PAIR_INPUT required");
    let export: serde_json::Value = serde_json::from_slice(&std::fs::read(input_path).expect("read export"))
        .expect("parse export");
    let before = working_state(&export, BEFORE_RUN);
    let after = working_state(&export, AFTER_RUN);
    assert!(before != after, "this pair must exercise a real working-state change");
    let old_records = expected_records(&before);
    let new_records = expected_records(&after);
    let unchanged: Vec<_> = new_records.iter().filter(|(key, record)| old_records.get(*key) == Some(*record)).collect();
    assert!(!unchanged.is_empty(), "this pair must include unchanged records");

    let mut baseline = ContextFragmentLedger::default();
    coarse_state(&mut baseline, &before);
    sample(&mut baseline, 2);
    coarse_state(&mut baseline, &after);
    let baseline_second = sample(&mut baseline, 4);
    let coarse_bytes = latest_delta(&baseline_second).len();

    let mut candidate = ContextFragmentLedger::default();
    refresh_tutor_working_fragments(&mut candidate, &serde_json::json!({"status":"active","agent_working_state":before})).unwrap();
    let candidate_first = sample(&mut candidate, 2);
    refresh_tutor_working_fragments(&mut candidate, &serde_json::json!({"status":"active","agent_working_state":after})).unwrap();
    let candidate_second = sample(&mut candidate, 4);
    assert!(candidate_second[..candidate_first.len()] == candidate_first,
        "the original request prefix must remain byte-for-byte unchanged");
    let delta = latest_delta(&candidate_second);
    for (_, record) in &unchanged {
        assert!(!delta.contains(&record.to_string()), "an unchanged full record must not be resent");
    }

    let snapshot = candidate.snapshot();
    let mut actual_records = BTreeMap::new();
    for fragment in &snapshot.fragments {
        if new_records.contains_key(&fragment.key) {
            let start = fragment.content.find('{').expect("record body missing");
            actual_records.insert(fragment.key.clone(), serde_json::from_str::<serde_json::Value>(&fragment.content[start..]).expect("record body invalid"));
        }
    }
    assert!(actual_records == new_records, "every active work and correction record must remain exact");
    assert!(candidate.retire_sampling_history(), "pressure must rebuild the current full state");
    let rebuilt = sample(&mut candidate, 4);
    for record in new_records.values() {
        assert!(latest_delta(&rebuilt).contains(&record.to_string()), "pressure rebuild must restore all active records");
    }

    let candidate_bytes = delta.len();
    assert!(candidate_bytes < coarse_bytes, "granular projection must reduce this real repeated-state delta");
    let report = serde_json::json!({
        "measurement":"real refresh_tutor_working_fragments and ContextFragmentLedger; UTF-8 message-body bytes, not token usage",
        "before_run":BEFORE_RUN,"after_run":AFTER_RUN,
        "coarse_working_state_json_bytes":after.to_string().len(),
        "coarse_delta_bytes":coarse_bytes,"granular_delta_bytes":candidate_bytes,
        "delta_bytes_saved":coarse_bytes-candidate_bytes,
        "delta_byte_reduction_percent":(coarse_bytes-candidate_bytes) as f64 / coarse_bytes as f64 * 100.0,
        "active_records":new_records.len(),"unchanged_records":unchanged.len(),
        "unchanged_record_json_bytes":unchanged.iter().map(|(_, record)| record.to_string().len()).sum::<usize>(),
        "exact_active_state_preserved":true,"exact_old_request_prefix_preserved":true,
        "pressure_rebuild_preserves_all_active_records":true
    });
    if let Some(output_path) = std::env::var_os("CACHE_STATE_MEASUREMENT_OUTPUT") {
        std::fs::write(output_path, serde_json::to_vec_pretty(&report).unwrap()).expect("write aggregate report");
    }
    println!("{report}");
}
