use super::*;
use crate::{AdapterError, AssistantTurn, NativeAdapter, ParsedResponse, ProviderConfig};
use std::cell::RefCell;
use std::path::PathBuf;

fn recordings() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../fixtures/compaction-20261016/failures.json"
    ))
    .unwrap()
}

fn recorded_request(recording: &serde_json::Value) -> CompactionRequest {
    let mut input = recording["request"].clone();
    input["schema_version"] = COMPACTION_REQUEST_VERSION.into();
    input["prompt_version"] = COMPACTION_PROMPT_VERSION.into();
    input["source_history_revision"] = recording["trace_id"].clone();
    input["raw_retained_item_ids"] = serde_json::json!([]);
    serde_json::from_value(input).unwrap()
}

struct ReplayCompactor {
    first: RefCell<Option<serde_json::Value>>,
    repeated: serde_json::Value,
    requests: RefCell<Vec<CompletionRequest>>,
    live: Option<(NativeAdapter, PathBuf)>,
}

impl ModelAdapter for ReplayCompactor {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        panic!("compaction must use structured completion")
    }

    fn chat(&self, _: &crate::AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        panic!("compaction must not use tools")
    }

    fn complete_structured(
        &self,
        req: CompletionRequest,
    ) -> Result<serde_json::Value, AdapterError> {
        self.requests.borrow_mut().push(req.clone());
        if let Some(first) = self.first.borrow_mut().take() {
            return Ok(first);
        }
        assert_eq!(
            self.requests.borrow().len(),
            2,
            "only one repair is allowed"
        );
        if let Some((live, path)) = &self.live {
            std::fs::write(
                path.with_extension("request.json"),
                serde_json::to_vec_pretty(&req).unwrap(),
            )
            .unwrap();
            let result = live.complete_structured(req)?;
            std::fs::write(
                path.with_extension("response.json"),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            Ok(result)
        } else {
            Ok(self.repeated.clone())
        }
    }
}

fn replay_recorded_failure(turn: usize, attempt: usize, expected_details: &[&str]) {
    let recordings = recordings();
    let recording = &recordings[turn];
    let request = recorded_request(recording);
    let rejected = &recording["attempts"][attempt]["draft"];
    let draft: CompactionDraft = serde_json::from_value(rejected.clone()).unwrap();
    let error = validate_draft(&request, &draft, &BTreeMap::new()).unwrap_err();
    for detail in expected_details {
        assert!(
            error.message.contains(detail),
            "missing {detail:?}: {}",
            error.message
        );
    }
    let adapter = ReplayCompactor {
        first: RefCell::new(Some(rejected.clone())),
        repeated: rejected.clone(),
        requests: RefCell::new(Vec::new()),
        live: None,
    };
    let profile = crate::ModelRuntimeCatalog::default().resolve(
        recording["model"].as_str().unwrap(),
        crate::ProviderToolProtocol::Native,
        None,
    );
    assert!(call_generator(
        &adapter,
        &profile,
        COMPACTION_GENERATION_PROMPT,
        &request,
        &BTreeMap::new(),
        100_000
    )
    .is_err());
    let requests = adapter.requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].system, requests[1].system);
    assert!(requests[1].user.starts_with(&requests[0].user));
    let tail: serde_json::Value =
        serde_json::from_str(&requests[1].user[requests[0].user.len()..]).unwrap();
    let diagnostic = &tail["runtime_repair_diagnostic"];
    assert_eq!(
        diagnostic["rejected_draft"], *rejected,
        "repair must see the actual rejected output"
    );
    assert_eq!(diagnostic["message"], error.message);
}

#[test]
fn recorded_compaction_first_turn_ranges() {
    replay_recorded_failure(
        0,
        0,
        &[
            "item.decision.crosslang",
            "evidence_refs",
            "7.7.1",
            "7.7.9",
            "26.5.4.3",
            "allowed_evidence_refs",
        ],
    );
}

#[test]
fn recorded_compaction_first_turn_dangling_targets() {
    replay_recorded_failure(
        0,
        1,
        &[
            "source.17.",
            "source.18.",
            "source.52.",
            "source.53.",
            "answer.readonlybase",
            "answer.crosslang",
            "target_item_ids",
        ],
    );
}

#[test]
fn recorded_compaction_second_turn_unicode_id() {
    replay_recorded_failure(
        1,
        0,
        &[
            "critical_examples",
            "item_id",
            "item.example.span-A中B",
            "ASCII",
        ],
    );
}

