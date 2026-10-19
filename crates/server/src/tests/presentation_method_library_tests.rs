//! Every registered template through the real Author preview and delivery path.
use super::*;
use base64::Engine as _;

#[test]
#[ignore = "requires Chromium/Edge and METHOD_LIBRARY_AUTHOR_OUTPUT"]
fn method_library_author_preview_and_deliver() {
    let output = std::path::PathBuf::from(
        std::env::var_os("METHOD_LIBRARY_AUTHOR_OUTPUT").expect("METHOD_LIBRARY_AUTHOR_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let save = |name: &str, value: &Value| {
        std::fs::write(output.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    };
    let mut results = vec![];
    let selected = std::env::var("METHOD_LIBRARY_IDS").unwrap_or_default();
    for method in runtime::presentation_methods::catalog()["methods"].as_array().unwrap() {
        let id = method["id"].as_str().unwrap();
        if !selected.is_empty() && !selected.split(',').any(|name| name == id) { continue; }
        let revision = method["revision"].as_u64().unwrap();
        let mut examples = vec![("", "example/index.html", "example/author.json")];
        if id == "spatial-linked-views" { examples.push(("-spring", "adaptations/spring.html", "adaptations/spring-author.json")); }
        for (suffix, entrypoint, metadata_file) in examples {
        let (_, html) = runtime::presentation_methods::source(id, revision, entrypoint).unwrap();
        let (_, metadata) = runtime::presentation_methods::source(id, revision, metadata_file).unwrap();
        let id = format!("{id}{suffix}");
        let mut input: Value = serde_json::from_str(metadata).unwrap();
        input["operation"] = json!("write");
        input["html"] = json!(html);
        let (_temp, mut state, turn) = setup();
        let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
        let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "method-library", None, None));
        let mut port = RuntimeStatePort { scope: &scope, port: &app, turn_ref: &turn,
            previewed: Default::default(), animations: Default::default(), plots: Default::default() };
        let cancel = CancellationToken::default();
        let written = port.author_presentation(serde_json::from_value(input).unwrap(), &[], &[], &cancel).unwrap();
        let candidate = written.body["candidate_id"].as_str().unwrap().to_string();
        save(&format!("{id}-write.json"), &written.body);
        for (environment, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
            let request = json!({"operation":"preview", "candidate_id":candidate, "viewport":viewport,
                "read_selector":"#readout", "actions":[
                    {"kind":"scroll","y":100},
                    {"kind":"seek","semantic_state":0,"transition_progress":0.35},
                    {"kind":"seek","semantic_state":0,"transition_progress":0},
                    {"kind":"seek","semantic_state":0,"transition_progress":0.35}
                ]});
            let result = port.author_presentation(serde_json::from_value(request).unwrap(), &[], &[], &cancel).unwrap();
            save(&format!("{id}-{environment}.json"), &result.body);
            assert_eq!(result.body["errors"], json!([]), "{id}/{environment}: {}", result.body);
            assert_eq!(result.body["observations"][2]["scene"]["actual"], result.body["observations"][4]["scene"]["actual"]);
            assert_eq!(result.body["reading"]["action_step"], 4);
            assert_eq!(result.body["reading"]["scene"]["playing"], false);
            let image = result.images.last().expect("preview returns actual screenshot");
            std::fs::write(output.join(format!("{id}-{environment}.png")),
                base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
        }
        let delivered = port.author_presentation(AuthorRequest::Deliver { candidate_id: candidate.clone() }, &[], &[], &cancel).unwrap();
        save(&format!("{id}-deliver.json"), &delivered.body);
        let reference = delivered.delivered.unwrap();
        app.with_app(|s| {
            let version = s.private_context().read_presentation(&turn.session_id, &reference).unwrap();
            assert_eq!(crate::presentation_libraries::remove_references(
                &version.content.content_files["index.html"], &version.content.content_files),
                crate::presentation_libraries::remove_references(html, &version.content.content_files));
            save(&format!("{id}-content.json"), &serde_json::to_value(version.content).unwrap());
        });
        results.push(json!({"id":id,"revision":revision,"reference":reference,"environments":3,"status":"passed"}));
        }
    }
    save("results.json", &json!({"methods":results,"provider_calls":0,"status":"passed"}));
}
