#![allow(unused_imports)]
use artifact_tools::{ArtifactAccessSnapshot, ARTIFACT_OVERLAY_UNAVAILABLE};
use book_tool_contracts::{
    from_rest_alias, validate_input, BookToolId, BookToolInput, ContextInput,
    PaperReadingGuideInput,
};
use memory::{
    classify_profile_fact_privacy, classify_profile_privacy, Anchor, Applicability,
    BackgroundClaim, CapabilityClaim, CollectionRuleMatcher, ConstraintClaim, ExplicitProfileFact,
    GoalClaim, HistoricalBackfillRange, MemoryOp, MemoryOpOutcome, MemoryStore, NoteBodyPlacement,
    NoteSaveOutcome, NoteSaveStatus, PdfSourceMapVersion, PendingTurnRef, PreferenceClaim,
    ProfileGovernanceAction, ProfileGovernanceMutation, ProfileGovernanceOutcome,
    ProfileGovernanceOutcomeKind, ProfilePayload, ProfilePayloadKind, ProfilePrivacyClass,
    ProfileResolutionContext, ProfileScope, ProfileStatus, PromoteInput, ReanchorInput,
    RecallQuery, ReplaceInput, ReviewJobStatus, ReviewSessionCursor, SaveInput, SelectedRange,
    SelectionContext, SelectionResolution, SelectionResolutionBasis, Sensitivity, SnapshotContext,
    SnapshotRequest,
};
use read_tools::{
    disambiguate_source_labels, Book, ContentProfileId, EvidenceRange, PaperLandmarkKind,
    PaperLexiconEntry, PaperMinimapBase, PaperRegionKind, ReaderLayoutAction, ResolvedSource,
    SourceSelectedRange, SourceTextRange, ToolError,
};
use reader::{
    project_paper_minimap_lens, PaperMinimapActor, PaperMinimapApplyOutcome, PaperMinimapCommand,
    Reader, SavedUserOverlay, DEFAULT_RADIUS,
};
use runtime::context_fragment::{
    ContextFragment, FragmentScope, FragmentSensitivity, MEMORY_OPERATION_FRAGMENT_KEY,
};
use runtime::memory_intent::{
    evaluate_memory_intent, scan_memory_intent, MemoryIntentDecision, MemoryIntentRequest,
};
use runtime::memory_policy::{
    MemoryPolicyRegistry, PaperPolicyContext, PolicyProjectionInput, PAPER_MEMORY_POLICY_ID,
};
use runtime::orchestrator::{
    new_session, AgentAnswerPart, AgentAnswerSource, AgentAnswerView, AgentEffect,
    AnswerDeliveryDiagnostics, OuterConfig, OuterOutcome, ProfileMemoryUpdate,
    ProfileMemoryUpdateKind, ProfileUsageTrace, ResidentTurnResources, SourceBinding,
};
use runtime::profile_api::{
    historical_backfill_job_view, profile_governance_outcome_view, HistoricalBackfillJobRequest,
    HistoricalBackfillSessionView, HistoricalBackfillStartRequest, HistoricalBackfillStateView,
    ProfileCollectionRuleMatcherView, ProfileFactDraftView, ProfileGovernanceActionRequest,
    ProfileGovernanceMutationRequest, ProfileGovernanceResponseView,
};
use runtime::{
    guided_route_from, synthesize, unvisited_back, validate_persisted_checkpoint, AdapterError,
    AgentRequestPlan, AssistantTurn, CompactionCheckpoint, CompactionCheckpointSink,
    CompactionError, CompletionRequest, Message, ModelAdapter, ParsedResponse, ProviderConfig,
    ProviderRegistry,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;


    use super::*;
    use base_schema::{
        sample_base, FormulaComposition, FormulaParameter, FormulaSemantics, LidNode, NodeKind,
        ReadOnlyBase, Span,
    };
    use memory::{CreateProfileFact, EvidenceRef, FactSource};
    use runtime::{RawCitation, ToolCall};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};


