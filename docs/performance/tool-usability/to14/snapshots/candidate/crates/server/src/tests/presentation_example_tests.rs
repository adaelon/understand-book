//! Execute the advertised inputs with IDs returned by real authoring storage.
use super::*;
use runtime::presentation::PresentationRef;

fn example(operation: &str, field: &str) -> Value {
    runtime::presentation_author::spec().parameters["examples"]
        .as_array().unwrap().iter()
        .find(|input| input["operation"] == operation && (field.is_empty() || input.get(field).is_some()))
        .unwrap().clone()
}

fn invoke(
    port: &mut impl ResidentStatePort,
    input: Value,
    bindings: &[runtime::orchestrator::SourceBinding],
    calls: &mut Vec<Value>,
) -> AuthorResult {
    let result = port.author_presentation(
        serde_json::from_value(input.clone()).unwrap(), bindings, &[], &CancellationToken::default(),
    ).unwrap_or_else(|e| panic!("{input}: {e:?}"));
    calls.push(json!({"input":input,"result":result.body,"images":result.images.iter().map(|image| json!({
        "candidate_id":image.candidate_id,"environment_name":image.environment_name,"caption":image.caption
    })).collect::<Vec<_>>()}));
    result
}

fn on_candidate(operation: &str, field: &str, id: &str) -> Value {
    let mut input = example(operation, field);
    input["candidate_id"] = json!(id);
    input
}

fn on_reference(operation: &str, field: &str, reference: &PresentationRef) -> Value {
    let mut input = example(operation, field);
    input[field] = json!(reference);
    input
}

fn candidate_id(result: &AuthorResult) -> String {
    assert_eq!(result.body["status"], "candidate_saved");
    assert!(result.delivered.is_none(), "a candidate is not a delivery");
    result.body["candidate_id"].as_str().unwrap().into()
}

fn preview(
    port: &mut impl ResidentStatePort,
    input: Value,
    calls: &mut Vec<Value>,
) -> AuthorResult {
    let id = input["candidate_id"].as_str().unwrap().to_owned();
    let result = invoke(port, input, &[], calls);
    assert_eq!(result.body["errors"], json!([]), "{}", result.body);
    assert_eq!(result.body["reading"]["text"], "1");
    assert!(!result.images.is_empty());
    for image in &result.images {
        assert_eq!(image.candidate_id.as_deref(), Some(id.as_str()));
        assert_eq!(image.environment_name.as_deref(), result.body["environment_name"].as_str());
    }
    result
}

