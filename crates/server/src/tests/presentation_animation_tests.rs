use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use runtime::presentation::{AnimationAsset, AnimationCue};

const CODE: &str = "class PresentationAnimation(Scene):\n    def construct(self):\n        dot = Dot(LEFT * 3, color=BLUE, radius=0.4)\n        self.add(dot)\n        self.play(dot.animate.shift(RIGHT * data['distance']), run_time=2, rate_func=linear)\n        self.wait(0.2)\n";
fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/performance/ex11-local-demonstrations/ex11.3")
}

#[test]
fn animation_limits_and_legacy_default() {
    let old = json!({"title":"old","content_files":{"index.html":"<p>old</p>"},"entrypoint":"index.html","readable_content":"old","source_bindings":[],"assumptions":[],"state_contract":{},"initial_state":{}});
    assert!(
        serde_json::from_value::<runtime::presentation::PresentationContent>(old)
            .unwrap()
            .animation_assets
            .is_empty()
    );
    let mut asset = AnimationAsset {
        video_base64: STANDARD.encode(vec![0; 8 * 1024 * 1024 + 1]),
        poster_png_base64: String::new(),
        width: 320,
        height: 240,
        duration_seconds: 1.0,
        fps: 30.0,
        cues: vec![],
    };
    assert!(
        crate::presentation_animation::validate_assets(&[("a".into(), asset.clone())].into())
            .unwrap_err()
            .message
            .contains("8 MiB")
    );
    asset.video_base64 = STANDARD.encode(vec![0; 8 * 1024 * 1024]);
    asset.poster_png_base64 = STANDARD.encode([1]);
    assert!(crate::presentation_animation::validate_assets(
        &[
            ("a".into(), asset.clone()),
            ("b".into(), asset.clone()),
            ("c".into(), asset)
        ]
        .into()
    )
    .unwrap_err()
    .message
    .contains("24 MiB"));
}

#[test]
#[ignore = "requires configured Manim 0.21.0 Python"]
fn ex11_animation_render_version_read_reuse() {
    let (_temp, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        plots: Default::default(),
        animations: Default::default(),
    };
    let cancel = CancellationToken::default();
    let started = std::time::Instant::now();
    let result = port
        .author_presentation(
            AuthorRequest::RenderAnimation {
                code: CODE.into(),
                data: json!({"distance":6}),
                size: Some(PlotSize {
                    width: 640,
                    height: 360,
                }),
                cues: vec![AnimationCue {
                    id: "middle".into(),
                    label: "Midpoint".into(),
                    at_seconds: 1.0,
                }],
            },
            &[],
            &[],
            &cancel,
        )
        .unwrap();
    assert_eq!(result.body["width"], 640);
    assert_eq!(result.body["height"], 360);
    assert_eq!(result.body["fps"], 30.0);
    assert!(result.body["duration_seconds"].as_f64().unwrap() >= 2.0);
    assert!(result.images.len() >= 3);
    assert_ne!(result.images[0].png_base64, result.images[1].png_base64);
    let id = result.body["asset_ref"].as_str().unwrap();
    let html=format!("<video playsinline src='assets/{id}.mp4' poster='assets/{id}.png'></video><p>Fixed motion from left to right.</p>");
    let request = json!({"operation":"write","title":"Fixed motion","html":html,"readable_content":"Fixed motion from left to right.","asset_refs":[id]});
    let written = port
        .author_presentation(
            serde_json::from_value(request.clone()).unwrap(),
            &[],
            &[],
            &cancel,
        )
        .unwrap();
    let candidate = app
        .with_app(|s| {
            s.read_presentation_candidate(
                &turn.session_id,
                written.body["candidate_id"].as_str().unwrap(),
            )
        })
        .unwrap();
    assert_eq!(candidate.content.animation_assets.len(), 1);
    let asset = &candidate.content.animation_assets[id];
    std::fs::create_dir_all(root()).unwrap();
    std::fs::write(
        root().join("content.json"),
        serde_json::to_vec_pretty(&candidate.content).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root().join("animation.mp4"),
        STANDARD.decode(&asset.video_base64).unwrap(),
    )
    .unwrap();
    for (i, image) in result.images.iter().enumerate() {
        std::fs::write(
            root().join(format!("frame-{i}.png")),
            STANDARD.decode(&image.png_base64).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(root().join("render.json"),serde_json::to_vec_pretty(&json!({"metadata":result.body,"elapsed_ms":started.elapsed().as_millis(),"media_bytes":STANDARD.decode(&asset.video_base64).unwrap().len(),"identity":"engineering scene"})).unwrap()).unwrap();
    let reference = app
        .with_app(|s| {
            s.persist_presentation_candidate(
                &turn.session_id,
                &turn.turn_id,
                &candidate.candidate_id,
            )
        })
        .unwrap();
    app.with_app(|s| {
        let outcome = OuterOutcome {
            answer: Some("motion".into()),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: reference.presentation_id.clone(),
                    revision: reference.revision,
                }],
                sources: vec![],
            }),
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: vec![],
            trace: vec![],
            profile_usage: Default::default(),
            memory_updates: vec![],
            source_bindings: vec![],
            delivery_diagnostics: None,
            request_audit: Default::default(),
        };
        finalize_agent_turn_completed(
            s,
            &turn,
            &outcome,
            &s.messages.clone(),
            "2026-09-28T06:00:00Z",
        )
        .unwrap();
    });
    for file in [
        None,
        Some(format!("assets/{id}.mp4")),
        Some(format!("assets/{id}.png")),
        Some(format!("animations/{id}.py")),
        Some(format!("animations/{id}.json")),
    ] {
        let read = port
            .author_presentation(
                AuthorRequest::Read {
                    reference: reference.clone(),
                    file: file.clone(),
                    offset: 0,
                },
                &[],
                &[],
                &cancel,
            )
            .unwrap();
        assert!(!read.body.to_string().contains(&asset.video_base64));
        if file.as_deref() == Some(&format!("animations/{id}.py")) {
            assert_eq!(read.body["text"], CODE);
        }
    }
    let revision_turn = app.with_app(|s| {
        let book = s.book.base.book_id.clone();
        precommit_agent_turn(
            s,
            &book,
            "revise".into(),
            None,
            None,
            None,
            "2026-09-28T06:01:00Z",
        )
        .unwrap()
    });
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &revision_turn,
        previewed: Default::default(),
        plots: Default::default(),
        animations: Default::default(),
    };
    let mut revision = request.clone();
    revision["based_on"] = json!(reference);
    let revised = port
        .author_presentation(
            serde_json::from_value(revision.clone()).unwrap(),
            &[],
            &[],
            &cancel,
        )
        .unwrap();
    let revised = app
        .with_app(|s| {
            s.read_presentation_candidate(
                &turn.session_id,
                revised.body["candidate_id"].as_str().unwrap(),
            )
        })
        .unwrap();
    assert_eq!(
        revised.content.animation_assets,
        candidate.content.animation_assets
    );
    revision["asset_refs"] = json!([]);
    assert!(port
        .author_presentation(serde_json::from_value(revision).unwrap(), &[], &[], &cancel)
        .is_err());
    assert!(port
        .author_presentation(serde_json::from_value(request).unwrap(), &[], &[], &cancel)
        .is_err());
}

