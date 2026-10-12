//! EX14.1-B: real author previews, deterministic fixture pixels, no provider.
use super::*;
use base64::Engine;
use runtime::presentation_preview::PreviewViewport;
use std::path::PathBuf;

const F2: &str = include_str!("../../tests/fixtures/presentation-ex14-f2-b.html");

fn output(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("EX14_BROWSER_OUTPUT")
            .expect("set EX14_BROWSER_OUTPUT to a new output root"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(name);
    std::fs::create_dir(&path).expect("preserve prior evidence; choose a new output root");
    path
}
fn save(path: &std::path::Path, name: &str, value: &Value) {
    std::fs::write(path.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn seek(step: u32, progress: f64) -> Value {
    json!({"kind":"seek","semantic_state":step,"transition_progress":progress})
}
fn preview(
    port: &mut impl ResidentStatePort,
    output: &std::path::Path,
    name: &str,
    id: &str,
    viewport: PreviewViewport,
    actions: Vec<Value>,
) -> AuthorResult {
    let request = json!({"operation":"preview","candidate_id":id,"viewport":viewport,
        "read_selector":"#readout","actions":actions});
    save(output, &format!("{name}-request.json"), &request);
    let result = port
        .author_presentation(
            serde_json::from_value(request).unwrap(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
    save_result(output, name, &result);
    assert_ne!(result.body["status"], "preview_failed", "{}", result.body);
    assert!(
        result.body["errors"].as_array().unwrap().is_empty(),
        "{}",
        result.body
    );
    result
}
fn save_result(output: &std::path::Path, name: &str, result: &AuthorResult) {
    save(output, &format!("{name}-response.json"), &result.body);
    for (i, image) in result.images.iter().enumerate() {
        assert_eq!(
            image.candidate_id.as_deref(),
            result.body["candidate_id"].as_str()
        );
        assert_eq!(
            image.environment_name.as_deref(),
            result.body["environment_name"].as_str()
        );
        std::fs::write(
            output.join(format!("{name}-{i}.png")),
            base64::engine::general_purpose::STANDARD
                .decode(&image.png_base64)
                .unwrap(),
        )
        .unwrap();
    }
    save(output, &format!("{name}-images.json"), &json!(result.images.iter().map(|image|json!({"candidate_id":image.candidate_id,"environment_name":image.environment_name,"caption":image.caption})).collect::<Vec<_>>()));
}
// Fixed fixture oracle: inspect the rendered solid object and trail, not reported state.
fn marker(image: &PreviewImage, color: [u8; 3]) -> (f64, usize) {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&image.png_base64)
        .unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    let channels = frame.color_type.samples();
    assert!(channels >= 3);
    let mut sum = 0usize;
    let mut count = 0;
    for (i, p) in pixels[..frame.buffer_size()]
        .chunks_exact(channels)
        .enumerate()
    {
        if p[..3] == color {
            sum += i % frame.width as usize;
            count += 1;
        }
    }
    assert!(
        count > 0,
        "fixture marker is outside the screenshot or not drawn"
    );
    (sum as f64 / count as f64, count)
}
fn position(result: &AuthorResult, index: usize, t: f64, active: &str) {
    let o = &result.body["observations"][index];
    let scene = &o["scene"];
    assert_eq!(scene["actual"], scene["after_capture"]);
    assert_eq!(scene["actual"]["playing"], false);
    let actual = scene["actual"]["semantic_state"].as_f64().unwrap()
        + scene["actual"]["transition_progress"].as_f64().unwrap();
    assert!((actual - t).abs() < 1e-6);
    assert!(o["dom"]["visible_text"]
        .as_str()
        .unwrap()
        .contains(&format!("活动 {active}")));
}
fn geometry(result: &AuthorResult, index: usize, t: f64, active: &str) -> bool {
    let o = &result.body["observations"][index];
    let svg = o["dom"]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "drawing")
        .unwrap();
    let color = if active == "B" {
        [0, 160, 255]
    } else {
        [255, 64, 0]
    };
    let (x, count) = marker(&result.images[index], color);
    (x - (svg["x"].as_f64().unwrap() + 20.0 + 20.0 * t + 5.5)).abs() < 0.6 && count == 192
}

#[test]
fn ex14_action_budget_and_unknown_action_rejected() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| {
        crate::run_scope::RunScope::capture(state, &turn, "ex14-limits", None, None)
    });
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let id = candidate(&mut port, F2);
    let err = port
        .author_presentation(
            AuthorRequest::Preview {
                candidate_id: id,
                width: None,
                viewport: Some(REQUIRED_PREVIEW_ENVIRONMENTS[2].1),
                read_selector: None,
                actions: (0..5).map(|_| PreviewAction::Scroll { y: 0 }).collect(),
            },
            &[],
            &[],
            &CancellationToken::default(),
        )
        .err()
        .expect("five actions must fail");
    assert!(err.message.contains("At most four actions"));
    let err = serde_json::from_value::<AuthorRequest>(json!({"operation":"preview","candidate_id":"unused","actions":[{"kind":"set_value","selector":"#r2Range","value":1.5}]})).unwrap_err();
    assert!(err.to_string().contains("unknown variant"));
}

#[test]
#[ignore = "requires installed Chromium/Edge and new EX14_BROWSER_OUTPUT"]
fn ex14_browser_f2_replay_and_pixel_oracle() {
    let output = output("f2");
    std::fs::write(output.join("fixture.html"), F2).unwrap();
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app
        .with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "ex14-f2", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let id = candidate(&mut port, F2);
    save(
        &output,
        "candidate.json",
        &json!({"candidate_id":id,"kind":"deterministic author-tool observations","provider_calls":0,"agent_observation":"not_run"}),
    );
    let select = || json!({"kind":"click","selector":"#select-b"});
    let mut findings = vec![];
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        for (label, step, progress) in [("before", 6, 0.25), ("fault", 6, 0.5), ("after", 6, 0.75)]
        {
            let result = preview(
                &mut port,
                &output,
                &format!("{name}-{label}"),
                &id,
                viewport,
                vec![
                    json!({"kind":"scroll","y":1100}),
                    select(),
                    seek(4, 0.25),
                    seek(step, progress),
                ],
            );
            let t = step as f64 + progress;
            position(&result, 3, 4.25, "B");
            position(&result, 4, t, "B");
            assert!(geometry(&result, 3, 4.25, "B"));
            assert!(geometry(&result, 4, t, "B"));
            let reading = &result.body["reading"];
            assert_eq!(reading["action_step"], 4);
            assert_eq!(reading["page_state"]["values"]["active"], "B");
            assert_eq!(reading["scene"]["transition_progress"], progress);
            assert!(reading["text"]
                .as_str()
                .unwrap()
                .contains(&format!("位置 {t:.2} 格")));
            assert!(reading["text"]
                .as_str()
                .unwrap()
                .contains(if label == "fault" {
                    "标签 A"
                } else {
                    "标签 B"
                }));
            // Every new page starts at A/0, even after B's previous late position.
            assert!(result.body["observations"][0]["dom"]["text"]
                .as_str()
                .unwrap()
                .contains("活动 A；位置 0.00"));
            findings.push(json!({"environment":name,"batch":label,"candidate_id":id,"position":t,"geometry":"passed","label_fault":label=="fault"}));
        }
        let rollback = preview(
            &mut port,
            &output,
            &format!("{name}-rollback"),
            &id,
            viewport,
            vec![select(), seek(6, 0.5), seek(2, 0.25), seek(2, 0.25)],
        );
        for i in [3, 4] {
            position(&rollback, i, 2.25, "B");
            assert!(geometry(&rollback, i, 2.25, "B"));
            assert_eq!(
                marker(&rollback.images[i], [128, 0, 255]).1,
                45 * 4,
                "future trail must be removed"
            );
        }
        assert_eq!(
            marker(&rollback.images[3], [0, 160, 255]),
            marker(&rollback.images[4], [0, 160, 255])
        );
    }
    let desktop = REQUIRED_PREVIEW_ENVIRONMENTS[2].1;
    let omitted = preview(
        &mut port,
        &output,
        "omitted-selection",
        &id,
        desktop,
        vec![json!({"kind":"scroll","y":1100}), seek(6, 0.5)],
    );
    position(&omitted, 2, 6.5, "A");
    assert!(geometry(&omitted, 2, 6.5, "A"));
    let frozen = F2.replace(
        "const freezeGeometry = false;",
        "const freezeGeometry = true;",
    );
    std::fs::write(output.join("frozen-geometry.html"), &frozen).unwrap();
    let bad_id = candidate(&mut port, &frozen);
    let bad = preview(
        &mut port,
        &output,
        "frozen-geometry",
        &bad_id,
        desktop,
        vec![select(), seek(6, 0.5)],
    );
    position(&bad, 2, 6.5, "B");
    assert!(
        !geometry(&bad, 2, 6.5, "B"),
        "pixel oracle must catch state-correct but frozen geometry"
    );
    assert!(geometry(&bad, 2, 0.0, "B"));
    save(
        &output,
        "verification.json",
        &json!({"status":"passed","cases":findings,"rollback_repeat":"passed in three environments","omitted_selection":"A observed, B not reached","injected_frozen_geometry":"detected by pixels; scene receipts alone accept it","injected_label_fault":"visible only at 6.50","formal_receipts":port.previewed.get(&id),"delivery":"not attempted: planted content fault","provider_calls":0}),
    );
}