#[test]
#[ignore = "requires installed Chromium/Edge and plot Python"]
fn presentation_to4_advertised_edit_chain_preserves_sources_assets_and_versions() {
    let (_root, mut state, turn) = setup();
    let range = EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(),
        ranges:vec![read_tools::SourceSelectedRange { lid:"1.1".into(), range:read_tools::SourceTextRange { start:0, end:3 } }] };
    let resolved = state.workspace.book.resolve_source(&range, "zh-CN", None).unwrap();
    let binding = runtime::orchestrator::SourceBinding {
        source_ref_id:"source-to4-observed".into(), book_id:state.workspace.book.base.book_id.clone(),
        evidence_range:range, evidence_text_digest:resolved.evidence_text_digest,
        label_snapshot:resolved.label, preview_snapshot:resolved.preview,
    };
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope:&scope, port:&app, turn_ref:&turn,
        previewed:Default::default(), animations:Default::default(), plots:Default::default() };
    let mut calls = Vec::new();
    let new_input = example("write", "new_object");
    assert!(new_input["html"].as_str().unwrap().ends_with(working().trim()), "reuse the existing authoring fixture with touch target styling");
    let first = candidate_id(&invoke(&mut port, new_input, &[], &mut calls));
    let premature = port.author_presentation(serde_json::from_value(on_candidate("deliver", "", &first)).unwrap(), &[], &[], &CancellationToken::default()).err().unwrap();
    assert!(premature.message.contains("Preview"));
    let inspected = preview(&mut port, on_candidate("preview", "width", &first), &mut calls);
    assert_eq!(inspected.body["status"], "preview_ready_for_inspection");
    let initial = invoke(&mut port, on_candidate("deliver", "", &first), &[], &mut calls).delivered.unwrap();

    let plot = invoke(&mut port, example("render_plot", ""), &[], &mut calls);
    assert_eq!(plot.body["mime"], "image/svg+xml");
    let asset = plot.body["asset_ref"].as_str().unwrap();
    let asset_path = plot.body["asset_path"].as_str().unwrap();
    let mut write = on_reference("write", "based_on", &initial);
    write["html"] = json!(write["html"].as_str().unwrap().replace("PLOT_ASSET_REF", asset).replace("SOURCE_REF_FROM_SOURCE_PRESENT", &binding.source_ref_id));
    write["readable_content"] = json!(write["readable_content"].as_str().unwrap().replace("SOURCE_REF_FROM_SOURCE_PRESENT", &binding.source_ref_id));
    write["asset_refs"] = json!([asset]);
    write["source_ref_ids"] = json!([binding.source_ref_id]);
    let sourced = candidate_id(&invoke(&mut port, write.clone(), &[binding.clone()], &mut calls));
    preview(&mut port, on_candidate("preview", "width", &sourced), &mut calls);
    let reference = invoke(&mut port, on_candidate("deliver", "", &sourced), &[], &mut calls).delivered.unwrap();
    assert_eq!(reference.presentation_id, initial.presentation_id);
    assert_eq!(reference.revision, initial.revision + 1);
    let original = app.with_app(|s| s.private_context().read_presentation(&turn.session_id, &reference)).unwrap();
    assert_eq!(original.content.source_bindings, vec![binding]);
    assert!(original.content.content_files.contains_key(asset_path));
    assert!(original.content.content_files.contains_key(&format!("plots/{asset}.py")));
    // Revisions must be able to reuse preserved resources, without run-local render state.
    port.plots.clear();
    write["based_on"] = json!(reference);
    let rewritten = candidate_id(&invoke(&mut port, write, &[], &mut calls));
    assert_eq!(app.with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, &rewritten)).unwrap().content, original.content);
    let read = invoke(&mut port, on_reference("read", "reference", &reference), &[], &mut calls);
    assert_eq!(read.body["text"], original.content.content_files["index.html"]);
    assert_eq!(read.body["source_ref_ids"], json!(["source-to4-observed"]));
    let found = invoke(&mut port, on_reference("search", "reference", &reference), &[], &mut calls);
    assert_eq!(found.body["matches"].as_array().unwrap().len(), 1);
    let offset = found.body["matches"][0]["offset"].as_u64().unwrap() as usize;
    assert!(original.content.content_files["index.html"].chars().skip(offset).collect::<String>().starts_with("补齐证据"));
    let patched = candidate_id(&invoke(&mut port, on_reference("patch", "reference", &reference), &[], &mut calls));
    let read = invoke(&mut port, on_candidate("read", "candidate_id", &patched), &[], &mut calls);
    assert_eq!(read.body["text"], original.content.readable_content);
    let found = invoke(&mut port, on_candidate("search", "candidate_id", &patched), &[], &mut calls);
    assert_eq!(found.body["matches"].as_array().unwrap().len(), 1);
    let last = candidate_id(&invoke(&mut port, on_candidate("patch", "candidate_id", &patched), &[], &mut calls));
    assert_ne!(last, patched);
    let premature = port.author_presentation(serde_json::from_value(on_candidate("deliver", "", &last)).unwrap(), &[], &[], &CancellationToken::default()).err().unwrap();
    assert!(premature.message.contains("Preview"), "old candidate previews must not authorize the patch");
    let mut final_preview = None;
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let mut input = on_candidate("preview", "viewport", &last);
        input["viewport"] = json!(viewport);
        final_preview = Some(preview(&mut port, input, &mut calls));
    }
    let final_preview = final_preview.unwrap();
    assert_eq!(final_preview.body["status"], "preview_ready_for_inspection");
    assert_eq!(final_preview.body["missing_environments"], json!([]));
    let delivered = invoke(&mut port, on_candidate("deliver", "", &last), &[], &mut calls);
    let next = delivered.delivered.unwrap();
    assert_eq!(delivered.body["reference"], json!(next));
    assert_eq!(next.presentation_id, reference.presentation_id);
    assert_eq!(next.revision, reference.revision + 1);
    let version = app.with_app(|s| s.private_context().read_presentation(&turn.session_id, &next)).unwrap();
    let mut expected = original.content.clone();
    expected.content_files.get_mut("index.html").unwrap().clone_from(&original.content.content_files["index.html"].replace("补齐证据", "补齐所需证据"));
    assert_eq!(version.content, expected);
    assert_eq!(app.with_app(|s| s.private_context().read_presentation(&turn.session_id, &reference)).unwrap(), original);
    if let Ok(directory) = std::env::var("TO4_DIR") {
        let directory = std::path::Path::new(&directory);
        std::fs::write(directory.join("chain.json"), serde_json::to_vec_pretty(&json!({
            "spec":runtime::presentation_author::spec(),"calls":calls,"initial_reference":initial,
            "original":original,"delivered":version,"premature_delivery":premature,
        })).unwrap()).unwrap();
        use base64::Engine as _;
        for (index, image) in final_preview.images.iter().enumerate() {
            std::fs::write(directory.join(format!("preview-{index}.png")), base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
        }
    }
}

#[test]
#[ignore = "requires configured Manim 0.21.0 Python"]
fn presentation_to4_advertised_animation_renders() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope:&scope, port:&app, turn_ref:&turn,
        previewed:Default::default(), animations:Default::default(), plots:Default::default() };
    let mut calls = Vec::new();
    let result = invoke(&mut port, example("render_animation", ""), &[], &mut calls);
    assert_eq!(result.body["status"], "animation_rendered");
    assert_eq!(result.body["width"], 640);
    assert_eq!(result.body["height"], 360);
    assert!(result.body["duration_seconds"].as_f64().unwrap() >= 1.0);
    assert!(result.images.len() >= 3);
    if let Ok(directory) = std::env::var("TO4_DIR") {
        std::fs::write(std::path::Path::new(&directory).join("animation.json"), serde_json::to_vec_pretty(&calls).unwrap()).unwrap();
    }
}
