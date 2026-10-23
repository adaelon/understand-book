//! A music action is complete only after the current browser player acknowledges it.
use crate::{AgentTurnRef, ToolError};
use base_schema::{MusicQuestionContext, MusicSourceManifest, PublishedBookRef};
use runtime::{music::*, run_context::CancellationToken};
use serde_json::{json, Value};
use std::{sync::{Condvar, Mutex}, time::{Duration, Instant}};
pub(crate) use crate::music_media::{load as manifest, reference as publication};

fn error(code: &str, message: impl Into<String>) -> ToolError {
    ToolError { error_code: code.into(), category: "conflict".into(), message: message.into() }
}
fn unavailable() -> ToolError { error("MUSIC_PLAYER_UNAVAILABLE", "Open the original music player before observing or operating it.") }

pub(crate) fn question(workspace: &crate::reader_workspace::ReaderWorkspace, input: &Value) -> Result<Option<MusicQuestionContext>, ToolError> {
    let Some(value) = input.get("music_question").filter(|v| !v.is_null()) else { return Ok(None); };
    let question: MusicQuestionContext = serde_json::from_value(value.clone())
        .map_err(|e| error("MUSIC_QUESTION_INVALID", e.to_string()))?;
    let reference = publication(&workspace.book_dir)?;
    let source = manifest(&workspace.book_dir)?;
    if source.book_id != workspace.book.base.book_id || workspace.publication.as_ref().is_some_and(|p| p.reference != reference) {
        return Err(unavailable());
    }
    question.validate(&source, &reference).map_err(|e| error("MUSIC_QUESTION_INVALID", e))?;
    if let Some(image) = &question.image {
        crate::video_draft::validate_image(image).map_err(|e| error("MUSIC_QUESTION_INVALID", e.message))?;
    }
    Ok(Some(question))
}

pub(crate) fn source_context(scope: &crate::run_scope::RunScope) -> Result<Option<Value>, ToolError> {
    if !scope.book_dir.join("music_source_manifest.json").exists() { return Ok(None); }
    let source = manifest(&scope.book_dir)?;
    let reference = publication(&scope.book_dir)?;
    if reference.book_id != source.book_id || scope.publication.as_ref().is_some_and(|p| p.reference != reference) {
        return Err(unavailable());
    }
    Ok(Some(json!({"publication":reference,"recording":source.recording,"work":source.work,
        "score":source.score,"parts":source.parts,"coverage":source.coverage,
        "question_context":scope.music_question,"tools":["music.read","music.interact"],
        "guidance":"The question_context freezes the original time range, score objects, occurrence and focus at submission. Later playback must not replace it. Prepared score facts, audio analyses, reference descriptions and teaching interpretations are different evidence kinds. Reading this data or a successful playback receipt does not mean the model listened to the original recording. Observe the player before acting, and claim success only from an applied browser receipt."})))
}

#[derive(Default)]
struct Pending {
    command: Option<MusicCommand>,
    player_id: Option<String>,
    claimed: bool,
    result: Option<MusicPlaybackReceipt>,
    observation: Option<(String, MusicPlaybackReceipt)>,
    manifest: Option<MusicSourceManifest>,
}
#[derive(Default)]
pub(crate) struct MusicCommands { pending: Mutex<Pending>, changed: Condvar }

fn request_reference(request: &MusicInteractRequest) -> (&PublishedBookRef, &base_schema::MusicRecordingRef) {
    match request { MusicInteractRequest::Observe { publication, recording } | MusicInteractRequest::Act { publication, recording, .. } => (publication, recording) }
}

impl MusicCommands {
    pub(crate) fn prepare(&self, scope: &crate::run_scope::RunScope, turn: &AgentTurnRef, request: MusicInteractRequest) -> Result<MusicCommand, ToolError> {
        let source = manifest(&scope.book_dir)?;
        let declared = publication(&scope.book_dir)?;
        let (reference, recording) = request_reference(&request);
        if reference != &declared || recording != &source.recording.source || declared.book_id != scope.book.base.book_id
            || scope.publication.as_ref().is_some_and(|p| p.reference != declared) { return Err(unavailable()); }
        request.validate(&source, &declared)?;
        let mut pending = self.pending.lock().unwrap();
        let player_id = if let MusicInteractRequest::Act { based_on, .. } = &request {
            let (player, _) = pending.observation.as_ref().filter(|(_, r)| &r.receipt_id == based_on
                && &r.publication == reference && &r.recording == recording)
                .ok_or_else(|| error("MUSIC_OBSERVATION_REQUIRED", "Observe this exact player and use its latest receipt before acting."))?;
            Some(player.clone())
        } else { None };
        let command = MusicCommand { command_id: uuid::Uuid::now_v7().to_string(), run_turn_id: turn.turn_id.clone(), session_id: turn.session_id.clone(), request };
        pending.command = Some(command.clone());
        pending.player_id = player_id;
        pending.claimed = false;
        pending.result = None;
        pending.manifest = Some(source);
        Ok(command)
    }

