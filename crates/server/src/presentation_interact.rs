//! One claimed browser command per run. Waiting never holds a user/workspace lock.
use crate::{PrivateBookContext, ToolError};
use runtime::presentation_interact::*;
use runtime::run_context::CancellationToken;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Pending {
    command: Option<PresentationCommand>,
    claimed: bool,
    result: Option<Value>,
    observations: HashMap<String, (String, runtime::presentation::PresentationFollowUp)>,
    state_contract: Value,
    saved_result: Option<Value>,
}
#[derive(Default)]
pub(crate) struct PresentationCommands {
    pending: Mutex<Pending>,
    changed: Condvar,
}
impl PresentationCommands {
    fn prepare(
        &self, state: &PrivateBookContext<'_>, turn: &crate::AgentTurnRef, request: InteractRequest,
    ) -> Result<PresentationCommand, ToolError> {
        let delivery = crate::presentation_api::delivered_turn(state, &turn.session_id, request.reference())?;
        let version = crate::presentation_api::delivered_version(state, &turn.session_id, &delivery, request.reference())?;
        let mut pending = self.pending.lock().unwrap();
        let mut expected_values = BTreeMap::new();
        let mut expected_edits = BTreeMap::new();
        let mut page_id = None;
        if let InteractRequest::Act { based_on, action, .. } = &request {
            validate_action(action, &version.content.action_contract)?;
            let (page, receipt) = pending.observations.get(based_on).cloned().ok_or_else(||
                error("PRESENTATION_OBSERVATION_REQUIRED", "Observe this open page in the current run before acting."))?;
            if &receipt.reference != request.reference() { return Err(error("PRESENTATION_OBSERVATION_REQUIRED", "Observe this exact presentation version before acting.")); }
            page_id = Some(page);
            // Read the exact immutable snapshot, never the latest saved state.
            let saved = state.read_presentation_state(&receipt)?;
            let keys = match action {
                PresentationAction::SetParameters { values } => values.keys().cloned().collect(),
                PresentationAction::Invoke { .. } => parameter_names(&version.content.action_contract),
                _ => vec![],
            };
            for key in keys {
                expected_edits.insert(key.clone(), saved.state.values["parameter_edits"][&key].as_u64().unwrap_or(0));
                expected_values.insert(key.clone(), saved.state.values["page"][&key].clone());
            }
        }
        pending.state_contract = version.content.state_contract;
        let command = PresentationCommand {
            command_id: uuid::Uuid::now_v7().to_string(), run_turn_id: turn.turn_id.clone(),
            session_id: turn.session_id.clone(), delivery_turn_id: delivery,
            request, page_id, expected_values, expected_edits, available_actions: version.content.action_contract,
        };
        pending.command = Some(command.clone());
        pending.claimed = false;
        pending.result = None;
        pending.saved_result = None;
        Ok(command)
    }

    pub(crate) fn respond(&self, state: &PrivateBookContext<'_>, input: &Value) -> Result<Value, ToolError> {
        let mut pending = self.pending.lock().unwrap();
        let command = pending.command.clone().filter(|command|
            input["command_id"].as_str() == Some(&command.command_id)
                && input["run_turn_id"].as_str() == Some(&command.run_turn_id)
                && state.selected_chat == Some(command.session_id.as_str())
        ).ok_or_else(|| error("PRESENTATION_COMMAND_ENDED", "This page command has ended; observe again."))?;
        if input["operation"] == "claim" {
            if pending.claimed || pending.result.is_some() { return Ok(json!({"claimed":false})); }
            let page = input["page_id"].as_str().filter(|p| !p.is_empty());
            if page.is_none() || command.page_id.as_deref().is_some_and(|expected| page != Some(expected)) {
                pending.result = Some(json!({"status":"unavailable","reason":"The original page is closed or has changed. Open it and observe again."}));
                self.changed.notify_all();
                return Ok(json!({"claimed":false}));
            }
            pending.claimed = true;
            pending.command.as_mut().unwrap().page_id = page.map(str::to_owned);
            return Ok(json!({"claimed":true}));
        }
        if input["operation"] != "complete" || !pending.claimed
            || input["page_id"].as_str() != command.page_id.as_deref() || pending.result.is_some() {
            return Err(error("PRESENTATION_COMMAND_ENDED", "This command cannot accept another result."));
        }
        let status = input["status"].as_str().unwrap_or("failed");
        if !matches!(status, "observed" | "applied" | "conflict" | "failed" | "cancelled") {
            return Err(error("PRESENTATION_RESULT_INVALID", "Unknown page result."));
        }
        let success = matches!(status, "observed" | "applied" | "conflict");
        let mut result = json!({"status":status,"command_id":command.command_id,"actor":"teacher",
            "reference":command.request.reference(),"page_id":command.page_id,
            "state_contract":pending.state_contract,"available_actions":command.available_actions});
        if let Some(reason) = input["reason"].as_str() { result["reason"] = json!(reason); }
        if success || input.get("receipt").is_some_and(|v| !v.is_null()) {
            if (status == "applied" && !matches!(command.request, InteractRequest::Act { .. }))
                || (status == "observed" && !matches!(command.request, InteractRequest::Observe { .. })) {
                return Err(error("PRESENTATION_RESULT_INVALID", "Result does not match the requested operation."));
            }
            let receipt: runtime::presentation::PresentationFollowUp = serde_json::from_value(input["receipt"].clone())
                .map_err(|_| error("PRESENTATION_RESULT_INVALID", "A saved browser receipt is required."))?;
            if receipt.session_id != command.session_id || receipt.turn_id != command.delivery_turn_id
                || &receipt.reference != command.request.reference() {
                return Err(error("PRESENTATION_RESULT_INVALID", "The receipt belongs to another page."));
            }
            let saved = state.read_presentation_state(&receipt)?;
            pending.observations.insert(receipt.saved_state_ref.clone(), (command.page_id.clone().unwrap(), receipt.clone()));
            result["state"] = json!(saved.state);
            result["receipt"] = json!(receipt);
            result["saved_state_ref"] = json!(receipt.saved_state_ref);
        }
        if let InteractRequest::Act { based_on, action, .. } = command.request {
            result["based_on"] = json!(based_on);
            result["action"] = json!(action);
        }
        if let Some(saved) = &pending.saved_result {
            if saved["receipt"] != result["receipt"] || saved["status"] != result["status"] {
                return Err(error("PRESENTATION_RESULT_INVALID", "Complete using the saved action receipt."));
            }
            result["teaching"] = saved["teaching"].clone();
        }
        pending.result = Some(result);
        self.changed.notify_all();
        Ok(json!({"accepted":true}))
    }
}

