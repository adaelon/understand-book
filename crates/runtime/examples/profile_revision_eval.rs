//! Small live regression sample for ordinary durable profile revisions.
//! Usage: cargo run -p runtime --example profile_revision_eval -- <output-directory>
use memory::*;
use runtime::memory_review::{execute_review, ReviewInput, ReviewTurnInput, ReviewTurnStatus};
use serde_json::json;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("Pass an output directory")?);
    std::fs::create_dir_all(&output)?;
    let config = runtime::ProviderConfig::from_env().map_err(|error| error.message)?;
    let adapter = runtime::ProviderRegistry::adapter_from_config(config.clone());
    let cases = [
        (
            "ordinary_after_correction",
            "我现在更习惯先看完整推导，再看例子。",
            true,
            false,
            false,
            false,
        ),
        (
            "temporary_exception",
            "这一次先把完整推导写出来，平常还是按原来的方式。",
            false,
            false,
            false,
            false,
        ),
        (
            "paraphrased_trait",
            "我现在更适合先把整套推理讲清楚再举例子，这样长期学起来顺畅多了。",
            true,
            false,
            false,
            false,
        ),
        (
            "general_capability",
            "我的统计学基础已经系统补过一遍了，现在具备中级统计学背景。",
            true,
            true,
            false,
            false,
        ),
        (
            "global_continuation",
            "我现在习惯把讲解读得完整一点，详细展开对我更适合。",
            true,
            false,
            true,
            false,
        ),
        (
            "old_job_finishes_last",
            "我现在更习惯先看完整推导，再看例子。",
            false,
            false,
            false,
            true,
        ),
    ];
    let mut reports = Vec::new();
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    for (name, words, should_change, capability, global, delayed) in cases {
        let mut store = MemoryStore::open(output.join(format!("{run_id}-{name}/memory.json")))
            .map_err(|error| error.message)?;
        let payload = |value: &str| {
            if capability {
                ProfilePayload::Capability(CapabilityClaim {
                    key: "statistics_background".into(),
                    value: value.into(),
                })
            } else {
                ProfilePayload::ExplanationPreference(PreferenceClaim {
                    key: if global {
                        "explanation_depth"
                    } else {
                        "presentation_order"
                    }
                    .into(),
                    value: value.into(),
                })
            }
        };
        let old_words = if capability {
            "我的统计学基础还比较薄弱。"
        } else if global {
            "不管读哪本书，我都偏好简短讲解。"
        } else {
            "我偏好先看直观例子，再听原理。"
        };
        let make_fact = |session: &str, value: &str| CreateProfileFact {
            scope: if global {
                ProfileScope::Global
            } else {
                ProfileScope::Book {
                    book_id: "book".into(),
                }
            },
            applicability: Applicability::Any,
            payload: payload(value),
            source: FactSource::UserStated,
            evidence: vec![EvidenceRef::Turn {
                session_id: session.into(),
                turn_id: "turn".into(),
            }],
            confidence: None,
            sensitivity: Sensitivity::Normal,
            valid_until: None,
        };
        let a = store
            .create_profile_fact(make_fact("original", "之前的偏好或背景"), "1")
            .map_err(|error| error.message)?;
        let old = store
            .correct_profile_fact(&a.fact_id, make_fact("corrected", old_words), "2")
            .map_err(|error| error.message)?;
        let mut unrelated = make_fact("unrelated", "长期使用中文");
        unrelated.payload = ProfilePayload::ExplanationPreference(PreferenceClaim {
            key: "language".into(),
            value: "Chinese".into(),
        });
        let unrelated = store
            .create_profile_fact(unrelated, "2")
            .map_err(|error| error.message)?;
        store
            .reconcile_review_jobs(
                &[ReviewSessionCursor {
                    learning_workspace_id: None,
                    turns: vec![],
                    session_id: name.into(),
                    book_id: "book".into(),
                    latest_user_turn_ordinal: 1,
                }],
                "10",
            )
            .map_err(|error| error.message)?;
        let job_id = store.review_state().review_jobs[0].job_id.clone();
        let job = store
            .claim_review_job(&job_id, "11")
            .map_err(|error| error.message)?;
        let input = ReviewInput {
            job_id: job.job_id.clone(),
            session_id: name.into(),
            book_id: "book".into(),
            content_profile: "technical_learning".into(),
            from_turn_exclusive: 0,
            to_turn_inclusive: 1,
            turns: vec![ReviewTurnInput {
                book_id: Some("book".into()),
                turn_id: "turn".into(),
                user_turn_ordinal: 1,
                observed_at_ms: Some(3_000),
                user: words.into(),
                input_action: None,
                assistant_status: ReviewTurnStatus::Completed,
                assistant_answer: None,
            }],
            existing_facts: vec![old.clone(), unrelated.clone()],
            existing_fact_evidence: BTreeMap::from([
                (old.fact_id.clone(), vec![old_words.into()]),
                (unrelated.fact_id.clone(), vec!["我一直用中文学习。".into()]),
            ]),
        };
        let started = std::time::Instant::now();
        let result = execute_review(adapter.as_ref(), &input);
        let current = if delayed {
            store
                .correct_profile_fact(
                    &old.fact_id,
                    make_fact("newest", "我现在长期更喜欢先看核心结论，再按需要展开。"),
                    "4",
                )
                .map_err(|error| error.message)?
        } else {
            old.clone()
        };
        let (mut passed, mut detail) = (false, json!(null));
        if let Ok(result) = &result {
            match store.commit_review_result(
                &job.job_id,
                &["turn".into()],
                &input.existing_facts,
                &result.fact_candidates,
                &result.intent_observations,
                "99",
            ) {
                Ok(commit) => {
                    let snapshot = store.project_reader_profile_snapshot(
                        &SnapshotRequest::current(SnapshotContext {
                            book_id: Some("book".into()),
                            ..Default::default()
                        }),
                    );
                    let ids = snapshot.injected_fact_ids();
                    let changed =
                        !ids.contains(&current.fact_id) && !commit.added_fact_ids.is_empty();
                    let replacement = commit
                        .added_fact_ids
                        .iter()
                        .filter_map(|id| {
                            store
                                .profile_facts()
                                .iter()
                                .find(|fact| fact.fact_id == *id)
                        })
                        .any(|fact| {
                            fact.supersedes.contains(&old.fact_id)
                                && fact.payload.semantic_key() == old.payload.semantic_key()
                        });
                    passed = changed == should_change
                        && ids.contains(&unrelated.fact_id)
                        && if should_change {
                            replacement
                        } else {
                            ids.contains(&current.fact_id) && commit.added_fact_ids.is_empty()
                        };
                    detail = json!({"added_fact_ids":commit.added_fact_ids,"snapshot":snapshot,"facts":store.profile_facts()});
                }
                Err(error) => {
                    detail = json!({"commit_error":error});
                }
            }
        }
        let report = json!({"case":name,"passed":passed,"model":config.model,"elapsed_ms":started.elapsed().as_millis(),
            "input":input,"parsed_output":result.as_ref().ok(),"error":result.as_ref().err().map(|error| &error.message),"committed":detail});
        std::fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!("{name}: {}", if passed { "passed" } else { "FAILED" });
        reports.push(report);
    }
    std::fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&reports)?,
    )?;
    if reports.iter().any(|report| report["passed"] != true) {
        return Err("Profile revision sample failed; inspect retained results".into());
    }
    Ok(())
}
