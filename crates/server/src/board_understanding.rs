//! Board understanding is a retained view of actual learner evidence, never model-authored claims.
use crate::{board_api::FrozenBoardFollowUp, note_api, teaching, user_runtime::UserRuntime, SourceBinding};
use memory::boards::{BoardContentRef, BoardDocument, BoardItem, BoardPosition, BoardSize, BoardTarget};
use memory::learning_evidence::LearningEvidence;
use memory::teaching::TeachingFact;
use memory::NoteSourceBinding;
use read_tools::ToolError;
use serde_json::Value;

fn source_matches(evidence: &LearningEvidence, source: &SourceBinding) -> bool {
    if source.book_id != evidence.source_id
        || source.source_revision.as_deref() != Some(evidence.source_revision.as_str())
    {
        return false;
    }
    let Some(target) = &evidence.target else { return false; };
    source.evidence_range.as_ref().is_some_and(|range| target.source_bindings.iter().any(|binding|
        binding.source_id == source.book_id && Some(binding.source_revision.as_str()) == source.source_revision.as_deref()
            && binding.start_lid == range.start_lid && binding.end_lid == range.end_lid))
        || source.media.as_ref().is_some_and(|snapshot| target.media_bindings.iter().flatten().any(|binding|
            binding.source_id == source.book_id && Some(binding.source_revision.as_str()) == source.source_revision.as_deref()
                && binding.target == snapshot.target))
        || source.music.as_ref().is_some_and(|snapshot| target.music_bindings.iter().flatten().any(|binding|
            binding.source_id == source.book_id && Some(binding.source_revision.as_str()) == source.source_revision.as_deref()
                && binding.target == snapshot.target))
}

fn bindings(evidence: &LearningEvidence, view: &Value, observed: &[SourceBinding]) -> Vec<NoteSourceBinding> {
    // Do not reintroduce private scoring sources through the run or parent snapshot.
    if view["feedback_hidden"] == true { return Vec::new(); }
    let mut sources: Vec<SourceBinding> = view["source_quotes"].as_array().into_iter().flatten()
        .filter_map(|value| serde_json::from_value(value.clone()).ok()).collect();
    for source in observed.iter().filter(|source| source_matches(evidence, source)) {
        if !sources.iter().any(|saved| saved.source_ref_id == source.source_ref_id) {
            sources.push(source.clone());
        }
    }
    note_api::bindings(&sources)
}

/// Stable, reversible identity encoding: evidence IDs contain ':' while board IDs do not.
fn item_id(evidence_id: &str) -> String {
    use std::fmt::Write;
    let mut id = String::from("item-understanding-");
    for byte in evidence_id.bytes() { write!(&mut id, "{byte:02x}").unwrap(); }
    id
}