#[test]
fn recorded_compaction_second_turn_reports_both_defects() {
    replay_recorded_failure(
        1,
        1,
        &[
            "item.decision.move_crosslang",
            "7.7.9",
            "item.example.A中B",
            "ASCII",
        ],
    );
}

#[test]
fn recorded_compaction_replays_all_four_successful_live_repairs() {
    let recordings = recordings();
    let repairs: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/compaction-20261016/repairs.json")).unwrap();
    let mut replayed = 0;
    for recording in recordings.as_array().unwrap() {
        let request = recorded_request(recording);
        let profile = crate::ModelRuntimeCatalog::default().resolve(
            recording["model"].as_str().unwrap(),
            crate::ProviderToolProtocol::Native,
            None,
        );
        for attempt in recording["attempts"].as_array().unwrap() {
            let repair = repairs
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["run_id"] == attempt["run_id"])
                .unwrap();
            let adapter = ReplayCompactor {
                first: RefCell::new(Some(attempt["draft"].clone())),
                repeated: repair["draft"].clone(),
                requests: RefCell::new(Vec::new()),
                live: None,
            };
            let draft = call_generator(
                &adapter,
                &profile,
                COMPACTION_GENERATION_PROMPT,
                &request,
                &BTreeMap::new(),
                100_000,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(&draft).unwrap(),
                repair["draft"],
                "recorded model output must not be rewritten locally"
            );
            assert_eq!(draft.source_coverage.len(), request.eligible_items.len());
            assert_eq!(adapter.requests.borrow().len(), 2);
            replayed += 1;
        }
    }
    assert_eq!(replayed, 4);
}

fn live_repair(turn: usize, attempt: usize) {
    let recordings = recordings();
    let recording = &recordings[turn];
    let request = recorded_request(recording);
    let rejected = recording["attempts"][attempt]["draft"].clone();
    let directory = PathBuf::from(
        std::env::var_os("COMPACTION_REPLAY_OUTPUT").expect("COMPACTION_REPLAY_OUTPUT"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(recording["attempts"][attempt]["run_id"].as_str().unwrap());
    let mut config = ProviderConfig::from_env().unwrap();
    config.model = recording["model"].as_str().unwrap().into();
    let profile = crate::ModelRuntimeCatalog::default().resolve(
        &config.model,
        crate::ProviderToolProtocol::Native,
        None,
    );
    let adapter = ReplayCompactor {
        first: RefCell::new(Some(rejected.clone())),
        repeated: rejected,
        requests: RefCell::new(Vec::new()),
        live: Some((
            NativeAdapter::from_config_with_timeout(config, std::time::Duration::from_secs(240)),
            path.clone(),
        )),
    };
    let result = call_generator(
        &adapter,
        &profile,
        COMPACTION_GENERATION_PROMPT,
        &request,
        &BTreeMap::new(),
        100_000,
    );
    let report = match &result {
        Ok(draft) => {
            serde_json::json!({"status":"passed", "model":profile.matched_model, "calls":adapter.requests.borrow().len(), "source_count":draft.source_coverage.len()})
        }
        Err(error) => {
            serde_json::json!({"status":"failed", "error_code":error.error_code, "message":error.message})
        }
    };
    std::fs::write(
        path.with_extension("result.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let repaired = result.unwrap();
    validate_draft(&request, &repaired, &BTreeMap::new()).unwrap();
    println!("{}: {}", recording["attempts"][attempt]["run_id"], report);
}

#[test]
#[ignore = "explicit live model replay; requires provider credentials and COMPACTION_REPLAY_OUTPUT"]
fn recorded_compaction_live_first_ranges() {
    live_repair(0, 0);
}
#[test]
#[ignore = "explicit live model replay; requires provider credentials and COMPACTION_REPLAY_OUTPUT"]
fn recorded_compaction_live_first_targets() {
    live_repair(0, 1);
}
#[test]
#[ignore = "explicit live model replay; requires provider credentials and COMPACTION_REPLAY_OUTPUT"]
fn recorded_compaction_live_second_id() {
    live_repair(1, 0);
}
#[test]
#[ignore = "explicit live model replay; requires provider credentials and COMPACTION_REPLAY_OUTPUT"]
fn recorded_compaction_live_second_combined() {
    live_repair(1, 1);
}
