//! SE1 candidate assets through the existing Author preview/delivery path. No provider.
use super::*;
use base64::Engine as _;
use std::path::Path;

const HTML: &str = include_str!("../../../../skills/presentation/methods/discrete-update/v1/example/index.html");
const GUIDE: &str = include_str!("../../../../skills/presentation/methods/discrete-update/v1/example/README.md");

fn write_example() -> AuthorRequest {
    let readable = GUIDE.lines().find_map(|line| line.strip_prefix("- readable_content：")).unwrap();
    AuthorRequest::Write {
                action_contract: vec![],
        teaching_binding: None,
        method_refs: None,
        new_object: false, libraries: vec![], based_on: None,
        title: "追踪一次离散更新".into(), html: HTML.into(), readable_content: readable.into(),
        state_contract: json!({
            "eta":"fixed dimensionless learning rate in 0..1.2",
            "semantic_state":"completed discrete updates, integer 0..8",
            "transition_progress":"visual interpolation toward next update in [0,1); zero at step 8"
        }),
        initial_state: json!({"eta":0.25,"semantic_state":0,"transition_progress":0}),
        source_ref_ids: vec![], asset_refs: vec![],
        assumptions: vec!["固定一维二次函数、学习率和初值 4，无动量与噪声；无量纲教学模型，展示前 8 次更新。".into()],
    }
}

fn save(root: &Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
#[ignore = "requires Chromium/Edge and a new SE1_OUTPUT directory"]
fn se1_author_preview_and_deliver() {
    let root = std::path::PathBuf::from(std::env::var_os("SE1_OUTPUT").expect("SE1_OUTPUT"));
    std::fs::create_dir_all(&root).unwrap();
    let output = root.join("author");
    std::fs::create_dir(&output).expect("choose a new SE1_OUTPUT to retain previous observations");
    let (_temp, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "se1", None, None));
    let mut port = RuntimeStatePort { scope: &scope, port: &app, turn_ref: &turn,
        previewed: Default::default(), animations: Default::default(), plots: Default::default() };
    let cancel = CancellationToken::default();
    let written = port.author_presentation(write_example(), &[], &[], &cancel).unwrap();
    save(&output, "write.json", &written.body);
    let id = written.body["candidate_id"].as_str().unwrap().to_owned();
    let seek = |k, p| json!({"kind":"seek","semantic_state":k,"transition_progress":p});
    for (environment, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        for (name, actions) in [
            ("replay", vec![json!({"kind":"scroll","y":110}),seek(2,0.35),seek(1,0.25),seek(2,0.35)]),
            ("near-end", vec![json!({"kind":"scroll","y":110}),seek(7,0.999)]),
            ("stationary", vec![json!({"kind":"click","selector":"[data-eta='0']"}),json!({"kind":"scroll","y":110}),seek(2,0.35)]),
            ("arrival", vec![json!({"kind":"click","selector":"[data-eta='0.5']"}),json!({"kind":"scroll","y":110}),seek(1,0.35)]),
            ("oscillation", vec![json!({"kind":"click","selector":"[data-eta='1']"}),json!({"kind":"scroll","y":110}),seek(1,0.5)]),
            ("divergence", vec![json!({"kind":"click","selector":"[data-eta='1.2']"}),json!({"kind":"scroll","y":110}),seek(7,0.35)]),
            ("controls", vec![json!({"kind":"scroll","y":500}),json!({"kind":"scroll","y":760}),json!({"kind":"scroll","y":99999})]),
        ] {
            let input = json!({"operation":"preview","candidate_id":id,"viewport":viewport,
                "read_selector":"#readout","actions":actions});
            save(&output, &format!("{environment}-{name}-request.json"), &input);
            let result = port.author_presentation(serde_json::from_value(input).unwrap(), &[], &[], &cancel).unwrap();
            save(&output, &format!("{environment}-{name}.json"), &result.body);
            assert_eq!(result.body["errors"], json!([]), "{}", result.body);
            for (index, image) in result.images.iter().enumerate() {
                assert_eq!(image.candidate_id.as_deref(), Some(id.as_str()));
                assert_eq!(image.environment_name.as_deref(), Some(environment));
                std::fs::write(output.join(format!("{environment}-{name}-{index}.png")),
                    base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
            }
            if name == "replay" {
                let observations = &result.body["observations"];
                assert_eq!(observations[2]["scene"]["actual"], observations[4]["scene"]["actual"]);
                assert_eq!(result.body["reading"]["action_step"], 4);
                assert!(result.body["reading"]["text"].as_str().unwrap().contains("2.4125"));
            }
            if name == "near-end" {
                let actual = &result.body["reading"]["scene"];
                assert_eq!(actual["semantic_state"], 7);
                assert!((actual["transition_progress"].as_f64().unwrap()-0.999).abs()<1e-12);
                assert_eq!(actual["playing"], false);
            }
        }
    }
    let delivered = port.author_presentation(AuthorRequest::Deliver {candidate_id:id.clone()}, &[], &[], &cancel).unwrap();
    save(&output, "deliver.json", &delivered.body);
    let reference = delivered.delivered.unwrap();
    app.with_app(|s| {
        let version = s.private_context().read_presentation(&turn.session_id, &reference).unwrap();
        assert_eq!(version.content.content_files["index.html"], HTML);
        save(&root, "content.json", &serde_json::to_value(version.content).unwrap());
    });
    save(&root, "author-result.json", &json!({"status":"passed","candidate_id":id,
        "reference":reference,"preview_batches":21,"provider_calls":0,
        "observation_kind":"deterministic Author calls; screenshot inspection recorded separately"}));
}