pub(crate) fn save_browser_state(
    workspace: &crate::reader_workspace::ReaderWorkspace, state: &PrivateBookContext<'_>,
    input: &Value, now: &str,
) -> Result<Value, ToolError> {
    let stream = workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(unavailable)?;
    let mut pending = stream.presentation_commands.pending.lock().unwrap();
    let meta = &input["teacher_interaction"];
    let reference: runtime::presentation::PresentationRef = serde_json::from_value(input["reference"].clone())
        .map_err(|_| error("PRESENTATION_RESULT_INVALID", "A presentation reference is required."))?;
    let command = pending.command.clone().filter(|command|
        pending.claimed && pending.result.is_none()
            && meta["command_id"] == command.command_id && meta["run_turn_id"] == command.run_turn_id
            && meta["page_id"].as_str() == command.page_id.as_deref()
            && input["session_id"] == command.session_id && input["turn_id"] == command.delivery_turn_id
            && &reference == command.request.reference()
            && state.selected_chat == Some(command.session_id.as_str())
    ).ok_or_else(|| error("PRESENTATION_COMMAND_ENDED", "This page command has ended; observe again."))?;
    if let Some(saved) = &pending.saved_result { return Ok(saved["receipt"].clone()); }
    let reply = crate::presentation_api::save_state(state, &input.to_string());
    if reply.status != 200 {
        return Err(error("PRESENTATION_RESULT_INVALID", "The browser state could not be saved."));
    }
    let receipt: runtime::presentation::PresentationFollowUp = serde_json::from_str(&reply.body)
        .map_err(|_| error("PRESENTATION_RESULT_INVALID", "Missing saved state receipt."))?;
    let mut teaching = Value::Null;
    if meta["status"] == "applied" {
        if let InteractRequest::Act { based_on, .. } = &command.request {
            let (_, before) = pending.observations.get(based_on).ok_or_else(unavailable)?;
            teaching = crate::teaching::record_presentation_action(state, &command, before, &receipt, &meta["support"], now)?;
        } else {
            return Err(error("PRESENTATION_RESULT_INVALID", "An observation cannot be an applied action."));
        }
    }
    pending.saved_result = Some(json!({"receipt":receipt,"status":meta["status"],"teaching":teaching}));
    Ok(json!(receipt))
}

pub(crate) fn execute(
    stream: &crate::agent_stream::RunStream, request: InteractRequest,
    cancellation: &CancellationToken,
    prepare: impl FnOnce(&PresentationCommands, InteractRequest) -> Result<PresentationCommand, ToolError>,
    check_scene: impl Fn() -> Result<(), ToolError>,
) -> Result<Value, ToolError> {
    cancellation.check()?;
    let command = prepare(&stream.presentation_commands, request)?;
    stream.presentation_command(Some(&command));
    let deadline = Instant::now() + Duration::from_secs(30);
    let result = (|| {
        loop {
            cancellation.check()?;
            check_scene()?;
            let mut pending = stream.presentation_commands.pending.lock().unwrap();
            if let Some(result) = pending.result.take() { return Ok(result); }
            if Instant::now() >= deadline {
                return Err(error("PRESENTATION_COMMAND_TIMEOUT", "No saved completion arrived. Observe the current page before deciding what to do; do not retry this action automatically."));
            }
            pending = stream.presentation_commands.changed.wait_timeout(pending, Duration::from_millis(100)).unwrap().0;
            drop(pending);
        }
    })();
    stream.presentation_commands.pending.lock().unwrap().command = None;
    stream.presentation_command(None);
    result
}

pub(crate) fn prepare(
    commands: &PresentationCommands, state: &PrivateBookContext<'_>,
    turn: &crate::AgentTurnRef, request: InteractRequest,
) -> Result<PresentationCommand, ToolError> { commands.prepare(state, turn, request) }

pub(crate) fn respond(
    workspace: &crate::reader_workspace::ReaderWorkspace, state: &PrivateBookContext<'_>, input: &Value,
) -> Result<Value, ToolError> {
    let stream = workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(unavailable)?;
    let snapshot = stream.snapshot();
    if !matches!(snapshot.execution_state.as_str(), "running" | "waiting_model")
        || input["run_turn_id"] != snapshot.descriptor.turn_id {
        return Err(error("PRESENTATION_COMMAND_ENDED", "This run is no longer accepting page results."));
    }
    stream.presentation_commands.respond(state, input)
}
