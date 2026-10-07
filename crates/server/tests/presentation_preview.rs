use runtime::{presentation_preview::*, run_context::CancellationToken};
use server::presentation_preview::BrowserPreview;
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

fn request(html: &str, actions: Vec<PreviewAction>) -> PreviewRequest {
    PreviewRequest {
        candidate_id: "rp1-recall-1".into(),
        read_selector: None,
        html: html.into(),
        actions,
        width: None,
        viewport: None,
    }
}
fn click(selector: &str) -> PreviewAction {
    PreviewAction::Click {
        selector: selector.into(),
    }
}
fn host() -> BrowserPreview {
    BrowserPreview::discover().expect("install preview browser for RP1 execution tests")
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn ex12_host_source_buttons_meet_touch_contract() {
    let html = format!("<style>{}</style><p>正文 <button data-source-ref='ref'>[1]</button></p>",
        include_str!("../../../packages/web/src/presentation.css"));
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let mut input = request(&html, vec![]);
        input.viewport = Some(viewport);
        let report = host().preview(&input,&CancellationToken::default()).unwrap();
        assert!(report.errors.is_empty());
        assert!(report.observations[0].issues.is_empty(), "{viewport:?}: {:?}", report.observations[0].issues);
    }
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn ex12_scroll_observes_document_positions_without_changing_controls() {
    use base64::Engine;
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let mut input = request(include_str!("fixtures/presentation-scroll.html"), vec![
            PreviewAction::Scroll { y: 950 },
            PreviewAction::Scroll { y: u32::MAX },
            PreviewAction::Scroll { y: 0 },
        ]);
        input.viewport = Some(viewport);
        let report = host().preview(&input, &CancellationToken::default()).unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.candidate_id, input.candidate_id);
        assert_eq!(report.environment, viewport);
        for (index, observation) in report.observations.iter().enumerate() {
            assert!(observation.issues.is_empty(), "{:?}", observation.issues);
            let y = [0.0, 950.0, 2700.0 - f64::from(viewport.height), 0.0][index];
            assert_eq!(observation.scroll.y, y);
            assert_eq!(observation.scroll.viewport_height, viewport.height);
            assert_eq!(observation.scroll.viewport_width, viewport.width);
            assert_eq!(observation.layout["cssLayoutViewport"]["pageY"].as_f64(), Some(y));
            let region = ["start", "middle", "end", "start"][index];
            let element = observation.dom["elements"].as_array().unwrap().iter().find(|e| e["id"] == region).unwrap();
            let document_y = [0.0, 900.0, 1800.0, 0.0][index];
            assert_eq!(element["y"].as_f64(), Some(document_y - y));
            let controls = observation.dom["controls"].as_array().unwrap();
            assert_eq!(controls.iter().find(|e| e["id"] == "change").unwrap()["text"], "0");
            assert_eq!(controls.iter().find(|e| e["id"] == "parameter").unwrap()["value"], "3");
            let png = base64::engine::general_purpose::STANDARD.decode(&observation.screenshot_png_base64).unwrap();
            let mut decoder = png::Decoder::new(std::io::Cursor::new(&png)).read_info().unwrap();
            let mut pixels = vec![0; decoder.output_buffer_size()];
            let info = decoder.next_frame(&mut pixels).unwrap();
            assert_eq!((info.width, info.height), (viewport.width, viewport.height));
            let channels = info.color_type.samples();
            let offset = ((viewport.height / 2 * info.width + viewport.width - 30) as usize) * channels;
            let expected = [[220,80,80], [80,180,100], [80,120,220], [220,80,80]][index];
            assert_eq!(&pixels[offset..offset+3], &expected);
            if let Some(dir) = std::env::var_os("EX12_EVIDENCE_DIR") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(format!("{}-{index}.png", report.environment_name)), png).unwrap();
            }
        }
        assert!(report.observations[1].dom["visible_text"].as_str().unwrap().contains("MIDDLE"), "{}", report.observations[1].dom);
        assert!(!report.observations[1].dom["visible_text"].as_str().unwrap().contains("START"));
        assert!(report.observations[2].dom["visible_text"].as_str().unwrap().contains("END"));
        assert!(matches!(report.observations[2].action, Some(PreviewAction::Scroll { y: u32::MAX })));
        if let Some(dir) = std::env::var_os("EX12_EVIDENCE_DIR") {
            let mut value = serde_json::to_value(&report).unwrap();
            for o in value["observations"].as_array_mut().unwrap() { o.as_object_mut().unwrap().remove("screenshot_png_base64"); }
            std::fs::write(std::path::PathBuf::from(dir).join(format!("{}.json", report.environment_name)), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
    }
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn selected_result_rejects_unreadable_regions_and_running_scene() {
    for (html, selector, message) in [
        ("<p>test</p>", "#missing", "matched 0"),
        ("<p>a</p><p>b</p>", "p", "matched 2"),
        ("<p style='display:none'>hidden</p>", "p", "hidden"),
        ("<p>value</p><script>window.presentationScene={snapshot:()=>({playing:true})}</script>", "p", "Pause or seek"),
    ] {
        let mut input = request(html, vec![]);
        input.read_selector = Some(selector.into());
        let error = host().preview(&input, &CancellationToken::default()).unwrap_err();
        assert!(error.message.contains(message), "{}", error.message);
    }
    let mut input = request(&format!("<p>{}</p>", "a".repeat(9000)), vec![]);
    input.read_selector = Some("p".into());
    let error = host().preview(&input, &CancellationToken::default()).unwrap_err();
    assert!(error.message.contains("8192 bytes"), "{}", error.message);
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn selected_result_reads_beyond_general_dom_excerpt() {
    let mut input = request(&format!("<p>{}</p><p id='result'>k=5; w=12.756480</p>", "intro ".repeat(3000)), vec![]);
    input.read_selector = Some("#result".into());
    let report = host().preview(&input, &CancellationToken::default()).unwrap();
    assert!(!report.observations[0].dom["text"].as_str().unwrap().contains("12.756480"));
    assert_eq!(report.observations[0].reading.as_ref().unwrap()["text"], "k=5; w=12.756480");
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_repeated_launches_release_profiles() {
    let browser = host();
    for _ in 0..3 {
        // preview returns success only after its owned process and profile are removed.
        let report = browser
            .preview(
                &request(
                    "<button id='change' onclick=\"this.textContent='changed'\">change</button>",
                    vec![click("#change")],
                ),
                &CancellationToken::default(),
            )
            .unwrap();
        assert!(report.observations.last().unwrap().dom["text"]
            .as_str()
            .unwrap()
            .contains("changed"));
    }
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_key_targets_the_requested_control() {
    let actions = serde_json::from_value(serde_json::json!([
        {"kind":"click","selector":"#complete"},
        {"kind":"key","selector":"#count","key":"Home"},
        {"kind":"key","key":"ArrowRight"}
    ]))
    .unwrap();
    let report = host()
        .preview(
            &request(include_str!("fixtures/presentation-recall.html"), actions),
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(report.observations[2].dom["text"]
        .as_str()
        .unwrap()
        .contains("0/3 = 0.000"));
    assert!(report.observations[3].dom["text"]
        .as_str()
        .unwrap()
        .contains("1/3 = 0.333"));
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_uses_requested_narrow_viewport() {
    let mut input = request("<style>body{margin:0}.wide{display:block}@media(max-width:400px){.wide{display:none}}</style><p class=wide>Wide layout</p><p>Narrow ready</p>", vec![]);
    input.width = Some(340);
    let report = host()
        .preview(&input, &CancellationToken::default())
        .unwrap();
    assert_eq!(
        report.observations[0].layout["cssLayoutViewport"]["clientWidth"],
        340
    );
    assert_eq!(
        report.environment,
        PreviewViewport {
            width: 340,
            height: 720,
            input: PreviewInput::Mouse
        }
    );
    assert!(!report.observations[0].dom["text"]
        .as_str()
        .unwrap()
        .contains("Wide layout"));
    input.width = Some(0);
    assert!(host()
        .preview(&input, &CancellationToken::default())
        .unwrap_err()
        .message
        .contains("width"));
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_uses_touch_input_and_reports_environment_layout_issues() {
    let html = r#"<style>body{margin:0;width:900px}button{width:30px;height:30px}</style>
      <button id='tap'>tap</button><input id='tiny-input' style='width:20px;height:20px'><output id='kind'>none</output>
      <script>tap.addEventListener('touchstart',()=>kind.textContent='touch');tap.addEventListener('mousedown',()=>{if(kind.textContent==='none')kind.textContent='mouse'});</script>"#;
    let mut touch = request(html, vec![click("#tap")]);
    touch.viewport = Some(PreviewViewport {
        width: 320,
        height: 420,
        input: PreviewInput::Touch,
    });
    let report = host()
        .preview(&touch, &CancellationToken::default())
        .unwrap();
    assert_eq!(report.environment_name, "narrow-content");
    assert!(report.observations.last().unwrap().dom["text"]
        .as_str()
        .unwrap()
        .contains("touch"));
    assert!(report.observations[0]
        .issues
        .iter()
        .any(|issue| issue.kind == "geometry_overflow"));
    assert!(report.observations[0]
        .issues
        .iter()
        .any(|issue| issue.kind == "touch_target_small"));
    assert!(report.observations[0]
        .issues
        .iter()
        .any(|issue| issue.kind == "touch_target_small" && issue.message.contains("tiny-input")));

    let mut mouse = request(html, vec![click("#tap")]);
    mouse.viewport = Some(PreviewViewport {
        width: 960,
        height: 720,
        input: PreviewInput::Mouse,
    });
    let report = host()
        .preview(&mouse, &CancellationToken::default())
        .unwrap();
    assert!(report.observations.last().unwrap().dom["text"]
        .as_str()
        .unwrap()
        .contains("mouse"));

    let mut combined_boundary = request(
        "<style>body{margin:0}button{min-width:44px;min-height:44px;max-width:100%}</style><button>ready</button>",
        vec![],
    );
    combined_boundary.viewport = Some(PreviewViewport {
        width: 320,
        height: 240,
        input: PreviewInput::Touch,
    });
    let report = host()
        .preview(&combined_boundary, &CancellationToken::default())
        .unwrap();
    assert_eq!(
        report.observations[0].layout["cssLayoutViewport"]["clientWidth"],
        320
    );
    assert_eq!(
        report.observations[0].layout["cssLayoutViewport"]["clientHeight"],
        240
    );
    assert!(
        report.observations[0].issues.is_empty(),
        "{:?}",
        report.observations[0].issues
    );
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_executes_graphics_and_real_input() {
    let report = host()
        .preview(
            &request(
                include_str!("fixtures/presentation-recall.html"),
                vec![
                    click("#irrelevant"),
                    click("#complete"),
                    click("#count"),
                    PreviewAction::Key {
                        key: "End".into(),
                        selector: None,
                    },
                    PreviewAction::Key {
                        key: "ArrowLeft".into(),
                        selector: None,
                    },
                ],
            ),
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let observations = &report.observations;
    assert!(observations[0].dom["text"]
        .as_str()
        .unwrap()
        .contains("2/3 = 0.667"));
    assert!(observations[1].dom["text"]
        .as_str()
        .unwrap()
        .contains("2/3 = 0.667"));
    assert!(observations[1].dom["text"]
        .as_str()
        .unwrap()
        .contains("无关材料：1"));
    assert!(observations[2].dom["text"]
        .as_str()
        .unwrap()
        .contains("3/3 = 1.000"));
    assert!(observations[5].dom["text"]
        .as_str()
        .unwrap()
        .contains("2/3 = 0.667"));
    assert_ne!(
        observations[1].screenshot_png_base64,
        observations[2].screenshot_png_base64
    );
    assert!(observations
        .iter()
        .all(|o| o.screenshot_png_base64.starts_with("iVBOR")
            && o.screenshot_png_base64.len() > 1000));
    assert_eq!(
        observations[0].layout["cssLayoutViewport"]["clientWidth"],
        960
    );
    assert!(observations[0].dom["elements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["tag"] == "CANVAS" && e["width"] == 360));
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_attributes_script_errors_and_failed_actions() {
    let report = host().preview(&request("<body><h1>candidate error</h1><script>throw new Error('rp1-deliberate-failure')</script></body>", vec![]), &CancellationToken::default()).unwrap();
    assert_eq!(report.candidate_id, "rp1-recall-1");
    assert!(report
        .errors
        .iter()
        .any(|e| e.to_string().contains("rp1-deliberate-failure")));
    let error = host()
        .preview(
            &request("<body>no button</body>", vec![click("#absent")]),
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert_eq!(error.phase, "interact:1");
    assert_eq!(error.candidate_id, "rp1-recall-1");
    assert!(error.message.contains("control not found"));
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_stops_hung_javascript_on_cancel_and_deadline() {
    let request = request("<body><script>while(true){}</script></body>", vec![]);
    let stop = Arc::new(AtomicBool::new(false));
    let cancellation = CancellationToken::with_host_stop(stop.clone());
    let trigger = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        stop.store(true, std::sync::atomic::Ordering::Release);
    });
    let start = Instant::now();
    let error = host().preview(&request, &cancellation).unwrap_err();
    trigger.join().unwrap();
    assert_eq!(error.message, "AGENT_RUN_CANCELLED");
    assert!(start.elapsed() < Duration::from_secs(8));
    let mut host = host();
    host.timeout = Duration::from_secs(2);
    let error = host
        .preview(&request, &CancellationToken::default())
        .unwrap_err();
    assert_eq!(error.message, "PREVIEW_TIMEOUT");
}

#[test]
fn presentation_preview_pre_cancelled_never_launches_browser() {
    let host = BrowserPreview {
        executable: "missing-browser".into(),
        timeout: Duration::from_secs(1),
    };
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    assert_eq!(
        host.preview(&request("", vec![]), &cancellation)
            .unwrap_err()
            .message,
        "AGENT_RUN_CANCELLED"
    );
}

#[cfg(unix)]
#[test]
fn presentation_preview_reports_browser_startup_stderr() {
    let host = BrowserPreview {
        executable: "/bin/sh".into(),
        timeout: Duration::from_secs(3),
    };
    let error = host
        .preview(
            &request("<body>test</body>", vec![]),
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert_eq!(error.phase, "launch");
    assert_eq!(error.candidate_id, "rp1-recall-1");
    assert!(error.message.contains("exited before ready"));
    assert!(error.message.contains("option"), "{}", error.message);
}

#[test]
#[ignore = "requires a real installed Chromium/Edge browser"]
fn presentation_preview_has_no_reader_access_or_network_side_effects() {
    use std::io::ErrorKind;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let html = format!(
        r#"<body><output id="scope"></output><script>
    scope.value=JSON.stringify({{tauri:typeof window.__TAURI_INTERNALS__,parentIsSelf:parent===window}});
    fetch('http://{}/reader/state').catch(()=>{{}});
    </script></body>"#,
        listener.local_addr().unwrap()
    );
    let report = host()
        .preview(&request(&html, vec![]), &CancellationToken::default())
        .unwrap();
    let text = report.observations[0].dom["text"].as_str().unwrap();
    assert!(
        text.contains("undefined") && text.contains("true"),
        "{text}"
    );
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
    assert!(report
        .errors
        .iter()
        .any(|e| e["method"] == "Network.loadingFailed"));
}