pub(crate) fn items(
    user: &UserRuntime,
    frozen: &FrozenBoardFollowUp,
    session_id: &str,
    turn_id: &str,
    now: &str,
) -> Result<Vec<BoardItem>, ToolError> {
    let store = user.learning_store_for_workspace(frozen.learning_workspace_id.as_deref())?;
    let mut observed = frozen.source_bindings.clone();
    if let Some(turn) = user.agent_history.sessions.iter()
        .find(|session| session.id == session_id && session.matches_scope(frozen.learning_workspace_id.as_deref(), &frozen.book_id))
        .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == turn_id))
    {
        observed.extend(turn.source_bindings.clone());
    }
    let mut items = Vec::new();
    let mut after = 0;
    loop {
        let page = store.evidence_page(&frozen.book_id, after, 100)?;
        if page.is_empty() { break; }
        for (sequence, evidence) in page {
            after = sequence;
            // Evidence.session_id is the Tutor session. Only original expression facts
            // establish which actual chat turn supplied the learner's words.
            let expressions = store.evidence_expressions(&evidence)?;
            if expressions.is_empty() { continue; }
            let current_expression = expressions.iter().any(|(_, fact)|
                fact.binding.chat_session_id == session_id && fact.binding.turn_id == turn_id);
            let current_interpretation = if current_expression { false } else {
                match store.teaching_event(&format!("evidence-accepted:{turn_id}:{}", evidence.evidence_id)) {
                    Ok(event) => event.kind == TeachingFact::EvidenceInterpreted
                        && event.binding.chat_session_id == session_id && event.binding.turn_id == turn_id
                        && event.binding.source_id == evidence.source_id && event.binding.source_revision == evidence.source_revision
                        && event.binding.tutor_session_id == evidence.session_id
                        && event.payload["evidence_ref"] == evidence.evidence_id,
                    Err(error) if error.error_code == "TEACHING_INVALID" => false,
                    Err(error) => return Err(error),
                }
            };
            // A correction can reinterpret old words. Its accepted system fact owns
            // this run, while the preserved expressions still own their original times.
            if !current_expression && !current_interpretation { continue; }
            let view = teaching::retained_evidence_view(user, frozen.learning_workspace_id.as_deref(),
                &frozen.book_id, &evidence.evidence_id)?;
            let quote = view["learner_quote"].as_str().unwrap_or_default().to_owned();
            let interpretation = view["interpretation"].as_str().unwrap_or_default().to_owned();
            let label = view["label"].as_str().unwrap_or_default();
            let object_revision = evidence.object_revision.map(u32::try_from).transpose().map_err(|_| ToolError {
                error_code: "BOARD_REQUEST_INVALID".into(), category: "validation".into(),
                message: "Object revision is too large".into(),
            })?;
            items.push(BoardItem {
                item_id: item_id(&evidence.evidence_id), reply_to_item_id: frozen.follow_up.item_id.clone(),
                block_id: None, explanation_blocks: Vec::new(),
                title: if label.trim().is_empty() { "我的理解".into() } else { label.to_owned() },
                readable_content: format!("你的原话：\n{quote}\n\n系统解释：\n{interpretation}"),
                position: BoardPosition { x: 0.0, y: 0.0 }, size: BoardSize { width: 320.0, height: 220.0 },
                created_at: now.into(), source_bindings: bindings(&evidence, &view, &observed),
                target: BoardTarget::Understanding {
                    evidence_id: evidence.evidence_id, source_id: evidence.source_id, source_revision: evidence.source_revision,
                    object_id: evidence.object_id, object_revision, learner_quote: quote,
                    interpretation, evidence_snapshot: view,
                },
            });
        }
    }
    Ok(items)
}

