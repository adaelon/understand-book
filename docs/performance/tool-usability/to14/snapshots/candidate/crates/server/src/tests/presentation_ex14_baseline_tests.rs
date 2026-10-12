//! Offline EX14.0 material preparation; no provider or production user data.
use super::*;
use base64::Engine;

#[test]
#[ignore = "EX14.3 static revision; needs browser and a new EX14_VISUAL_N3_OUTPUT directory"]
fn ex14_visual_static_revision_previews_and_delivers() {
    let output = std::path::PathBuf::from(
        std::env::var_os("EX14_VISUAL_N3_OUTPUT").expect("EX14_VISUAL_N3_OUTPUT"),
    );
    std::fs::create_dir(&output).expect("preserve previous evidence");
    let save = |name: &str, value: &Value| {
        std::fs::write(output.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    };
    let baseline: runtime::presentation::AgentPresentation = serde_json::from_str(include_str!(
        "../../../../docs/performance/presentation-critical-moments-ex14/materials/N3/version.json"
    ))
    .unwrap();
    let (_root, mut state, turn) = setup();
    // Import the frozen content into an isolated test owner; retain its origin.
    let original = state
        .private_context()
        .create_presentation_candidate(
            &turn.session_id,
            &turn.turn_id,
            None,
            baseline.content.clone(),
        )
        .unwrap();
    let reference = state
        .private_context()
        .persist_presentation_candidate(&turn.session_id, &turn.turn_id, &original.candidate_id)
        .unwrap();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app
        .with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "ex14-visual-n3", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let mut expected = baseline.content.clone();
    expected.content_files.insert(
        "index.html".into(),
        baseline.content.content_files["index.html"].replace("只要 A 就 B", "A 成立时 B 一定成立"),
    );
    expected.readable_content = expected
        .readable_content
        .replace("只要 A 就 B", "A 成立时 B 一定成立");
    let patch = json!({"operation":"patch","reference":reference,
        "edits":[{"old_text":"只要 A 就 B","new_text":"A 成立时 B 一定成立"}],
        "readable_content":expected.readable_content});
    save("patch.json", &patch);
    let result = port
        .author_presentation(
            serde_json::from_value(patch).unwrap(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
    let id = result.body["candidate_id"].as_str().unwrap().to_owned();
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let request = json!({"operation":"preview","candidate_id":id,"viewport":viewport,
            "read_selector":"#page","actions":[{"kind":"scroll","y":99999}]});
        save(&format!("{name}-request.json"), &request);
        let result = port
            .author_presentation(
                serde_json::from_value(request).unwrap(),
                &[],
                &[],
                &CancellationToken::default(),
            )
            .unwrap();
        save(&format!("{name}-response.json"), &result.body);
        assert_ne!(result.body["status"], "preview_failed", "{}", result.body);
        let text = result.body["reading"]["text"].as_str().unwrap();
        for expected in [
            "A 成立时 B 一定成立",
            "例子一",
            "例子二",
            "6 是偶数",
            "长 3、宽 2",
        ] {
            assert!(text.contains(expected));
        }
        assert!(!text.contains("只要 A 就 B"));
        for (index, image) in result.images.iter().enumerate() {
            std::fs::write(
                output.join(format!("{name}-{index}.png")),
                base64::engine::general_purpose::STANDARD
                    .decode(&image.png_base64)
                    .unwrap(),
            )
            .unwrap();
        }
    }
    let delivered = port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id: id },
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
    let next = delivered.delivered.unwrap();
    assert_eq!(next.presentation_id, reference.presentation_id);
    assert_eq!(next.revision, reference.revision + 1);
    app.with_app(|s| {
        let version = s
            .private_context()
            .read_presentation(&turn.session_id, &next)
            .unwrap();
        assert_eq!(version.content, expected);
        assert_eq!(
            s.private_context()
                .read_presentation(&turn.session_id, &reference)
                .unwrap()
                .content,
            baseline.content
        );
        save("version.json", &serde_json::to_value(version).unwrap());
    });
    save(
        "verification.json",
        &json!({"status":"passed","origin":baseline.reference,
        "imported_reference":reference,"delivered_reference":next,"provider_calls":0,
        "model_behavior":"not evaluated; deterministic patch and real browser delivery",
        "content":"exactly one HTML phrase plus matching readable_content; all other fields identical"}),
    );
}

#[test]
#[ignore = "EX14.0 offline baseline export; needs installed browser and a new EX14_N3_OUTPUT directory"]
fn ex14_baseline_deliver_static_n3() {
    let output =
        std::path::PathBuf::from(std::env::var_os("EX14_N3_OUTPUT").expect("EX14_N3_OUTPUT"));
    std::fs::create_dir(&output).expect("use a new output directory; preserve frozen revisions");
    let save = |name: &str, value: &Value| {
        std::fs::write(output.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    };
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| {
        crate::run_scope::RunScope::capture(state, &turn, "ex14-n3-baseline", None, None)
    });
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let html = include_str!("../../tests/fixtures/presentation-ex14-n3.html");
    let prose = "必要条件与充分条件。A 是 B 的充分条件：只要 A 就 B。B 是 A 的必要条件：没有 B，就没有 A。A 成立能推出 B，不保证反向推理。例子一：整数能被 4 整除，则它是偶数；6 是偶数却不能被 4 整除。例子二：在平面欧氏几何中，正方形是矩形；长 3、宽 2 的矩形不是正方形。";
    let request = json!({"operation":"write", "title":"必要条件与充分条件",
        "html":html,"readable_content":prose,"source_ref_ids":[],"asset_refs":[],
        "state_contract":{},"initial_state":{},
        "assumptions":["EX14 N3 自包含教学基准：整数整除和平面欧氏几何；不是书籍引文。"]});
    save("write.json", &request);
    let result = port
        .author_presentation(
            serde_json::from_value(request).unwrap(),
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    save("candidate.json", &result.body);
    let id = result.body["candidate_id"].as_str().unwrap().to_string();
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let request = json!({"operation":"preview","candidate_id":id,"viewport":viewport,
            "read_selector":"#page", "actions":[{"kind":"scroll","y":99999}]});
        save(&format!("{name}-request.json"), &request);
        let result = port
            .author_presentation(
                serde_json::from_value(request).unwrap(),
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        save(&format!("{name}-response.json"), &result.body);
        assert_ne!(result.body["status"], "preview_failed", "{}", result.body);
        let text = result.body["reading"]["text"].as_str().unwrap();
        for expected in ["只要 A 就 B", "例子一", "例子二", "6 是偶数", "长 3、宽 2"]
        {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!result.images.is_empty());
        for (index, image) in result.images.iter().enumerate() {
            assert_eq!(image.candidate_id.as_deref(), Some(id.as_str()));
            assert_eq!(image.environment_name.as_deref(), Some(name));
            std::fs::write(
                output.join(format!("{name}-{index}.png")),
                base64::engine::general_purpose::STANDARD
                    .decode(&image.png_base64)
                    .unwrap(),
            )
            .unwrap();
        }
    }
    let result = port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id: id },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    save("delivery.json", &result.body);
    let reference = result.delivered.unwrap();
    assert_eq!(reference.revision, 1);
    app.with_app(|state| {
        let version = state
            .private_context()
            .read_presentation(&turn.session_id, &reference)
            .unwrap();
        assert_eq!(version.content.content_files["index.html"], html);
        assert_eq!(version.content.readable_content, prose);
        assert!(version.content.animation_assets.is_empty());
        assert!(version.content.source_bindings.is_empty());
        save("version.json", &serde_json::to_value(version).unwrap());
    });
    save(
        "preparation.json",
        &json!({"case":"N3","kind":"deterministic server authoring fixture",
        "reference":reference,"provider_calls":0,"model_observation":"not_run",
        "browser_preview":"passed","native_delivery":"passed",
        "owner_note":"isolated test book/session; import this same version into each experimental session via existing fixture setup"}),
    );
}