#[test]
#[ignore = "requires configured Manim 0.21.0 Python"]
fn ex11_animation_errors_and_process_tree_cancellation() {
    let cancel = CancellationToken::default();
    let error = crate::presentation_animation::render(
        "raise RuntimeError('scene diagnostic')".into(),
        json!({}),
        None,
        vec![],
        &cancel,
    )
    .err()
    .unwrap();
    assert!(error.message.contains("scene diagnostic"));
    let error = crate::presentation_animation::render(
        CODE.into(),
        json!({"distance":6}),
        Some(PlotSize {
            width: 320,
            height: 240,
        }),
        vec![AnimationCue {
            id: "bad".into(),
            label: "bad".into(),
            at_seconds: 99.0,
        }],
        &cancel,
    )
    .err()
    .unwrap();
    assert!(error.message.contains("beyond actual duration"));
    let dir = tempfile::tempdir().unwrap();
    let record = dir.path().join("process.json");
    let code=format!("import subprocess, sys, os, json, time\nfrom pathlib import Path\nchild=subprocess.Popen([sys.executable,'-c','import time; time.sleep(120)'])\nPath({}).write_text(json.dumps({{'child':child.pid,'parent':os.getpid(),'cwd':os.getcwd()}}))\ntime.sleep(120)\n",serde_json::to_string(&record.to_string_lossy()).unwrap());
    let token = cancel.clone();
    let worker = std::thread::spawn(move || {
        crate::presentation_animation::render(code, json!({}), None, vec![], &token)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !record.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    cancel.cancel();
    assert_eq!(
        worker.join().unwrap().err().unwrap().error_code,
        "AGENT_RUN_CANCELLED"
    );
    let processes: Value = serde_json::from_slice(&std::fs::read(record).unwrap()).unwrap();
    assert!(!std::path::Path::new(processes["cwd"].as_str().unwrap()).exists());
    #[cfg(windows)]
    for key in ["parent", "child"] {
        let output = std::process::Command::new("tasklist")
            .args([
                "/FI",
                &format!("PID eq {}", processes[key]),
                "/FO",
                "CSV",
                "/NH",
            ])
            .output()
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains(&format!("\"{}\"", processes[key]))
        );
    }
    std::fs::create_dir_all(root()).unwrap();
    std::fs::write(root().join("cancellation.json"),serde_json::to_vec_pretty(&json!({"processes":processes,"cancelled":true,"temporary_directory_removed":true,"process_tree_stopped":true})).unwrap()).unwrap();
}

#[test]
#[ignore = "requires rendered EX11.3 asset and installed Chromium/Edge"]
fn ex11_media_formal_preview_delivery() {
    let (_temp, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        plots: Default::default(),
        animations: Default::default(),
    };
    let cancel = CancellationToken::default();
    let content: runtime::presentation::PresentationContent =
        serde_json::from_slice(&std::fs::read(root().join("content.json")).unwrap()).unwrap();
    let (id, asset) = content.animation_assets.into_iter().next().unwrap();
    port.animations.insert(
        id.clone(),
        crate::presentation_animation::RenderedAnimation {
            asset,
            code: CODE.into(),
            data: json!({"distance":6}),
            frames: vec![],
        },
    );
    let html =
        include_str!("../../tests/fixtures/presentation-animation.html").replace("@ASSET@", &id);
    let initial = json!({"active_demo":"a","a_time":0,"a_step":0,"a_progress":0,"b_time":0,"b_step":0,"b_progress":0});
    let contract = json!({"active_demo":"local demo id a or b","a_time":"demo a actual decoded media seconds 0..2.2","a_step":"demo a completed stages 0..3 (3 is final frame)","a_progress":"demo a fractional seconds [0,1)","b_time":"demo b actual decoded media seconds 0..2.2","b_step":"demo b completed stages 0..3 (3 is final frame)","b_progress":"demo b fractional seconds [0,1)"});
    let result=port.author_presentation(serde_json::from_value(json!({"operation":"write","title":"Local motion","html":html,"readable_content":"The blue point travels from left to right in two seconds. Both demonstrations show the same fixed process.","asset_refs":[id],"initial_state":initial,"state_contract":contract})).unwrap(),&[],&[],&cancel).unwrap();
    let candidate = result.body["candidate_id"].as_str().unwrap().to_owned();
    let output = root().parent().unwrap().join("ex11.4");
    std::fs::create_dir_all(&output).unwrap();
    let mut records = vec![];
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let preview = port
            .author_presentation(
                AuthorRequest::Preview {
                    candidate_id: candidate.clone(),
                    width: None,
                    viewport: Some(viewport),
                    read_selector: Some("#result".into()),
                    actions: vec![
                        PreviewAction::Seek {
                            semantic_state: 1,
                            transition_progress: json!(0.5),
                        },
                        PreviewAction::Seek {
                            semantic_state: 0,
                            transition_progress: json!(0.0),
                        },
                        PreviewAction::Seek {
                            semantic_state: 1,
                            transition_progress: json!(0.5),
                        },
                        PreviewAction::Seek {
                            semantic_state: 1,
                            transition_progress: json!(0.5),
                        },
                    ],
                },
                &[],
                &[],
                &cancel,
            )
            .unwrap();
        std::fs::write(
            output.join(format!("preview-{}.json", viewport.width)),
            serde_json::to_vec_pretty(&preview.body).unwrap(),
        )
        .unwrap();
        for (i, image) in preview.images.iter().enumerate() {
            std::fs::write(
                output.join(format!("preview-{}-{i}.png", viewport.width)),
                STANDARD.decode(&image.png_base64).unwrap(),
            )
            .unwrap();
        }
        assert!(preview.body.get("error_code").is_none(), "{}", preview.body);
        assert!(preview.body["reading"]["text"]
            .as_str()
            .unwrap()
            .contains("1.500s"));
        records.push(preview.body);
    }
    let delivered = port
        .author_presentation(
            AuthorRequest::Deliver {
                candidate_id: candidate.clone(),
            },
            &[],
            &[],
            &cancel,
        )
        .unwrap();
    let saved = app
        .with_app(|s| s.read_presentation_candidate(&turn.session_id, &candidate))
        .unwrap();
    std::fs::write(
        output.join("content.json"),
        serde_json::to_vec_pretty(&saved.content).unwrap(),
    )
    .unwrap();
    std::fs::write(output.join("delivery.json"),serde_json::to_vec_pretty(&json!({"identity":"engineering page, production author preview/deliver","delivery":delivered.body,"previews":records})).unwrap()).unwrap();
    let unresolved = port.author_presentation(serde_json::from_value(json!({"operation":"write","title":"Missing media","html":"<video src='assets/missing.mp4'></video><p>Missing media</p>","readable_content":"Missing media"})).unwrap(), &[], &[], &cancel).unwrap();
    let preview = port.author_presentation(AuthorRequest::Preview {candidate_id:unresolved.body["candidate_id"].as_str().unwrap().into(),width:None,viewport:None,read_selector:None,actions:vec![]}, &[], &[], &cancel).unwrap();
    assert_eq!(preview.body["status"], "preview_failed");
}
