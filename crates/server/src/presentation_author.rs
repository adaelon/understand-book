//! Short private storage borrows surround a browser rehearsal outside AppState's lock.
use crate::{
    agent_run::{AppStatePort, RuntimeStatePort},
    *,
};
use runtime::{
    presentation::*, presentation_author::*, presentation_preview::*,
    run_context::CancellationToken,
};

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "PRESENTATION_AUTHORING_FAILED".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

fn missing_preview_environments(receipts: &std::collections::HashSet<String>) -> Vec<String> {
    REQUIRED_PREVIEW_ENVIRONMENTS
        .iter()
        .map(|(name, _)| (*name).to_string())
        .filter(|name| !receipts.contains(name))
        .collect()
}

fn preview_contract_complete(receipts: &std::collections::HashSet<String>) -> bool {
    receipts.contains("legacy") || missing_preview_environments(receipts).is_empty()
}

fn plot_bytes(p: &crate::presentation_plot::PlotAsset) -> usize {
    p.svg.len() + p.png_base64.len() + p.code.len() + p.data.to_string().len()
}
fn animation_bytes(a: &crate::presentation_animation::RenderedAnimation) -> usize {
    a.asset.video_base64.len() + a.frames.iter().map(|(_, png)| png.len()).sum::<usize>()
        + a.code.len() + a.data.to_string().len()
}