#[test]
#[ignore = "requires installed Chromium/Edge and new EX14_BROWSER_OUTPUT"]
fn ex14_browser_disk_parameter_cannot_accumulate() {
    let output = output("disk");
    let html=include_str!("../../../../docs/performance/ex11-local-demonstrations/ex11.7/batch12/runs/disk-1/delivered-raw.html");
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| {
        crate::run_scope::RunScope::capture(state, &turn, "ex14-disk", None, None)
    });
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let mut request = write(html);
    if let AuthorRequest::Write { libraries, .. } = &mut request {
        libraries.push(PresentationLibrary::Konva);
    }
    let id = port
        .author_presentation(request, &[], &[], &CancellationToken::default())
        .unwrap()
        .body["candidate_id"]
        .as_str()
        .unwrap()
        .to_string();
    app.with_app(|state| {
        let c = state
            .private_context()
            .read_presentation_candidate(&turn.session_id, &id)
            .unwrap();
        save(&output, "candidate.json", &serde_json::to_value(c).unwrap());
    });
    for (name, actions, expected) in [
        (
            "batch-1",
            vec![json!({"kind":"key","key":"ArrowLeft","selector":"#r2Range"}); 4],
            1.96,
        ),
        (
            "batch-2",
            vec![json!({"kind":"key","key":"ArrowLeft","selector":"#r2Range"}); 4],
            1.96,
        ),
        (
            "center-seek",
            vec![json!({"kind":"click","selector":"#r2Range"}), seek(2, 0.25)],
            2.0,
        ),
    ] {
        let request = json!({"operation":"preview","candidate_id":id,"viewport":REQUIRED_PREVIEW_ENVIRONMENTS[2].1,"read_selector":"#result","actions":actions});
        save(&output, &format!("{name}-request.json"), &request);
        let result = port
            .author_presentation(
                serde_json::from_value(request).unwrap(),
                &[],
                &[],
                &CancellationToken::default(),
            )
            .unwrap();
        save_result(&output, name, &result);
        assert!(
            result.body["errors"].as_array().unwrap().is_empty(),
            "{}",
            result.body
        );
        let controls = result.body["reading"]["controls"].as_array().unwrap();
        let r = controls.iter().find(|c| c["id"] == "r2Range").unwrap()["value"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!((r - expected).abs() < 1e-6);
        let initial = &result.body["observations"][0]["dom"]["controls"];
        assert_eq!(
            initial
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == "r2Range")
                .unwrap()["value"],
            "2"
        );
    }
    save(
        &output,
        "verification.json",
        &json!({"status":"passed","candidate_id":id,"target_r2":1.5,"initial_r2":2,"step":0.01,"required_arrow_left":50,"actions_per_preview":4,"batch_results":[1.96,1.96],"target_coverage":"not covered by author preview","supplement":"separate directed browser run on exported candidate","provider_calls":0}),
    );
}
