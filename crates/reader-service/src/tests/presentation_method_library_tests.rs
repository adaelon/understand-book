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
        let mut examples: Vec<(&str, &str, &str)> = if let Some(scenes) = method["scenes"].as_array() {
            scenes.iter().map(|scene| {
                let file = scene["file"].as_str().unwrap();
                (file.split('/').next().unwrap(), file, scene["metadata"].as_str().unwrap())
            }).collect()
        } else { vec![("", "example/index.html", "example/author.json")] };
        if id == "spatial-linked-views" { examples.push(("-spring", "adaptations/spring.html", "adaptations/spring-author.json")); }
        for (suffix, entrypoint, metadata_file) in examples {
        let (_, html) = runtime::presentation_methods::source(id, revision, entrypoint).unwrap();
        let (_, metadata) = runtime::presentation_methods::source(id, revision, metadata_file).unwrap();
        let case = (id, revision, suffix);
        let id = if method["scenes"].is_array() {
            format!("{id}-v{revision}-{suffix}")
        } else if id == "compare-one-factor" && revision > 2 {
            format!("{id}-v{revision}")
        } else {
            format!("{id}{suffix}")
        };
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
            let click = |selector: &str| json!({"kind":"click","selector":selector});
            let (actions, selector, expected) = match case {
                ("compare-rules", _, "routes") => (vec![click("#linf"),click("#step")],"#readout",Some("1 步")),
                ("compare-rules", _, _) => (vec![click("#linf")],"#readout",Some("乙更长")),
                ("inspect-space", _, "sections") => (vec![click("#l1"),json!({"kind":"key","key":"End","selector":"#slice"})],"#readout",Some("0.100")),
                ("inspect-space", _, _) => (vec![click("#apart"),click("#angle")],"#readout",Some("100%")),
                ("compare-one-factor", 3, _) => (vec![json!({"kind":"key","key":"End","selector":"#speed"}), click("#apply")],"#reading",Some("B 走 30 米")),
                _ => (vec![json!({"kind":"scroll","y":100}),
                    json!({"kind":"seek","semantic_state":0,"transition_progress":0.35}),
                    json!({"kind":"seek","semantic_state":0,"transition_progress":0}),
                    json!({"kind":"seek","semantic_state":0,"transition_progress":0.35})],"#readout",None),
            };
            let request = json!({"operation":"preview","candidate_id":candidate,"viewport":viewport,
                "read_selector":selector,"actions":actions});
            let result = port.author_presentation(serde_json::from_value(request).unwrap(), &[], &[], &cancel).unwrap();
            save(&format!("{id}-{environment}.json"), &result.body);
            assert_eq!(result.body["errors"], json!([]), "{id}/{environment}: {}", result.body);
            if let Some(text) = expected {
                assert!(result.body["reading"]["text"].as_str().is_some_and(|v|v.contains(text)), "{id}: {}",result.body["reading"]);
            } else {
                assert_eq!(result.body["observations"][2]["scene"]["actual"], result.body["observations"][4]["scene"]["actual"]);
                assert_eq!(result.body["reading"]["action_step"], 4);
                assert_eq!(result.body["reading"]["scene"]["playing"], false);
            }
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