pub(crate) trait AuthorStorage {
    fn with_private<R>(&self, operation: impl FnOnce(&PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError>;
}
struct LocalStorage<'a, P> { port: &'a P, scope: &'a crate::run_scope::RunScope }
impl<P: AppStatePort> AuthorStorage for LocalStorage<'_, P> {
    fn with_private<R>(&self, operation: impl FnOnce(&PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError> {
        self.port.with_app(|state| { self.scope.check_owner(state)?; operation(&self.scope.private_context(state)) })
    }
}
pub(crate) struct AuthorSession<'a, H> {
    pub storage: &'a H,
    pub turn_ref: &'a AgentTurnRef,
    pub previewed: &'a mut std::collections::HashMap<String, std::collections::HashSet<String>>,
    pub animations: &'a mut std::collections::HashMap<String, crate::presentation_animation::RenderedAnimation>,
    pub plots: &'a mut std::collections::HashMap<String, crate::presentation_plot::PlotAsset>,
    pub sandbox: Option<crate::presentation_sandbox::Execution<'a>>,
}
impl<P: AppStatePort> RuntimeStatePort<'_, P> {
    pub(crate) fn author(&mut self, request: AuthorRequest, bindings: &[SourceBinding], messages: &[Message], cancellation: &CancellationToken) -> Result<AuthorResult, ToolError> {
        AuthorSession { storage: &LocalStorage { port: self.port, scope: self.scope }, turn_ref: self.turn_ref,
            previewed: &mut self.previewed, animations: &mut self.animations, plots: &mut self.plots, sandbox: None,
        }.author(request, bindings, messages, cancellation)
    }
}
impl<H: AuthorStorage> AuthorSession<'_, H> {
    fn media_limit(&self, added: usize) -> Result<(), ToolError> {
        if self.sandbox.is_none() { return Ok(()); }
        let bytes = self.plots.values().map(plot_bytes).sum::<usize>()
            + self.animations.values().map(animation_bytes).sum::<usize>();
        if self.plots.len() + self.animations.len() >= 8 || bytes + added > 32 * 1024 * 1024 {
            return Err(crate::user_storage_paths::error("PRESENTATION_ASSET_LIMIT", "rate_limit", "This turn's temporary media limit is reached"));
        }
        Ok(())
    }
    fn with_private<R>(&self, operation: impl FnOnce(&PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError> {
        self.storage.with_private(operation)
    }

    fn edit_base(&self, reference: &Option<PresentationRef>, candidate_id: &Option<String>) -> Result<(PresentationContent, Option<PresentationRef>), ToolError> {
        self.with_private(|state| match (reference, candidate_id) {
            (Some(reference), None) => {
                let version = state.read_presentation(&self.turn_ref.session_id, reference).or_else(|original| {
                    let receipt = state.user.agent_history.sessions.iter().find(|s| s.id == self.turn_ref.session_id)
                        .and_then(|s| s.turns.iter().find(|t| t.turn_id == self.turn_ref.turn_id))
                        .and_then(|t| t.presentation_follow_up.as_ref()).filter(|r| &r.reference == reference).ok_or(original)?;
                    let owner = runtime::presentation::PresentationOwner { book_id: state.book.base.book_id.clone(), session_id: receipt.session_id.clone() };
                    let store = presentation_store::PresentationStore::for_user(state.user)?;
                    store.read_retained_state(&owner, receipt)?;
                    store.read_version(&owner, reference)
                })?;
                Ok((version.content, Some(reference.clone())))
            }
            (None, Some(id)) => {
                let candidate = state.read_presentation_candidate(&self.turn_ref.session_id, id)?;
                if candidate.created_by_turn_id != self.turn_ref.turn_id { return Err(invalid("Candidate belongs to a different run")); }
                Ok((candidate.content, candidate.based_on))
            }
            _ => Err(invalid("Provide exactly one reference or candidate_id")),
        })
    }

    pub(crate) fn author(
        &mut self,
        request: AuthorRequest,
        bindings: &[SourceBinding],
        messages: &[Message],
        cancellation: &CancellationToken,
    ) -> Result<AuthorResult, ToolError> {
        cancellation.check()?;
        if self.sandbox.is_some() && matches!(&request, AuthorRequest::RenderPlot {..} | AuthorRequest::RenderAnimation {..}) {
            self.media_limit(0)?;
        }
        let mut result = AuthorResult {
            body: Value::Null,
            images: vec![],
            previewed_candidate: None,
            delivered: None,
        };
        match request {
            AuthorRequest::Prepare { .. } => return Err(invalid("prepare is handled by the active Runtime run")),
            AuthorRequest::RenderAnimation { code, data, size, cues } => {
                let rendered = if let Some(executor) = &self.sandbox {
                    executor.run(crate::presentation_sandbox::Job::Animation { code, data, size, cues }, cancellation)?
                } else { crate::presentation_animation::render(code, data, size, cues, cancellation)? };
                self.media_limit(animation_bytes(&rendered))?;
                let id = format!("animation-{}", uuid::Uuid::now_v7());
                result.body = crate::presentation_animation::metadata(&id, &rendered.asset);
                result.body["status"] = json!("animation_rendered");
                result.body["preview_images"] = json!(rendered.frames.iter().map(|(time,_)| json!({"at_seconds":time})).collect::<Vec<_>>());
                for (time, png) in &rendered.frames {
                    result.images.push(PreviewImage { caption:format!("Animation {id}: decoded frame at {time:.3}s. Inspect before embedding and previewing the page."), png_base64:png.clone(), candidate_id:None, environment_name:None });
                }
                self.animations.insert(id, rendered);
            }
            AuthorRequest::RenderPlot { code, data, size } => {
                let plot = if let Some(executor) = &self.sandbox {
                    executor.run(crate::presentation_sandbox::Job::Plot { code, data, size }, cancellation)?
                } else { crate::presentation_plot::render(code, data, size, cancellation)? };
                self.media_limit(plot_bytes(&plot))?;
                let asset_ref = format!("plot-{}", uuid::Uuid::now_v7());
                let asset_path = format!("assets/{asset_ref}.svg");
                result.images.push(PreviewImage {
                    caption: format!("Matplotlib plot {asset_ref}, {}x{}; inspect the actual labels, axes, values and legend before using it.", plot.width, plot.height),
                    png_base64: plot.png_base64.clone(),
                    candidate_id: None,
                    environment_name: None,
                });
                result.body = json!({"status":"plot_rendered","asset_ref":asset_ref,"asset_path":asset_path,
                    "mime":"image/svg+xml","width":plot.width,"height":plot.height,"font":plot.font,
                    "next":"Inspect the returned image, then use asset_path in an img src and include asset_ref in write.asset_refs"});
                self.plots.insert(asset_ref, plot);
            }
            AuthorRequest::Read { reference, candidate_id, file, offset, length } => {
                let (content, based_on) = self.edit_base(&reference, &candidate_id)?;
                let library_metadata = crate::presentation_libraries::metadata(&content.content_files);
                let animations: Vec<_> = content.animation_assets.iter().map(|(id,a)| crate::presentation_animation::metadata(id,a)).collect();
                let explicit_file = file.is_some();
                let file = file.unwrap_or_else(|| content.entrypoint.clone());
                if let Some(asset) = animations.iter().find(|a| a["asset_path"] == file || a["poster_path"] == file) {
                    result.body = json!({"status":"animation_metadata","reference":reference,"candidate_id":candidate_id,"animation":asset});
                    return Ok(result);
                }
                let source = if file == "readable_content" { &content.readable_content }
                    else { content.content_files.get(&file).ok_or_else(|| invalid("Content file not found"))? };
                if file.starts_with("libraries/") {
                    result.body = json!({"status":"managed_library","reference":reference,"candidate_id":candidate_id,"file":file,"libraries":library_metadata,
                        "next":"Use write.libraries to select dependencies; managed source is not authoring text."});
                    return Ok(result);
                }
                result.body = crate::presentation_source::read(source, offset, length).map_err(invalid)?;
                let metadata = json!({"status":"version_read","reference":reference,"candidate_id":candidate_id,"based_on":based_on,
                    "title":content.title,"libraries":library_metadata,"animations":animations,"files":content.content_files.keys().collect::<Vec<_>>(),"entrypoint":content.entrypoint,
                    "asset_refs":content.content_files.keys().filter_map(|path| path.strip_prefix("assets/").and_then(|name| name.strip_suffix(".svg"))).chain(content.animation_assets.keys().map(String::as_str)).collect::<Vec<_>>(),
                    "file":file,"assumptions":content.assumptions,"source_ref_ids":content.source_bindings.iter().map(|b| &b.source_ref_id).collect::<Vec<_>>(),
                    "state_contract":content.state_contract,"initial_state":content.initial_state});
                result.body.as_object_mut().unwrap().extend(metadata.as_object().unwrap().clone());
                if !explicit_file { result.body["readable_content"] = json!(content.readable_content); }
            }
            AuthorRequest::Search { reference, candidate_id, file, query, offset, max_matches } => {
                let (content, _) = self.edit_base(&reference, &candidate_id)?;
                let file = file.unwrap_or_else(|| content.entrypoint.clone());
                if file.starts_with("libraries/") { return Err(invalid("Managed libraries are not editable source")); }
                let source = if file == "readable_content" { &content.readable_content }
                    else { content.content_files.get(&file).ok_or_else(|| invalid("Content file not found"))? };
                result.body = crate::presentation_source::search(source, &query, offset, max_matches).map_err(invalid)?;
                for (key, value) in [("status",json!("source_matches")),("reference",json!(reference)),("candidate_id",json!(candidate_id)),("file",json!(file))] {
                    result.body[key] = value;
                }
            }
            AuthorRequest::Patch { reference, candidate_id, edits, title, readable_content, state_contract, initial_state } => {
                let (mut content, based_on) = self.edit_base(&reference, &candidate_id)?;
                let old_contract = content.state_contract.clone();
                let html = crate::presentation_source::patch(&content.content_files[&content.entrypoint], &edits).map_err(invalid)?;
                content.content_files.insert(content.entrypoint.clone(), html);
                if let Some(title) = title { content.title = title; }
                if let Some(text) = readable_content { content.readable_content = text; }
                if let Some(contract) = state_contract { content.state_contract = contract; }
                if let Some(initial) = initial_state { content.initial_state = initial; }
                if content.content_files.values().map(String::len).sum::<usize>() > 1024 * 1024 {
                    return Err(invalid("Candidate and version assets exceed 1 MiB"));
                }
                // Version patches inherit the exact follow-up receipt, like write.
                // Candidate patches already carry the parameters chosen by that draft.
                let saved = self.with_private(|state| {
                    let receipt = state.user.agent_history.sessions.iter().find(|s| s.id == self.turn_ref.session_id)
                        .and_then(|s| s.turns.iter().find(|t| t.turn_id == self.turn_ref.turn_id))
                        .and_then(|t| t.presentation_follow_up.as_ref());
                    // An explicitly new current-run candidate has no delivered base.
                    if receipt.is_some_and(|r| based_on.as_ref().is_some_and(|base| &r.reference != base)) {
                        return Err(invalid("Edit the exact version selected by the presentation follow-up receipt"));
                    }
                    match &reference {
                        Some(reference) => match receipt {
                            Some(receipt) => state.read_presentation_state(receipt).map(Some),
                            None => state.latest_presentation_state(&self.turn_ref.session_id, reference),
                        },
                        None => Ok(None),
                    }
                })?;
                if let Some(saved) = saved { inherit_parameters(&mut content.initial_state, &content.state_contract, &old_contract, &saved.state); }
                validate_content_semantics(&content, messages)?;
                let candidate = self.with_private(|state| state.create_presentation_candidate(
                    &self.turn_ref.session_id, &self.turn_ref.turn_id, based_on, content))?;
                result.body = json!({"status":"candidate_saved","candidate_id":candidate.candidate_id,"based_on":candidate.based_on,
                    "initial_state":candidate.content.initial_state,"applied_edits":edits.len(),
                    "next":"Preview and inspect this new candidate in all required environments before deliver. The base and its preview receipts are unchanged."});
            }
            AuthorRequest::Write {
                based_on,
                new_object,
                state_contract,
                title,
                html,
                readable_content,
                asset_refs,
                libraries,
                source_ref_ids,
                assumptions,
                mut initial_state,
            } => {
                if new_object && based_on.is_some() {
                    return Err(invalid("write.new_object=true conflicts with based_on; choose a new object or an exact version revision"));
                }
                let base_and_saved = self.with_private(|state| {
                    let receipt = state.user.agent_history.sessions.iter().find(|s| s.id == self.turn_ref.session_id)
                        .and_then(|s| s.turns.iter().find(|t| t.turn_id == self.turn_ref.turn_id))
                        .and_then(|t| t.presentation_follow_up.as_ref());
                    if let Some(receipt) = receipt {
                        if !new_object && based_on.as_ref() != Some(&receipt.reference) {
                            return Err(invalid(format!("Set write.based_on to the exact presentation follow-up reference {} to revise it, or omit based_on and set new_object=true for an intentionally separate presentation", json!(receipt.reference))));
                        }
                    }
                    based_on.as_ref().map(|reference| {
                        let base = state.read_presentation(&self.turn_ref.session_id, reference)?;
                        let saved = match receipt {
                            Some(receipt) => Some(state.read_presentation_state(receipt)?),
                            None => state.latest_presentation_state(&self.turn_ref.session_id, reference)?,
                        };
                        Ok::<_, ToolError>((base, saved))
                    }).transpose()
                })?;
                #[cfg(test)]
                let html = crate::tests::presentation_author_tests::ex10::assemble_write(&html);
                if html.len() > 1024 * 1024 {
                    return Err(invalid("HTML exceeds 1 MiB"));
                }
                let mut available_bindings = bindings.to_vec();
                let mut animation_assets = std::collections::BTreeMap::new();
                let mut content_files =
                    std::collections::BTreeMap::from([("index.html".to_string(), html.clone())]);
                if let Some((base, saved)) = base_and_saved {
                    if let Some(saved) = saved {
                        inherit_parameters(
                            &mut initial_state,
                            &state_contract,
                            &base.content.state_contract,
                            &saved.state,
                        );
                    }
                    animation_assets.extend(base.content.animation_assets.into_iter().filter(|(id,_)| asset_refs.contains(id)));
                    content_files.extend(base.content.content_files.into_iter().filter(
                        |(path, _)| {
                            if path == "index.html" {
                                return false;
                            }
                            if path.starts_with("assets/") || path.starts_with("plots/") || path.starts_with("animations/") {
                                return asset_refs.iter().any(|id| {
                                    path == &format!("assets/{id}.svg")
                                        || path == &format!("plots/{id}.py")
                                        || path == &format!("plots/{id}.json")
                                        || path == &format!("animations/{id}.py")
                                        || path == &format!("animations/{id}.json")
                                });
                            }
                            true
                        },
                    ));
                    for binding in base.content.source_bindings {
                        if !available_bindings
                            .iter()
                            .any(|b| b.source_ref_id == binding.source_ref_id)
                        {
                            available_bindings.push(binding);
                        }
                    }
                }
                for asset_ref in &asset_refs {
                    if let Some(rendered) = self.animations.get(asset_ref) {
                        animation_assets.insert(asset_ref.clone(), rendered.asset.clone());
                        content_files.insert(format!("animations/{asset_ref}.py"), rendered.code.clone());
                        content_files.insert(format!("animations/{asset_ref}.json"), rendered.data.to_string());
                    }
                    if animation_assets.contains_key(asset_ref) {
                        let path = format!("assets/{asset_ref}.mp4");
                        if !html.contains(&format!("src=\"{path}\"")) && !html.contains(&format!("src='{path}'")) {
                            return Err(invalid(format!("Use {path} as a video src")));
                        }
                        continue;
                    }
                    let path = format!("assets/{asset_ref}.svg");
                    if !html.contains(&format!("src=\"{path}\""))
                        && !html.contains(&format!("src='{path}'"))
                    {
                        return Err(invalid(format!("Use {path} as an img src")));
                    }
                    if let Some(plot) = self.plots.get(asset_ref) {
                        content_files.insert(path, plot.svg.clone());
                        content_files.insert(format!("plots/{asset_ref}.py"), plot.code.clone());
                        content_files
                            .insert(format!("plots/{asset_ref}.json"), plot.data.to_string());
                    } else if !content_files.contains_key(&path) {
                        return Err(invalid(format!("Unknown asset_ref: {asset_ref}")));
                    }
                }
                crate::presentation_animation::validate_assets(&animation_assets)?;
                // Detect omitted animation refs at write time, before saving an unusable candidate.
                for extension in ["mp4", "png"] {
                    for part in html.split("assets/animation-").skip(1) {
                        let name = part.split(['\"', '\'', '<', '>', ' ']).next().unwrap_or("");
                        if let Some(id) = name.strip_suffix(&format!(".{extension}")) {
                            if !animation_assets.contains_key(&format!("animation-{id}")) {
                                return Err(invalid("Include each referenced animation in asset_refs"));
                            }
                        }
                    }
                }
                let html = crate::presentation_libraries::assemble(&html, &libraries, &mut content_files);
                content_files.insert("index.html".into(), html);
                if content_files.values().map(String::len).sum::<usize>() > 1024 * 1024 {
                    return Err(invalid("Candidate and version assets exceed 1 MiB"));
                }
                let content = PresentationContent {
                    animation_assets,
                    title,
                    content_files,
                    entrypoint: "index.html".into(),
                    readable_content,
                    source_bindings: bindings_for(&source_ref_ids, &available_bindings)?,
                    assumptions,
                    state_contract,
                    initial_state,
                };
                validate_content_semantics(&content, messages)?;
                let candidate = self.with_private(|state| {
                    state.create_presentation_candidate(
                        &self.turn_ref.session_id,
                        &self.turn_ref.turn_id,
                        based_on,
                        content,
                    )
                })?;
                result.body = json!({"candidate_id":candidate.candidate_id,"based_on":candidate.based_on,"initial_state":candidate.content.initial_state,"status":"candidate_saved","libraries":crate::presentation_libraries::metadata(&candidate.content.content_files),"next":"Preview with real actions and inspect screenshots. For local changes, search/read/patch this candidate_id. Patch preserves assets and creates a new candidate; preview it again. Use write with full HTML for broad rewrites; based_on is only for a delivered presentation reference. Deliver the final candidate after its required previews."});
            }
            AuthorRequest::Preview {
                candidate_id,
                read_selector,
                width,
                viewport,
                actions,
            } => {
                if actions.len() > 4 {
                    return Err(invalid("At most four actions per rehearsal; preview further paths in a new rehearsal"));
                }
                for action in &actions {
                    action.scene_position().map_err(invalid)?;
                }
                let candidate = self.with_private(|state| {
                    state.read_presentation_candidate(&self.turn_ref.session_id, &candidate_id)
                })?;
                if candidate.created_by_turn_id != self.turn_ref.turn_id {
                    return Err(invalid("Candidate belongs to a different run"));
                }
                let html = preview_document(&candidate.content);
                let request = PreviewRequest {
                    candidate_id: candidate_id.clone(),
                    read_selector,
                    html,
                    actions,
                    width,
                    viewport,
                };
                let environment = request.environment().map_err(invalid)?;
                let environment_name = preview_environment_name(environment);
                let legacy_request = request.viewport.is_none();
                let report: PreviewReport = if let Some(executor) = &self.sandbox {
                    executor.run(crate::presentation_sandbox::Job::Preview { request }, cancellation)?
                } else {
                    crate::presentation_preview::BrowserPreview::discover().map_err(invalid)?
                        .preview(&request, cancellation).map_err(|error| invalid(format!("{}: {}", error.phase, error.message)))?
                };
                cancellation.check()?;
                let mut problems = report
                    .errors
                    .iter()
                    .map(|error| {
                        json!({"kind":"candidate_execution","message":error,"environment":report.environment_name})
                    })
                    .collect::<Vec<_>>();
                for observation in &report.observations {
                    problems.extend(observation.issues.iter().map(|issue| {
                        json!({"step":observation.step,"kind":issue.kind,"message":issue.message,"environment":report.environment_name})
                    }));
                    if let Err(error) =
                        validate_observation(&candidate.content, &observation.dom, messages)
                    {
                        problems.push(json!({"step":observation.step,"kind":"result_mismatch","message":error.message,"environment":report.environment_name}));
                    }
                }
                let clean = problems.is_empty() && !report.observations.is_empty();
                let receipt = if legacy_request {
                    "legacy".to_string()
                } else {
                    environment_name.clone()
                };
                if clean {
                    self.previewed
                        .entry(candidate_id.clone())
                        .or_default()
                        .insert(receipt.clone());
                } else if let Some(receipts) = self.previewed.get_mut(&candidate_id) {
                    receipts.remove(&receipt);
                }
                let receipts = self
                    .previewed
                    .get(&candidate_id)
                    .cloned()
                    .unwrap_or_default();
                let missing_environments = missing_preview_environments(&receipts);
                let complete = clean && preview_contract_complete(&receipts);
                let status = if !clean {
                    "preview_failed"
                } else if complete {
                    "preview_ready_for_inspection"
                } else {
                    "preview_environment_recorded"
                };
                let mut recorded_environments = receipts.into_iter().collect::<Vec<_>>();
                recorded_environments.sort();
                for observation in &report.observations {
                    let scene = observation.scene.as_ref().map(|scene| format!(" Target scene: semantic step {}, transition {:.3}; actual: semantic step {}, transition {:.3}, paused {}.", scene.target.semantic_state, scene.target.transition_progress, scene.actual.semantic_state, scene.actual.transition_progress, !scene.actual.playing)).unwrap_or_default();
                    let position = format!(" Action: {}; actual document position x={}, y={} CSS px (max y={}), viewport {}x{} CSS px.", serde_json::to_string(&observation.action).unwrap(), observation.scroll.x, observation.scroll.y, observation.scroll.max_y, observation.scroll.viewport_width, observation.scroll.viewport_height);
                    result.images.push(PreviewImage { caption: format!("Browser observation: candidate {candidate_id}, environment {} ({}x{} {:?}), step {}, status {status}.{position}{scene} Inspect layout, graphics and agreement with readable content before delivery.", report.environment_name, report.environment.width, report.environment.height, report.environment.input, observation.step), png_base64: observation.screenshot_png_base64.clone(), candidate_id: Some(candidate_id.clone()), environment_name: Some(report.environment_name.clone()) });
                }
                if complete {
                    result.previewed_candidate = Some(candidate_id.clone());
                }
                result.body = json!({"candidate_id":candidate_id,"status":status,"errors":problems,
                    "environment_name":report.environment_name,"environment":report.environment,"recorded_environments":recorded_environments,"missing_environments":missing_environments,
                    "observations":report.observations.iter().map(|o| json!({"step":o.step,"action":o.action,"scroll":o.scroll,"dom":o.dom,"layout":o.layout,"issues":o.issues,"scene":o.scene})).collect::<Vec<_>>()});
                if let Some(reading) = report.observations.last().and_then(|o| o.reading.as_ref()) {
                    result.body["reading"] = reading.clone();
                }
                if !clean {
                    result.body["error_code"] = json!("PRESENTATION_PREVIEW_FAILED");
                    result.body["category"] = json!("execution");
                }
            }
            AuthorRequest::Deliver { candidate_id } => {
                let ready = self
                    .previewed
                    .get(&candidate_id)
                    .is_some_and(preview_contract_complete);
                if !ready {
                    return Err(invalid(
                        "Preview this exact candidate successfully before delivery",
                    ));
                }
                let reference = self.with_private(|state| {
                    let candidate = state.read_presentation_candidate(&self.turn_ref.session_id, &candidate_id)?;
                    validate_content_semantics(&candidate.content, messages)?;
                    cancellation.check()?;
                    state.persist_presentation_candidate(
                        &self.turn_ref.session_id,
                        &self.turn_ref.turn_id,
                        &candidate_id,
                    )
                })?;
                result.body = json!({"status":"version_saved","reference":reference,"next":"Conclude normally; Runtime attaches this version to the final answer. History commit is still pending."});
                result.delivered = Some(reference);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod preview_contract_tests {
    use super::*;

    #[test]
    fn explicit_preview_contract_requires_all_three_bound_environments() {
        let mut receipts = std::collections::HashSet::new();
        receipts.insert("narrow-content".to_string());
        receipts.insert("short-content".to_string());
        assert_eq!(
            missing_preview_environments(&receipts),
            vec!["desktop-content"]
        );
        assert!(!preview_contract_complete(&receipts));
        receipts.insert("desktop-content".to_string());
        assert!(preview_contract_complete(&receipts));
    }

    #[test]
    fn legacy_preview_receipt_remains_deliverable() {
        assert!(preview_contract_complete(&["legacy".to_string()].into()));
    }
}

fn inherit_parameters(
    initial: &mut Value,
    contract: &Value,
    old_contract: &Value,
    saved: &PresentationState,
) {
    let (Some(defaults), Some(parameters)) = (
        initial.as_object_mut(),
        saved.values.get("page").and_then(Value::as_object),
    ) else {
        return;
    };
    for (key, default) in defaults {
        let Some(definition) = contract
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        else {
            continue;
        };
        if old_contract.get(key).and_then(Value::as_str) != Some(definition) {
            continue;
        }
        if let Some(value) = parameters.get(key) {
            let same_type = matches!(
                (&*default, value),
                (Value::Bool(_), Value::Bool(_))
                    | (Value::Number(_), Value::Number(_))
                    | (Value::String(_), Value::String(_))
            );
            if same_type {
                *default = value.clone();
            }
        }
    }
}

fn compile(
    text: &str,
    content: &PresentationContent,
    messages: &[Message],
    allow_refs: bool,
) -> Result<(), ToolError> {
    let view =
        runtime::orchestrator::compile_presentation_text(text, &content.source_bindings, messages)
            .map_err(|issues| {
                let hint = if issues.iter().any(|issue| issue.error_code == "UNKNOWN_SOURCE_REF") {
                    " Include every cited ref in write.source_ref_ids (or patch.source_ref_ids when adding sources). A ref already observed through source.present or preserved by based_on need not be registered again; it must be explicitly attached to this candidate. Use only observed or preserved refs."
                } else { "" };
                invalid(format!("Public content rejected: {issues:?}{hint}"))
            })?;
    if !allow_refs
        && view
            .parts
            .iter()
            .any(|p| matches!(p, AgentAnswerPart::Sources { .. }))
    {
        return Err(invalid(
            "Use data-source-ref controls, not raw source markup in page text",
        ));
    }
    Ok(())
}
fn validate_content_semantics(
    content: &PresentationContent,
    messages: &[Message],
) -> Result<(), ToolError> {
    compile(&content.readable_content, content, messages, true)?;
    for text in std::iter::once(&content.title).chain(content.assumptions.iter()) {
        compile(text, content, messages, false)?;
    }
    Ok(())
}
fn validate_observation(
    content: &PresentationContent,
    dom: &Value,
    messages: &[Message],
) -> Result<(), ToolError> {
    if dom["unsupported_assets"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        return Err(invalid("Use inline CSS/JS, SVG/data images or version animation assets; unresolved external/local asset dependencies are unsupported by authoring"));
    }
    for id in dom["source_ref_ids"].as_array().into_iter().flatten() {
        if !content
            .source_bindings
            .iter()
            .any(|b| Some(b.source_ref_id.as_str()) == id.as_str())
        {
            return Err(invalid("Page contains an unbound source ref"));
        }
    }
    compile(
        dom["semantic_text"]
            .as_str()
            .ok_or_else(|| invalid("Missing browser semantics"))?,
        content,
        messages,
        false,
    )
}

fn preview_document(content: &PresentationContent) -> String {
    let sources: Vec<_> = content
        .source_bindings
        .iter()
        .map(|b| json!({"source_ref_id":b.source_ref_id,"label":b.label_snapshot}))
        .collect();
    let config = json!({"initialState":content.initial_state,"sources":sources})
        .to_string()
        .replace('<', "\\u003c");
    let mut html = crate::presentation_libraries::inline(&content.content_files[&content.entrypoint], &content.content_files);
    for (id, asset) in &content.animation_assets {
        for (path, data) in [
            (format!("assets/{id}.mp4"), format!("data:video/mp4;base64,{}",asset.video_base64)),
            (format!("assets/{id}.png"), format!("data:image/png;base64,{}",asset.poster_png_base64)),
        ] {
            for quote in ['\"', '\''] {
                for attr in ["src", "poster"] {
                    html = html.replace(&format!("{attr}={quote}{path}{quote}"), &format!("{attr}={quote}{data}{quote}"));
                }
            }
        }
    }
    for (path, svg) in content
        .content_files
        .iter()
        .filter(|(path, _)| path.ends_with(".svg"))
    {
        let encoded = svg
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect::<String>();
        let data_url = format!("data:image/svg+xml;charset=utf-8,{encoded}");
        html = html.replace(&format!("src=\"{path}\""), &format!("src=\"{data_url}\""));
        html = html.replace(&format!("src='{path}'"), &format!("src='{data_url}'"));
    }
    // Same common CSS, initial-state and source-label contract as the Reader iframe.
    format!(
        r#"<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; media-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'">
<style>{}</style><script>{}</script><script>{}</script><script>
(()=>{{const config={config};let stateReader=null;window.__presentationPreviewState=()=>stateReader?stateReader():null;window.presentation=Object.freeze({{initialState:config.initialState,restoredState:null,registerStateReader:reader=>{{stateReader=reader}},registerStateRestorer:()=>{{}},commitState:()=>{{}}}});
document.addEventListener('DOMContentLoaded',()=>{{
 const label=()=>document.querySelectorAll('[data-source-ref]').forEach(e=>{{const id=e.getAttribute('data-source-ref');const s=config.sources.find(s=>s.source_ref_id===id);const t=sourceChipLabel(config.sources,id);if(e.textContent!==t)e.textContent=t;const title=s?s.label:'来源不可用';if(e.title!==title)e.title=title;const aria=s?t+'：'+s.label:title;if(e.getAttribute('aria-label')!==aria)e.setAttribute('aria-label',aria);}});
 label();new MutationObserver(label).observe(document.body,{{subtree:true,childList:true,attributes:true}});
 document.addEventListener('click',e=>{{if(e.target.closest('[data-source-ref],a'))e.preventDefault();}},true);
 document.addEventListener('submit',e=>e.preventDefault(),true);
}});}})();</script>{}"#,
        include_str!("../../../packages/web/src/presentation.css"),
        include_str!("../../../packages/web/src/presentation-media.js"),
        include_str!("../../../packages/web/src/source-chip.js").replace("export function", "function"),
        html
    )
}