    fn respond(&self, selected_chat: Option<&str>, input: &Value) -> Result<Value, ToolError> {
        let mut pending = self.pending.lock().unwrap();
        let command = pending.command.clone().filter(|c| input["command_id"] == c.command_id && input["run_turn_id"] == c.run_turn_id
            && selected_chat == Some(c.session_id.as_str())).ok_or_else(|| error("MUSIC_COMMAND_ENDED", "This music command has ended; observe again."))?;
        if input["operation"] == "claim" {
            if pending.claimed || pending.result.is_some() { return Ok(json!({"claimed":false})); }
            let (reference, recording) = request_reference(&command.request);
            let player = input["player_id"].as_str().filter(|s| !s.is_empty());
            if player.is_none() || pending.player_id.as_deref().is_some_and(|id| Some(id) != player)
                || input["publication"] != json!(reference) || input["recording"] != json!(recording) {
                pending.command = None;
                self.changed.notify_all();
                return Ok(json!({"claimed":false}));
            }
            pending.player_id = player.map(str::to_owned);
            pending.claimed = true;
            return Ok(json!({"claimed":true}));
        }
        if input["operation"] != "complete" || !pending.claimed || pending.result.is_some()
            || input["player_id"].as_str() != pending.player_id.as_deref() {
            return Err(error("MUSIC_COMMAND_ENDED", "This music command cannot accept another result."));
        }
        let mut receipt: MusicPlaybackReceipt = serde_json::from_value(input["receipt"].clone())
            .map_err(|e| error("MUSIC_RESULT_INVALID", e.to_string()))?;
        validate_receipt(&command, &receipt, pending.manifest.as_ref().unwrap())?;
        receipt.receipt_id = uuid::Uuid::now_v7().to_string();
        if receipt.status != MusicReceiptStatus::Failed {
            pending.observation = Some((pending.player_id.clone().unwrap(), receipt.clone()));
        } else {
            pending.observation = None;
        }
        pending.result = Some(receipt.clone());
        self.changed.notify_all();
        Ok(json!({"accepted":true,"receipt":receipt}))
    }
}

