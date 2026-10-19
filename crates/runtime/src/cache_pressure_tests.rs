use super::*;
use crate::tool_result::{ToolContinuation, ToolResultEnvelope, ToolResultStatus};

fn cc2_profile() -> ModelRuntimeProfile {
    let mut profile = ModelRuntimeProfile::fallback("cc2-pressure", ProviderToolProtocol::Native);
    profile.context_window_tokens = 1_000_000;
    profile.output_reserve_tokens = 1_024;
    profile.safety_margin_tokens = 1_024;
    profile.compaction.high_watermark_ratio = 0.9;
    profile
}

fn cc2_append_result(messages: &mut Vec<Message>, ledger: &mut ActiveToolResultLedger, id: &str, size: usize) {
    let tool_call = call(id, "book.text", r#"{"lid":"1.1"}"#);
    messages.push(Message { provider_continuation: None, role: Role::Assistant, content: None,
        tool_calls: vec![tool_call], tool_call_id: None });
    messages.push(Message { provider_continuation: None, role: Role::Tool,
        content: Some(format!("durable source for {id}")), tool_calls: vec![], tool_call_id: Some(id.into()) });
    ledger.insert(id, ToolResultEnvelope {
        version: crate::tool_result::TOOL_RESULT_ENVELOPE_VERSION.into(),
        status: ToolResultStatus::Ok,
        model_body: serde_json::json!({"text":"x".repeat(size)}),
        receipt: HistoricalToolReceipt {
            version: "historical_tool_receipt.v1".into(), tool: "book.text".into(),
            locator_args: serde_json::json!({"lid":"1.1"}), status: HistoricalToolStatus::Ok,
            error_code: None, accepted_evidence: vec![read_tools::EvidenceRange {
                start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![],
            }], source_refs: vec!["source_ref_cc2".into()], opaque_result_digest: format!("result-{id}"),
        },
        truncated: true,
        continuation: Some(ToolContinuation::NextCall {
            tool: "book.text".into(), arguments: serde_json::json!({"lid":"1.1","start_utf16":32}),
            reason: "continue the observed range".into(),
        }),
    });
}

fn cc2_plan(messages: &[Message], ledger: &mut ActiveToolResultLedger, profile: &ModelRuntimeProfile) -> AgentRequestPlan {
    let (_, plan) = build_sample_request(messages, &mut ContextFragmentLedger::default(), &book(), ledger,
        None, COMPACTION_CONSUMPTION_WRAPPER, &resident_tool_registry(), profile,
        ToolPermissions::default(), &ToolExposureState::default(), ArtifactExposureContext::no_overlay(),
        EvidenceState::KnownLids, &[], None, false, None).unwrap();
    plan
}

fn cc2_envelope(plan: &AgentRequestPlan, id: &str) -> ToolResultEnvelope {
    let message = plan.input.iter().find(|message| message.tool_call_id.as_deref() == Some(id)).unwrap();
    serde_json::from_str(message.content.as_deref().unwrap()).unwrap()
}

#[test]
fn cc2_high_watermark_retires_one_sampled_batch_and_keeps_new_body_and_receipts() {
    let mut messages = new_session();
    messages.push(Message::user("Continue reading the observed passage."));
    let mut ledger = ActiveToolResultLedger::default();
    cc2_append_result(&mut messages, &mut ledger, "sampled-a", 30_000);
    cc2_append_result(&mut messages, &mut ledger, "sampled-b", 30_000);
    ledger.mark_projected_fresh_results_sampled();
    cc2_append_result(&mut messages, &mut ledger, "fresh", 400);
    let durable_before = serde_json::to_vec(&messages).unwrap();
    let mut profile = cc2_profile();
    let before = cc2_plan(&messages, &mut ledger, &profile);
    assert!(!ActiveContextBudget::from_plan(&before).over_high_watermark);
    profile.compaction.high_watermark_ratio = 0.000001;
    let after = cc2_plan(&messages, &mut ledger, &profile);
    for id in ["sampled-a", "sampled-b"] {
        let old = cc2_envelope(&before, id);
        let retired = cc2_envelope(&after, id);
        assert!(retired.model_body.is_null(), "soft pressure must retire the sampled batch");
        assert_eq!(retired.receipt, old.receipt);
        assert_eq!(retired.continuation, old.continuation);
        assert_eq!(retired.truncated, old.truncated);
    }
    assert_eq!(cc2_envelope(&after, "fresh"), cc2_envelope(&before, "fresh"));
    assert_eq!(serde_json::to_vec(&messages).unwrap(), durable_before);
    assert!(after.active_context.estimated_input_tokens < before.active_context.estimated_input_tokens);

    // A useful compaction may leave the fixed baseline above the soft threshold.
    // Do not turn each following small observation into another prefix rewrite.
    ledger.mark_projected_fresh_results_sampled();
    let next = cc2_plan(&messages, &mut ledger, &profile);
    assert!(ActiveContextBudget::from_plan(&next).over_high_watermark);
    assert_eq!(cc2_envelope(&next, "fresh"), cc2_envelope(&before, "fresh"));
}

#[test]
fn cc2_hard_budget_reclaims_a_smaller_sampled_batch_and_recomputes_fit() {
    let mut messages = new_session();
    messages.push(Message::user("Continue reading."));
    let mut ledger = ActiveToolResultLedger::default();
    cc2_append_result(&mut messages, &mut ledger, "sampled", 4_000);
    ledger.mark_projected_fresh_results_sampled();
    cc2_append_result(&mut messages, &mut ledger, "fresh", 400);
    let mut profile = cc2_profile();
    let before = cc2_plan(&messages, &mut ledger, &profile);
    profile.context_window_tokens = ActiveContextBudget::from_plan(&before).pressure_tokens - 128;
    profile.compaction.high_watermark_ratio = 1.0;
    let after = cc2_plan(&messages, &mut ledger, &profile);
    assert!(cc2_envelope(&after, "sampled").model_body.is_null());
    assert_eq!(cc2_envelope(&after, "fresh"), cc2_envelope(&before, "fresh"));
    assert!(ActiveContextBudget::from_plan(&after).fits, "hard fit must use the rebuilt request estimate");
}

#[test]
fn cc2_hard_budget_never_retires_unseen_results() {
    let mut messages = new_session();
    messages.push(Message::user("Read the source before answering."));
    let mut ledger = ActiveToolResultLedger::default();
    cc2_append_result(&mut messages, &mut ledger, "fresh", 4_000);
    let mut profile = cc2_profile();
    let before = cc2_plan(&messages, &mut ledger, &profile);
    profile.context_window_tokens = ActiveContextBudget::from_plan(&before).pressure_tokens - 128;
    profile.compaction.high_watermark_ratio = 1.0;
    let after = cc2_plan(&messages, &mut ledger, &profile);
    assert_eq!(cc2_envelope(&after, "fresh"), cc2_envelope(&before, "fresh"));
    assert!(!ActiveContextBudget::from_plan(&after).fits,
        "unseen source cannot be erased to make a request appear to fit");
}
