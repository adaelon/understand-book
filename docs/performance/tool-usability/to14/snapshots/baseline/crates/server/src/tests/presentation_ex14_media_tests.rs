//! EX14.1-M: real fixed runner, native Author previews and fixture pixel oracles.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use runtime::presentation::AnimationCue;
use runtime::presentation_preview::PreviewViewport;
use std::path::Path;

const SCENE: &str = include_str!("../../tests/fixtures/presentation-ex14-f1.py");
const PAGE: &str = include_str!("../../tests/fixtures/presentation-ex14-f2-m.html");

fn save(root: &Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn record(root: &Path, name: &str, result: &AuthorResult) {
    save(root, &format!("{name}-response.json"), &result.body);
    for (index, image) in result.images.iter().enumerate() {
        std::fs::write(
            root.join(format!("{name}-{index}.png")),
            STANDARD.decode(&image.png_base64).unwrap(),
        )
        .unwrap();
    }
    save(root, &format!("{name}-images.json"), &json!(result.images.iter().map(|i| json!({"caption":i.caption,"candidate_id":i.candidate_id,"environment_name":i.environment_name})).collect::<Vec<_>>()));
}
fn seek(time: f64) -> Value {
    json!({"kind":"seek","semantic_state":time.floor() as u32,"transition_progress":time.fract()})
}
fn preview(
    port: &mut impl ResidentStatePort,
    root: &Path,
    name: &str,
    id: &str,
    viewport: PreviewViewport,
    actions: Vec<Value>,
) -> AuthorResult {
    let request = json!({"operation":"preview","candidate_id":id,"viewport":viewport,"read_selector":"#readout","actions":actions});
    save(root, &format!("{name}-request.json"), &request);
    let result = port
        .author_presentation(
            serde_json::from_value(request).unwrap(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
    record(root, name, &result);
    result
}
fn passed(result: &AuthorResult) {
    assert_ne!(result.body["status"], "preview_failed", "{}", result.body);
    assert!(
        result.body["errors"].as_array().unwrap().is_empty(),
        "{}",
        result.body
    );
}
// H.264 and browser scaling change edge colors. Count only the solid interiors.
fn pixels(image: &PreviewImage, kind: &str) -> (f64, usize) {
    pixels_in(image, kind, None)
}
fn pixels_in(image: &PreviewImage, kind: &str, rect: Option<&Value>) -> (f64, usize) {
    let bytes = STANDARD.decode(&image.png_base64).unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).unwrap();
    let channels = frame.color_type.samples();
    let mut sum = 0usize;
    let mut count = 0usize;
    for (i, p) in buffer[..frame.buffer_size()]
        .chunks_exact(channels)
        .enumerate()
    {
        if let Some(rect) = rect {
            let x = (i % frame.width as usize) as f64;
            let y = (i / frame.width as usize) as f64;
            if x < rect["x"].as_f64().unwrap()
                || x >= rect["x"].as_f64().unwrap() + rect["width"].as_f64().unwrap()
                || y < rect["y"].as_f64().unwrap()
                || y >= rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap()
            {
                continue;
            }
        }
        let selected = match kind {
            "blue" => p[0] < 80 && p[1] > 120 && p[1] < 195 && p[2] > 215,
            "red" => p[0] > 210 && p[1] < 80 && p[2] < 45,
            "trail" => p[0] > 95 && p[0] < 160 && p[1] < 40 && p[2] > 210,
            _ => panic!("unknown fixture color"),
        };
        if selected {
            sum += i % frame.width as usize;
            count += 1;
        }
    }
    (
        if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        },
        count,
    )
}
fn geometry(result: &AuthorResult, index: usize, time: f64, active: &str) -> bool {
    let rect = result.body["observations"][index]["dom"]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == active)
        .unwrap();
    let width = rect["width"].as_f64().unwrap();
    let height = rect["height"].as_f64().unwrap();
    let scale = (width / 640.0).min(height / 360.0);
    let left = rect["x"].as_f64().unwrap() + (width - 640.0 * scale) / 2.0;
    // The scene's world frame is 128/9 wide; square center x=-5+1.25*t.
    let expected = left + (320.0 + 45.0 * (-5.0 + 1.25 * time)) * scale;
    // Restrict the object oracle to media pixels: LCD text outside the video can
    // contain blue subpixels too, particularly the logical/media readout.
    let (x, count) = pixels_in(&result.images[index], "blue", Some(rect));
    count > 8 && (x - expected).abs() <= 2.5
}
fn position(result: &AuthorResult, index: usize, time: f64) {
    let scene = &result.body["observations"][index]["scene"];
    assert_eq!(scene["actual"], scene["after_capture"]);
    assert_eq!(scene["actual"]["semantic_state"], time.floor() as u32);
    assert!((scene["actual"]["transition_progress"].as_f64().unwrap() - time.fract()).abs() < 1e-8);
    assert_eq!(scene["actual"]["playing"], false);
}
fn write_media(port: &mut impl ResidentStatePort, html: String, asset: &str) -> String {
    let initial = json!({"active_demo":"a","a_step":0,"a_progress":0,"a_time":0,"b_step":0,"b_progress":0,"b_time":0});
    let contract = json!({"active_demo":"a or b","a_step":"completed seconds 0..8","a_progress":"logical fraction [0,1)","a_time":"decoded seconds","b_step":"completed seconds 0..8","b_progress":"logical fraction [0,1)","b_time":"decoded seconds"});
    let request = json!({"operation":"write","title":"EX14 media engineering fixture","html":html,"readable_content":"Two independent fixed media demonstrations. A red WRONG A card is a controlled content fault.","source_ref_ids":[],"asset_refs":if asset.is_empty(){vec![]}else{vec![asset]},"initial_state":initial,"state_contract":contract,"assumptions":["Controlled engineering fixture; not source material or a model-produced explanation."]});
    port.author_presentation(
        serde_json::from_value(request).unwrap(),
        &[],
        &[],
        &CancellationToken::default(),
    )
    .unwrap()
    .body["candidate_id"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
#[ignore = "EX14.1-M needs configured Manim 0.21.0, browser and a new EX14_MEDIA_OUTPUT"]
fn ex14_manim_f1_f2_native_observations() {
    let root =
        std::path::PathBuf::from(std::env::var_os("EX14_MEDIA_OUTPUT").expect("EX14_MEDIA_OUTPUT"));
    std::fs::create_dir(&root).expect("use a new evidence directory");
    std::fs::write(root.join("scene.py"), SCENE).unwrap();
    let data = json!({"fault_start":5.8,"fault_end":6.2});
    let cues = vec![
        AnimationCue {
            id: "early".into(),
            label: "Early position".into(),
            at_seconds: 1.5,
        },
        AnimationCue {
            id: "late".into(),
            label: "Controlled wrong label".into(),
            at_seconds: 6.0,
        },
    ];
    save(
        &root,
        "input.json",
        &json!({"data":data,"cues":cues,"width":640,"height":360}),
    );
    let (_temp, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope =
        app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "ex14-media", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let start = std::time::Instant::now();
    let render = port
        .author_presentation(
            AuthorRequest::RenderAnimation {
                code: SCENE.into(),
                data,
                size: Some(PlotSize {
                    width: 640,
                    height: 360,
                }),
                cues,
            },
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
    record(&root, "render", &render);
    let asset_id = render.body["asset_ref"].as_str().unwrap().to_owned();
    let asset = &port.animations[&asset_id].asset;
    std::fs::write(
        root.join("animation.mp4"),
        STANDARD.decode(&asset.video_base64).unwrap(),
    )
    .unwrap();
    assert_eq!(asset.duration_seconds, 8.0);
    assert_eq!(asset.fps, 30.0);
    assert_eq!(render.images.len(), 4);
    let times: Vec<f64> = render.body["preview_images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["at_seconds"].as_f64().unwrap())
        .collect();
    for (actual, expected) in times.iter().zip([0.0, 1.5, 4.0, 8.0 - 1.0 / 30.0]) {
        assert!((actual - expected).abs() < 1e-6);
    }
    for image in &render.images {
        assert_eq!(
            pixels(image, "red").1,
            0,
            "initial samples must miss the fault"
        );
    }
    let html = PAGE.replace("@ASSET@", &asset_id);
    let id = write_media(&mut port, html.clone(), &asset_id);
    app.with_app(|s| {
        let candidate = s
            .private_context()
            .read_presentation_candidate(&turn.session_id, &id)
            .unwrap();
        save(
            &root,
            "candidate.json",
            &serde_json::to_value(candidate).unwrap(),
        );
    });
    let select = || json!({"kind":"click","selector":"#select-b"});
    let mut observations = vec![];
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        for (label, time) in [("before", 5.5), ("fault", 6.0), ("after", 6.5)] {
            let result = preview(
                &mut port,
                &root,
                &format!("{name}-{label}"),
                &id,
                viewport,
                vec![
                    json!({"kind":"scroll","y":1100}),
                    select(),
                    seek(4.25),
                    seek(time),
                ],
            );
            passed(&result);
            for (index, t) in [(3, 4.25), (4, time)] {
                position(&result, index, t);
                assert!(
                    geometry(&result, index, t, "b"),
                    "{name}/{label}/{t}: media geometry"
                );
            }
            let reading = &result.body["reading"];
            assert_eq!(reading["action_step"], 4);
            assert_eq!(reading["page_state"]["values"]["active_demo"], "b");
            let media = reading["page_state"]["values"]["b_time"].as_f64().unwrap();
            assert!((media - time).abs() < 1.0 / 30.0 + 1e-6);
            assert!(reading["text"]
                .as_str()
                .unwrap()
                .contains(&format!("逻辑 {time:.3}")));
            assert!(result.body["observations"][0]["dom"]["text"]
                .as_str()
                .unwrap()
                .contains("活动 A；逻辑 0.000"));
            let red = pixels(&result.images[4], "red").1;
            assert_eq!(
                red > 20,
                label == "fault",
                "wrong-label pixels: {name}/{label}"
            );
            observations.push(json!({"environment":name,"batch":label,"logical":time,"decoded":media,"red_pixels":red,"candidate_id":id}));
        }
        let result = preview(
            &mut port,
            &root,
            &format!("{name}-rollback"),
            &id,
            viewport,
            vec![select(), seek(6.0), seek(2.25), seek(2.25)],
        );
        passed(&result);
        for index in [3, 4] {
            position(&result, index, 2.25);
            assert!(geometry(&result, index, 2.25, "b"));
            assert_eq!(pixels(&result.images[index], "red").1, 0);
        }
        assert_eq!(
            pixels(&result.images[3], "blue"),
            pixels(&result.images[4], "blue")
        );
        assert_eq!(
            pixels(&result.images[3], "trail"),
            pixels(&result.images[4], "trail")
        );
        assert!(
            pixels(&result.images[4], "trail").1 < pixels(&result.images[2], "trail").1 / 2,
            "future trail remains"
        );
        let end = preview(
            &mut port,
            &root,
            &format!("{name}-endpoint"),
            &id,
            viewport,
            vec![select(), seek(8.0), seek(8.0)],
        );
        passed(&end);
        position(&end, 3, 8.0);
        assert!(geometry(&end, 3, 8.0 - 1.0 / 30.0, "b"));
        assert!(
            (end.body["reading"]["page_state"]["values"]["b_time"]
                .as_f64()
                .unwrap()
                - (8.0 - 1.0 / 30.0))
                .abs()
                < 1e-6
        );
    }
    let desktop = REQUIRED_PREVIEW_ENVIRONMENTS[2].1;
    let omitted = preview(
        &mut port,
        &root,
        "omitted-selection",
        &id,
        desktop,
        vec![json!({"kind":"scroll","y":1100}), seek(6.0)],
    );
    passed(&omitted);
    assert_eq!(
        omitted.body["reading"]["page_state"]["values"]["active_demo"],
        "a"
    );
    assert!(geometry(&omitted, 2, 6.0, "a"));
    let frozen_html = html.replace("const freezeMedia = false;", "const freezeMedia = true;");
    let frozen_id = write_media(&mut port, frozen_html, &asset_id);
    let frozen = preview(
        &mut port,
        &root,
        "frozen-media",
        &frozen_id,
        desktop,
        vec![select(), seek(6.0)],
    );
    passed(&frozen);
    position(&frozen, 2, 6.0);
    assert!(
        !geometry(&frozen, 2, 6.0, "b"),
        "oracle must detect lying logical state"
    );
    assert!(geometry(&frozen, 2, 0.0, "b"));
    assert_eq!(pixels(&frozen.images[2], "red").1, 0);
    let broken_html = PAGE
        .replace(
            "src=\"assets/@ASSET@.mp4\"",
            "src=\"data:video/mp4;base64,YmFk\"",
        )
        .replace("poster=\"assets/@ASSET@.png\"", "");
    let broken_id = write_media(&mut port, broken_html, "");
    let request = json!({"operation":"preview","candidate_id":broken_id,"viewport":desktop,"read_selector":"#readout","actions":[select(),seek(6.0)]});
    save(&root, "decode-failure-request.json", &request);
    // Rejected asynchronous seek is a tool execution error, before a completed
    // PreviewResult exists; it is not the status used for completed bad audits.
    let broken = port
        .author_presentation(
            serde_json::from_value(request).unwrap(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .err()
        .expect("damaged media must reject seek");
    save(
        &root,
        "decode-failure-error.json",
        &json!({"error_code":broken.error_code,"category":broken.category,"message":broken.message}),
    );
    assert_eq!(broken.error_code, "PRESENTATION_AUTHORING_FAILED");
    assert!(broken.message.contains("Media decode failed"));
    assert!(!port.previewed.contains_key(&broken_id));
    save(
        &root,
        "verification.json",
        &json!({"status":"passed","candidate_id":id,"asset_ref":asset_id,"sample_times":times,"cases":observations,"native_previews":18,"formal_receipts":port.previewed.get(&id),"rollback_repeat_endpoint":"passed in three environments","frozen_media":"reported position passes; actual pixel oracle detects failure","decode_failure":"PRESENTATION_AUTHORING_FAILED / Media decode failed; no receipt","delivery":"not attempted: controlled content fault","provider_calls":0,"elapsed_seconds":start.elapsed().as_secs_f64()}),
    );
}