/// Formal conditions retain their own object and map versions, independently of learner judgments.
pub(crate) fn formal_condition_refs(board: &BoardDocument, items: &[BoardItem]) -> Vec<BoardContentRef> {
    board.items.iter().filter(|prebuilt| {
        let BoardTarget::Prebuild { source_id, object_id, object_revision, map_revision, object_snapshot } = &prebuilt.target
            else { return false; };
        items.iter().any(|item| {
            let BoardTarget::Understanding { source_id: evidence_source, source_revision, object_id: evidence_object,
                object_revision: evidence_revision, evidence_snapshot, .. } = &item.target else { return false; };
            source_id == evidence_source && evidence_snapshot["map_revision"].as_str() == Some(map_revision.as_str())
                && object_snapshot["source_bindings"].as_array().into_iter().flatten().any(|source|
                    source["source_id"].as_str() == Some(source_id.as_str())
                        && source["source_revision"].as_str() == Some(source_revision.as_str()))
                && (evidence_object.as_ref() == Some(object_id) && *evidence_revision == Some(*object_revision)
                    || evidence_snapshot["target"]["object_refs"].as_array().into_iter().flatten().any(|object|
                        object["source_id"].as_str() == Some(source_id.as_str())
                            && object["object_id"].as_str() == Some(object_id.as_str())
                            && object["object_revision"].as_u64() == Some(u64::from(*object_revision))))
        })
    }).map(|item| BoardContentRef { item_id: item.item_id.clone(), block_id: None, content_revision: board.revision }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use memory::learning_evidence::EvidenceNature;
    use memory::teaching::{TeachingEvent, TeachingFact};

    const NOW: &str = "2026-10-25T20:00:00Z";

    fn fixture() -> (tempfile::TempDir, AppState, FrozenBoardFollowUp, BoardDocument, String, String) {
        let (root, state, metadata) = crate::tests::board_browser_host::fixture();
        let board = board_api::route(&state.user, &state.workspace, "create", &json!({"title":"理解与条件"}), NOW).unwrap();
        let board = board_api::route(&state.user, &state.workspace, "import", &json!({"board_id":board["board_id"],
            "board_revision":board["revision"],"source":metadata["sources"]["prebuild"]}), NOW).unwrap();
        let board: BoardDocument = serde_json::from_value(board).unwrap();
        let follow_up = memory::boards::BoardFollowUp { note_refs:None, board_id: board.board_id.clone(), board_revision: board.revision,
            item_id: Some(board.items[0].item_id.clone()), block_id: None, presentation: None };
        let frozen = board_api::freeze_follow_up(&state.user, &state.workspace, &follow_up).unwrap();
        (root, state, frozen, board, metadata["session_id"].as_str().unwrap().into(), metadata["turn_id"].as_str().unwrap().into())
    }

    fn original(state: &AppState) -> (LearningEvidence, TeachingEvent) {
        let store = state.user.learning_store().unwrap();
        let evidence = store.evidence("wb1-understanding").unwrap();
        let fact = store.teaching_event(&evidence.action_ref).unwrap();
        (evidence, fact)
    }

    #[test]
    fn wb3_board_understanding_uses_current_actual_expression_and_keeps_corrections_after_reopen() {
        let (root, mut state, frozen, board, session, turn) = fixture();
        let (evidence, fact) = original(&state);
        let initial = items(&state.user, &frozen, &session, &turn, NOW).unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].reply_to_item_id.as_deref(), frozen.follow_up.item_id.as_deref());
        assert!(!initial[0].source_bindings.is_empty());
        let mut learning = state.user.learning_store().unwrap();
        for (id, chat, run) in [("other-chat", "another-chat", turn.as_str()), ("old-turn", session.as_str(), "previous-turn")] {
            let mut other = fact.clone(); other.event_id = id.into(); other.binding.chat_session_id = chat.into(); other.binding.turn_id = run.into();
            learning.append_teaching(&other).unwrap();
            let mut interpreted = evidence.clone(); interpreted.evidence_id = format!("interpretation:{id}");
            interpreted.action_ref = other.event_id.clone(); interpreted.fact_refs = vec![other.event_id];
            learning.append_evidence(&interpreted).unwrap();
        }
        let mut corrected = evidence.clone(); corrected.evidence_id = "interpretation:corrected".into();
        corrected.supersedes = Some(evidence.evidence_id.clone()); corrected.interpretation = "只确认已表达的连接，独立迁移仍未知。".into();
        learning.append_evidence(&corrected).unwrap();
        drop(learning);
        state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
        let retained = items(&state.user, &frozen, &session, &turn, NOW).unwrap();
        assert_eq!(retained.len(), 2, "other chats and older expressions are not current-run evidence");
        assert_eq!(retained[0].item_id, initial[0].item_id);
        let BoardTarget::Understanding { learner_quote, interpretation, evidence_snapshot, .. } = &retained[0].target else { panic!() };
        assert_eq!(learner_quote, &evidence.learner_quote); assert_eq!(interpretation, &evidence.interpretation);
        assert_eq!(evidence_snapshot["superseded"], true);
        let BoardTarget::Understanding { evidence_snapshot, .. } = &retained[1].target else { panic!() };
        assert_eq!(evidence_snapshot["supersedes"], evidence.evidence_id);
        assert_eq!(evidence_snapshot["expressions"][0]["text"], evidence.learner_quote);
        let mut candidate = board.clone(); candidate.items.extend(retained.clone());
        memory::boards::BoardStore::open(root.path(), None, true).unwrap().validate_candidate(&candidate).unwrap();
        assert_eq!(retained, items(&state.user, &frozen, &session, &turn, NOW).unwrap());
    }

    #[test]
    fn wb3_board_understanding_links_only_exact_prebuilt_conditions() {
        let (_root, state, frozen, board, session, turn) = fixture();
        let retained = items(&state.user, &frozen, &session, &turn, NOW).unwrap();
        assert_eq!(formal_condition_refs(&board, &retained), vec![BoardContentRef {
            item_id: board.items[0].item_id.clone(), block_id: None, content_revision: board.revision,
        }]);
        for changed in ["source_id", "source_revision", "object_id", "object_revision", "map_revision"] {
            let mut stale = board.clone();
            let BoardTarget::Prebuild { source_id, object_id, object_revision, map_revision, object_snapshot } = &mut stale.items[0].target else { panic!() };
            match changed {
                "source_id" => *source_id = "another-book".into(),
                "source_revision" => object_snapshot["source_bindings"][0]["source_revision"] = json!("old-edition"),
                "object_id" => *object_id = "another-object".into(),
                "object_revision" => *object_revision += 1,
                "map_revision" => *map_revision = "old-map".into(),
                _ => unreachable!(),
            }
            assert!(formal_condition_refs(&stale, &retained).is_empty(), "{changed}");
        }
    }

    #[test]
    fn wb3_board_understanding_reuses_hidden_feedback_boundary_and_ignores_interface_answers() {
        let (_root, state, frozen, board, session, turn) = fixture();
        let (evidence, fact) = original(&state);
        let mut store = state.user.learning_store().unwrap();
        let mut selected = fact.clone(); selected.event_id = "selected:private".into(); selected.kind = TeachingFact::MoveSelected;
        selected.payload = json!({"move":{"assessment":{"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},
            "source_lids":["1.1"],"source_sufficient":true,"feedback":"on_reveal"}}});
        store.append_teaching(&selected).unwrap();
        let mut delivery = fact.clone(); delivery.event_id = "delivery:private".into(); delivery.kind = TeachingFact::MessageDelivered;
        delivery.payload = json!({"assessment_contract_ref":selected.event_id,"move":{"capability":evidence.capability,"target":evidence.target,"prompt":"Explain the conditions."}});
        store.append_teaching(&delivery).unwrap();
        let mut action = fact.clone(); action.event_id = "action:private".into(); action.kind = TeachingFact::LearnerAction;
        action.payload = json!({"delivery_ref":delivery.event_id,"response":evidence.learner_quote});
        store.append_teaching(&action).unwrap();
        let mut performance = evidence.clone(); performance.evidence_id = "assessment:private".into(); performance.nature = EvidenceNature::Performance;
        performance.action_ref = action.event_id.clone(); performance.fact_refs = vec![action.event_id.clone()];
        performance.delivery_ref = Some(delivery.event_id.clone());
        performance.interpretation = "本次回答满足部分标准".into();
        performance.source_quotes = json!([{"reason":"PRIVATE_SCORING_REASON", "quote":"PRIVATE_SCORING_SOURCE"}]);
        store.append_evidence(&performance).unwrap();
        let mut interface = action.clone(); interface.event_id = "action:interface".into();
        interface.payload["response_origin"] = json!({"kind":"interface_request"}); store.append_teaching(&interface).unwrap();
        let mut interface_evidence = performance.clone(); interface_evidence.evidence_id = "assessment:interface".into();
        interface_evidence.action_ref = interface.event_id.clone(); interface_evidence.fact_refs = vec![interface.event_id];
        store.append_evidence(&interface_evidence).unwrap();
        let hidden = items(&state.user, &frozen, &session, &turn, NOW).unwrap();
        assert_eq!(hidden.len(), 2, "an interface request is not a learner expression");
        let hidden = hidden.iter().find(|item| matches!(&item.target, BoardTarget::Understanding { evidence_id, .. } if evidence_id == &performance.evidence_id)).unwrap();
        assert!(hidden.source_bindings.is_empty());
        let BoardTarget::Understanding { evidence_snapshot, .. } = &hidden.target else { panic!() };
        assert_eq!(evidence_snapshot["feedback_hidden"], true); assert!(evidence_snapshot["source_quotes"].is_null());
        assert_eq!(evidence_snapshot["interpretation"], performance.interpretation, "the existing UI exposes the assessment status, not its private reasons");
        assert!(hidden.readable_content.contains(&performance.interpretation));
        assert!(!json!(hidden).to_string().contains("PRIVATE_SCORING_"));
        assert_eq!(formal_condition_refs(&board, std::slice::from_ref(hidden)), vec![BoardContentRef {
            item_id: board.items[0].item_id.clone(), block_id: None, content_revision: board.revision,
        }], "an already public formal condition does not reveal private scoring reasons");
        let mut reveal = fact; reveal.event_id = "help:revealed".into(); reveal.kind = TeachingFact::HelpDisplayed;
        reveal.payload = json!({"delivery_ref":delivery.event_id,"action":"reveal"}); store.append_teaching(&reveal).unwrap();
        drop(store);
        let visible = items(&state.user, &frozen, &session, &turn, NOW).unwrap();
        let visible = visible.iter().find(|item| item.item_id == hidden.item_id).unwrap();
        assert!(!visible.source_bindings.is_empty());
        let BoardTarget::Understanding { evidence_snapshot, .. } = &visible.target else { panic!() };
        assert_eq!(evidence_snapshot["feedback_hidden"], false); assert!(evidence_snapshot["source_quotes"].is_array());
        assert_eq!(evidence_snapshot["source_quotes"][0]["reason"], "PRIVATE_SCORING_REASON");
        assert_eq!(teaching::evidence_view(&state.private_context(), &performance.evidence_id).unwrap(), *evidence_snapshot);
    }

    #[test]
    fn wb3_board_understanding_keeps_current_run_correction_of_an_older_expression() {
        let (_root, mut state, frozen, board, session, old_turn) = fixture();
        let (original, original_fact) = original(&state);
        let retained_board = board_api::route(&state.user, &state.workspace, "import", &json!({
            "board_id":board.board_id,"board_revision":board.revision,
            "source":{"kind":"understanding","evidence_id":original.evidence_id}
        }), NOW).unwrap();
        let book_id = state.workspace.book.base.book_id.clone();
        let current = precommit_agent_turn(&mut state, &book_id,
            "请纠正刚才对我原话的解释，不要把它算成独立迁移。".into(), None, None, None, NOW).unwrap();
        assert_eq!(current.session_id, session);
        let mut binding = original_fact.binding.clone();
        binding.turn_id = current.turn_id.clone();
        let mut unrelated = original.clone();
        unrelated.evidence_id = "interpretation:not-accepted-this-run".into();
        state.user.learning_store().unwrap().append_evidence(&unrelated).unwrap();
        let accepted = teaching::accept_hypothesis_with_assistance(&state.private_context(), &binding, &json!({
            "operation_id":"correct-older-expression", "target":original.target,
            "fact_refs":[original_fact.event_id], "supersedes":original.evidence_id,
            "interpretation":"原话只解释了已展示例子，独立迁移仍然未知。",
            "teaching_implication":"保留这个连接，后续用另一个情境检查能否迁移。"
        }), &[], &[EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:vec![] }], &[]).unwrap();

        let added = items(&state.user, &frozen, &session, &current.turn_id, NOW).unwrap();
        assert_eq!(added.len(), 1, "only evidence actually accepted by this run belongs to its new board content");
        let BoardTarget::Understanding { evidence_id, learner_quote, interpretation, evidence_snapshot, .. } = &added[0].target else { panic!() };
        assert_eq!(evidence_id, accepted["evidence_ref"].as_str().unwrap());
        assert_eq!(learner_quote, &original.learner_quote);
        assert_eq!(interpretation, "原话只解释了已展示例子，独立迁移仍然未知。");
        assert_eq!(evidence_snapshot["supersedes"], original.evidence_id);
        assert_eq!(evidence_snapshot["expressions"][0]["source_turn_id"], old_turn);
        assert_eq!(evidence_snapshot["expressions"][0]["text"], original.learner_quote);
        assert_eq!(evidence_snapshot["expressions"][0]["occurred_at"], original_fact.occurred_at);
        let learning = state.user.learning_store().unwrap();
        assert_eq!(learning.evidence(&original.evidence_id).unwrap(), original, "the earlier interpretation stays immutable");
        let receipt = learning.teaching_event(&format!("evidence-accepted:{}:{}", current.turn_id, evidence_id)).unwrap();
        assert_eq!(receipt.kind, TeachingFact::EvidenceInterpreted);
        assert_eq!(receipt.binding.chat_session_id, current.session_id);
        assert_eq!(receipt.binding.turn_id, current.turn_id);
        assert_eq!(receipt.payload["evidence_ref"], *evidence_id);
        assert_eq!(receipt.payload["origin"], "system_interpretation");
        assert_eq!(receipt.learner_expression(), None, "accepting an interpretation adds no learner words or mastery evidence");
        assert_eq!(board_api::route(&state.user, &state.workspace, "read", &json!({"board_id":board.board_id}), NOW).unwrap()["document"],
            retained_board, "deriving a new card does not rewrite the prior retained board snapshot");
    }
}