fn valid_range(start: f64, end: f64, duration: f64) -> bool { start.is_finite() && end.is_finite() && start >= 0.0 && start < end && end <= duration }
fn validate_receipt(command: &MusicCommand, receipt: &MusicPlaybackReceipt, source: &MusicSourceManifest) -> Result<(), ToolError> {
    let (reference, recording) = request_reference(&command.request);
    receipt.validate(command)?;
    // The main recording position stays on its original timeline while the
    // separate score-synthesis player starts, pauses or finishes.
    let demonstration = receipt.demonstration_id.as_ref()
        .and_then(|id| source.demonstrations.iter().find(|d| &d.demonstration_id == id));
    let demo_state_valid = match (&receipt.demonstration_id, &receipt.demonstration_state, demonstration) {
        (None, None, None) => true,
        (Some(_), Some(state), Some(asset)) => state.current_ms.is_finite()
            && state.current_ms >= 0.0 && state.current_ms <= asset.duration_ms,
        _ => false,
    };
    if receipt.command_id != command.command_id || &receipt.publication != reference || &receipt.recording != recording
        || !receipt.current_ms.is_finite() || receipt.current_ms < 0.0 || receipt.current_ms > source.recording.duration_ms
        || !receipt.playback_rate.is_finite() || receipt.playback_rate < 0.5 || receipt.playback_rate > 2.0
        || !receipt.focused_part_ids.iter().all(|id| source.parts.iter().any(|p| &p.part_id == id))
        || receipt.focused_voices.iter().any(|voice| voice.validate(source).is_err())
        || !demo_state_valid
        || receipt.displayed_cue.as_ref().is_some_and(|cue| !source.events.iter()
            .any(|event| event.event_id == cue.event_id && event.listen_for == cue.text))
        || receipt.occurrence_id.as_ref().is_some_and(|id| !source.occurrences.iter().any(|o| &o.occurrence_id == id))
        || receipt.loop_range.as_ref().is_some_and(|r| !valid_range(r.start_ms, r.end_ms, source.recording.duration_ms)) {
        return Err(error("MUSIC_RESULT_INVALID", "The browser result does not match the original player and command."));
    }
    if receipt.status == MusicReceiptStatus::Applied {
        if let MusicInteractRequest::Act { action, .. } = &command.request {
            let cue_matches = |requested: &Option<String>| requested.as_ref().is_none_or(|id|
                receipt.displayed_cue.as_ref().is_some_and(|cue| &cue.event_id == id));
            let applied = match action {
                MusicAction::Play { start_ms, end_ms, cue_event_id } => !receipt.paused
                    && receipt.demonstration_state.as_ref().is_none_or(|state| state.paused)
                    && (start_ms.is_none() && end_ms.is_none() || receipt.loop_range.is_none())
                    && cue_matches(cue_event_id),
                MusicAction::Demonstration { demonstration_id, cue_event_id } => receipt.paused
                    && receipt.demonstration_id.as_ref() == Some(demonstration_id)
                    && receipt.demonstration_state.as_ref().is_some_and(|state| !state.paused)
                    && cue_matches(cue_event_id),
                MusicAction::Pause => receipt.paused && receipt.demonstration_state.as_ref().is_none_or(|state| state.paused),
                MusicAction::Rate { rate } => (receipt.playback_rate - rate).abs() < 0.001,
                MusicAction::Loop { start_ms, end_ms } => receipt.loop_range.as_ref().is_some_and(|r| r.start_ms == *start_ms && r.end_ms == *end_ms),
                MusicAction::ClearLoop => receipt.loop_range.is_none(),
                MusicAction::Focus { part_ids, voices } => receipt.focused_part_ids == *part_ids && receipt.focused_voices == *voices,
                MusicAction::Seek { .. } => true,
            };
            if !applied { return Err(error("MUSIC_RESULT_INVALID", "The requested playback action is not reflected in the browser result.")); }
        }
    }
    Ok(())
}

pub(crate) fn execute(stream: &crate::agent_stream::RunStream, request: MusicInteractRequest, cancellation: &CancellationToken,
    prepare: impl FnOnce(&MusicCommands, MusicInteractRequest) -> Result<MusicCommand, ToolError>,
    check_scene: impl Fn() -> Result<(), ToolError>,
) -> Result<MusicPlaybackReceipt, ToolError> {
    cancellation.check()?;
    let command = prepare(&stream.music_commands, request)?;
    stream.music_command(Some(&command));
    let deadline = Instant::now() + Duration::from_secs(30);
    let result = (|| loop {
        cancellation.check()?;
        check_scene()?;
        let mut pending = stream.music_commands.pending.lock().unwrap();
        if let Some(result) = pending.result.take() { return Ok(result); }
        if pending.command.is_none() { return Err(unavailable()); }
        if Instant::now() >= deadline { return Err(error("MUSIC_COMMAND_TIMEOUT", "No player completion arrived. Observe again before deciding the next action; do not automatically retry playback.")); }
        pending = stream.music_commands.changed.wait_timeout(pending, Duration::from_millis(100)).unwrap().0;
        drop(pending);
    })();
    let mut pending = stream.music_commands.pending.lock().unwrap();
    pending.command = None;
    if result.is_err() { pending.observation = None; }
    drop(pending);
    stream.music_command(None);
    result
}

pub(crate) fn respond(workspace: &crate::reader_workspace::ReaderWorkspace, input: &Value) -> Result<Value, ToolError> {
    let stream = workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(unavailable)?;
    let snapshot = stream.snapshot();
    if !matches!(snapshot.execution_state.as_str(), "running" | "waiting_model") || input["run_turn_id"] != snapshot.descriptor.turn_id {
        return Err(error("MUSIC_COMMAND_ENDED", "This run no longer accepts music results."));
    }
    stream.music_commands.respond(workspace.selected_chat.as_deref(), input)
}