pub use reader_service::tests::*;
mod music_runtime_tests;
    mod video_draft_tests;
    mod video_publication_tests;
    mod music_publication_tests;
    mod video_source_tests;
    mod video_workspace_tests;
    mod video_operator_build_tests;
    mod adm10_tests;
    mod adm2_tests;
    mod inv2_tests;
    mod guest_trial_tests;
    mod inv5_tests;
    mod inv6_tests;
    mod inv7_tests;
    mod adm3_tests;
    mod q1_tests;
    mod q3_cost_tests;
    mod q4_allowance_tests;
    mod q5_cost_tests;
    mod s1_workspace_publication_tests;
    mod b1_operator_build_tests;
    mod b2_preparation_tests;
    mod c4_service_usage_tests;
    mod cloud_host_tests;
    mod cloud_course_e2e_tests;
    mod cloud_browser_host;
    mod s2_b1_browser_host;
    mod inquiry_course_tests;
    mod q5_s1_browser_host;
    mod mcp_pack_api_tests;
    mod adm5_tests;
    mod adm6_tests;
    mod adm7_tests;
    mod adm9_tests;
    mod monitor_tests;
    mod operations_integration_tests;
    mod mu2_tests;
    mod mu3_tests;
    mod online_publication_tests;
    mod mu4_tests;
    mod pdf_service_tests;
    pub(crate) mod mu5_tests;
    mod mu6_tests;
    mod cross_book_tests;
    mod presentation_request_tests;
    mod mu7_tests;
    mod mu8_tests;
    mod w3_profile_scope_tests;
    mod w5_reader_tests;
    mod board_network_tests;
    mod workspace_notes_tests;
    pub(crate) mod board_follow_up_browser_host;
    pub(crate) mod board_wb3_browser_host;
    pub(crate) mod board_wb4_browser_host;
    mod mu9_tests;
    pub(crate) mod mu10_tests;
    mod presentation_store_tests;
    mod tutor_tests;
    pub(crate) mod tutor_loop_tests;
    mod tutor_continuation_tests;

    #[test]
    fn agent_source_rs6_bound_material_title_and_cross_material_fallback() {
        let mut state = state_named("rs6-answer-source");
        let (turn_id, source_ref_id) = install_source_bound_turn(&mut state, tmp("rs6-answer-history"));
        state.workspace.book_dir = tmp("rs6-source-material");
        let path = workbench_input_manifest_path(&state.workspace.book_dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"display_title":"原回答的材料"}"#).unwrap();
        let request = json!({ "turn_id": turn_id, "source_ref_id": source_ref_id });
        let history_before = serde_json::to_value(&state.user.agent_history).unwrap();
        let local = post(&mut state, "/agent/source.resolve", &request.to_string());
        assert_eq!(local.status, 200, "{}", local.body);
        let local: Value = serde_json::from_str(&local.body).unwrap();
        assert_eq!(local["material_title"], "原回答的材料");
        let network = workspace_client::read(&state.user, &state.workspace, "agent/source.resolve", &request, "2026-10-10").unwrap().unwrap();
        assert_eq!(network["material_title"], local["material_title"]);
        assert_eq!(network["highlighted_quote"], local["highlighted_quote"]);
        assert_eq!(serde_json::to_value(&state.user.agent_history).unwrap(), history_before);

        let binding = &mut state.user.agent_history.sessions[0].turns[0].source_bindings[0];
        binding.book_id = "other-material".into();
        binding.label_snapshot = "原外部材料 · 第二章".into();
        let local = post(&mut state, "/agent/source.resolve", &request.to_string());
        let local: Value = serde_json::from_str(&local.body).unwrap();
        let network = workspace_client::read(&state.user, &state.workspace, "agent/source.resolve", &request, "2026-10-10").unwrap().unwrap();
        for result in [local, network] {
            assert!(result["material_title"].is_null());
            assert_eq!(result["label"], "原外部材料 · 第二章");
            assert_eq!(result["stale"], true);
            assert_eq!(result["can_open_in_reader"], false);
        }
    }
mod observability_tests;

mod q2_cost_tests;
mod fp5_tests;
mod session_store_tests;
mod board_api_tests;
mod board_source_publication_tests;

use reader_service::tests::host_calls::{precommit_agent_turn,prepare_agent_chat,agent_source_binding,run_precommitted_agent_chat,source_manifest_value,list_mixed_book_library,current_note_source_fingerprint,localize_paper_minimap,finalize_agent_turn,route_agent_new,route_agent_history_select,workbench_input_manifest_path};
use reader_service::service_error;

use reader_service::tests::runtime_access::*;

mod model_settings_tests;
