//! EX14.2/3: actual Resident requests, recorded observations, no provider.
use super::*;

pub(super) fn record_value(case: &str, name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("EX14_GUIDANCE_OUTPUT") {
        let path = std::path::PathBuf::from(root).join(case);
        std::fs::create_dir_all(&path).unwrap();
        let path = path.join(name);
        assert!(
            !path.exists(),
            "preserve evidence; use a new EX14_GUIDANCE_OUTPUT"
        );
        std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
}

fn record(case: &str, plans: &[AgentRequestPlan]) {
    for (index, plan) in plans.iter().enumerate() {
        let (native, _) = crate::native_chat_request_projection("ex14-controlled", plan);
        let react = crate::react_chat_request_projection("ex14-controlled", plan);
        let request = json!({"instructions":plan.instructions,"instruction_assets":plan.instruction_assets,
            "runtime_profile":plan.runtime_profile,"messages":plan.input,"usage":null,
            "tools":plan.tools.iter().map(|t| json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>(),
            "images":plan.preview_images.iter().map(|i| json!({"caption":i.caption,"candidate_id":i.candidate_id,"environment_name":i.environment_name,"png_base64":i.png_base64})).collect::<Vec<_>>()});
        for (kind, value) in [("request", request), ("native", native), ("react", react)] {
            record_value(case, &format!("{kind}-{index:02}.json"), &value);
        }
    }
}

fn guidance_count(plan: &AgentRequestPlan) -> usize {
    plan.ordered_messages()
        .iter()
        .filter(|m| {
            m.content
                .as_deref()
                .is_some_and(|s| s.starts_with("sampling_guidance.v1\n"))
        })
        .count()
}

#[test]
fn ex14_selected_guidance_reaches_sampling_without_static_or_global_leaks() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex14-selection");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "static",
                "local",
                "Static relationship",
                "Read the comparison",
                json!([]),
            ),
            prepare(
                "scene",
                "local",
                "Changing relationship",
                "Compare positions",
                json!(["continuous_scene"]),
            ),
            prepare(
                "review",
                "review",
                "Changing relationship",
                "Check the relationship",
                json!(["continuous_scene"]),
            ),
            prepare(
                "repeat",
                "review",
                "Changing relationship",
                "Check the relationship",
                json!(["continuous_scene", "state", "continuous_scene"]),
            ),
            prepare(
                "manim",
                "local",
                "Fixed media",
                "Check media",
                json!(["manim"]),
            ),
            prepare(
                "global",
                "global",
                "Reconsider order",
                "Choose representation",
                json!(["manim"]),
            ),
            turn_final("Draft planning remains."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    let plans = adapter.seen_plans.borrow();
    record("selection", &plans);
    assert_eq!(plans.len(), 8);
    for (index, name, references) in [
        (1, "global", vec![]),
        (2, "local", vec![]),
        (3, "local", vec!["state", "continuous-scene"]),
        (4, "review", vec!["state", "continuous-scene"]),
        (5, "review", vec!["state", "continuous-scene"]),
        (6, "local", vec!["state", "continuous-scene", "manim"]),
        (7, "global", vec![]),
    ] {
        phase(&plans[index], name, &references);
        assert_eq!(plans[index].instructions, plans[1].instructions);
        assert_eq!(selection_state(&plans[index]).unwrap()["phase"], name);
        for asset in plans[index]
            .instruction_assets
            .iter()
            .filter(|a| a.asset_id.contains("presentation"))
        {
            assert_eq!(asset.revision, crate::agent_prompt::presentation::REVISION);
        }
    }
    for plan in &plans[..3] {
        for text in [
            "观察关系真正发生的位置",
            "prerequisite actions plus target actions",
            "requestVideoFrameCallback",
            "presentationScene",
        ] {
            assert!(
                !serde_json::to_string(&plan.ordered_messages())
                    .unwrap()
                    .contains(text),
                "unexpected early guidance: {text}"
            );
        }
    }
    let local = serde_json::to_string(&plans[3].ordered_messages()).unwrap();
    assert!(local.contains("prerequisite actions plus target actions"));
    assert!(!local.contains("观察关系真正发生的位置"));
    let review = serde_json::to_string(&plans[4].ordered_messages()).unwrap();
    assert!(review.contains("观察关系真正发生的位置"));
    assert!(review.contains("修改后复查当前候选"));
    assert_eq!(guidance_count(&plans[4]), guidance_count(&plans[5]));
    let last = plans[7]
        .ordered_messages()
        .into_iter()
        .filter_map(|m| m.content)
        .filter(|s| s.starts_with("sampling_guidance.v1\n"))
        .last()
        .unwrap();
    assert!(!last.contains("requestVideoFrameCallback"));
    assert!(!last.contains("Continuous scene contract"));
    let visual = include_str!("../../../skills/presentation/phases/local.md").trim();
    for index in [2, 3, 6] {
        let selected = plans[index]
            .ordered_messages()
            .into_iter()
            .filter_map(|m| m.content)
            .filter(|s| s.starts_with("sampling_guidance.v1\n"))
            .last()
            .unwrap();
        assert!(
            selected.contains(visual),
            "local methods missing at sampling {index}"
        );
        for wire in [
            crate::native_chat_request_projection("ex14-controlled", &plans[index]).0,
            crate::react_chat_request_projection("ex14-controlled", &plans[index]),
        ] {
            assert!(wire["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["content"].as_str().is_some_and(|s| s.contains(visual))));
        }
    }
    for index in [1, 4, 7] {
        let selected = plans[index]
            .ordered_messages()
            .into_iter()
            .filter_map(|m| m.content)
            .filter(|s| s.starts_with("sampling_guidance.v1\n"))
            .last()
            .unwrap();
        assert!(
            !selected.contains("对象对应不清楚时"),
            "local methods leaked at sampling {index}"
        );
    }
    // Media additions follow the accepted baseline without replacing its contracts.
    let manim = include_str!("../../../skills/presentation/references/manim.md").trim();
    assert!(manim.starts_with(include_str!("../../../docs/performance/presentation-critical-moments-ex14/baseline/source/skills/presentation/references/manim.md").trim()));
    assert!(manim.contains("limited initial screening"));
    for (index, plan) in plans.iter().enumerate() {
        let selected = plan.ordered_messages().into_iter().filter_map(|m| m.content)
            .filter(|s| s.starts_with("sampling_guidance.v1\n")).last().unwrap_or_default();
        assert_eq!(selected.contains(manim), index == 6);
        for wire in [crate::native_chat_request_projection("ex14-controlled", plan).0,
                     crate::react_chat_request_projection("ex14-controlled", plan)] {
            let contents: Vec<_> = wire["messages"].as_array().unwrap().iter()
                .filter_map(|m| m["content"].as_str()).collect();
            // Previously sampled guidance remains at its historical anchor.
            assert_eq!(contents.iter().any(|s| s.contains(manim)), index >= 6);
        }
    }
    assert_eq!((port.writes, port.deliveries), (0, 0));
}

#[test]
fn ex14_manim_review_guidance_is_selected_once_and_survives_compaction() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex14-media-selection");
    let adapter = RequestPlanRecordingAdapter::new(vec![discover(),
        prepare("media-review", "review", "Fixed process", "Inspect the late relationship", json!(["manim"])),
        prepare("repeat-media", "review", "Fixed process", "Inspect the late relationship", json!(["manim", "state"])),
        prepare("static-review", "review", "Static relationship", "Read the labels", json!([])),
        turn_final("Review planning complete.")], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    let plans = adapter.seen_plans.borrow();
    record("media-selection", &plans);
    let manim = include_str!("../../../skills/presentation/references/manim.md").trim();
    for index in [2, 3] {
        phase(&plans[index], "review", &["state", "continuous-scene", "manim"]);
        for wire in [crate::native_chat_request_projection("ex14-controlled", &plans[index]).0,
                     crate::react_chat_request_projection("ex14-controlled", &plans[index])] {
            assert_eq!(wire["messages"].as_array().unwrap().iter()
                .filter_map(|m| m["content"].as_str()).filter(|s| s.contains(manim)).count(), 1);
        }
    }
    assert_eq!(guidance_count(&plans[2]), guidance_count(&plans[3]));
    phase(&plans[4], "review", &[]);
    let selected = plans[4].ordered_messages().into_iter().filter_map(|m| m.content)
        .filter(|s| s.starts_with("sampling_guidance.v1\n")).last().unwrap();
    assert!(!selected.contains("limited initial screening"));
    drop(plans);
    check_framework_survives_mid_turn_compaction("review", json!(["manim"]));
}

#[test]
fn ex14_plain_request_has_no_author_or_presentation_guidance() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex14-plain");
    let question =
        "请只用文字解释这里一个必要条件和充分条件的区别，并各给一个与材料一致的例子，不做演示页";
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("A text explanation.")], vec![]);
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    run_context(
        &b,
        &mut port,
        &adapter,
        &mut context,
        &snapshot,
        &ResidentTurnResources::default(),
        None,
        &mut EphemeralCompactionCheckpointSink::default(),
        question,
        "t0",
    )
    .unwrap();
    let plans = adapter.seen_plans.borrow();
    record("plain", &plans);
    assert_eq!(plans.len(), 1);
    assert!(plans[0]
        .tools
        .iter()
        .all(|t| t.name != "presentation.author"));
    assert!(plans[0]
        .instruction_assets
        .iter()
        .all(|a| !a.asset_id.contains("presentation")));
    assert!(!serde_json::to_string(&plans[0].ordered_messages())
        .unwrap()
        .contains("Presentation method"));
    assert_eq!((port.writes, port.deliveries), (0, 0));
}

#[test]
fn ex14_recorded_critical_images_reach_next_wire_request_and_not_delivery_early() {
    // Exact EX14.1-B desktop 6.50 receipt/PNGs. Only the candidate ID is remapped
    // to this scripted store's c1; this tests transport, not a new browser run.
    let fixture: Value = serde_json::from_str(include_str!("testdata/ex14-preview.json")).unwrap();
    let original = fixture["body"]["candidate_id"].as_str().unwrap();
    let replay: Value = serde_json::from_str(&fixture.to_string().replace(original, "c1")).unwrap();
    let images: Vec<PreviewImage> = replay["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| PreviewImage {
            caption: i["caption"].as_str().unwrap().into(),
            png_base64: i["png_base64"].as_str().unwrap().into(),
            candidate_id: Some("c1".into()),
            environment_name: Some("desktop-content".into()),
        })
        .collect();
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex14-images");
    port.recorded_preview = Some(AuthorResult {
        body: replay["body"].clone(),
        images: images.clone(),
        previewed_candidate: Some("c1".into()),
        delivered: None,
    });
    let preview = json!({"operation":"preview","candidate_id":"c1","viewport":{"width":960,"height":720,"input":"mouse"},"read_selector":"#readout",
        "actions":[{"kind":"scroll","y":1100},{"kind":"click","selector":"#select-b"},{"kind":"seek","semantic_state":4,"transition_progress":0.25},{"kind":"seek","semantic_state":6,"transition_progress":0.5}]});
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "review",
                "review",
                "Compare B positions",
                "Check labels",
                json!(["continuous_scene"]),
            ),
            write("write"),
            turn_calls(vec![
                call("preview", "presentation.author", &preview.to_string()),
                call(
                    "early",
                    "presentation.author",
                    r#"{"operation":"deliver","candidate_id":"c1"}"#,
                ),
            ]),
            author(
                "patch",
                json!({"operation":"patch","candidate_id":"c1","edits":[{"old_text":"Example","new_text":"Revised"}]}),
            ),
            author("stale", json!({"operation":"deliver","candidate_id":"c2"})),
            turn_final("Revised draft awaits its own preview."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!(port.deliveries, 0);
    for id in ["early", "stale"] {
        assert!(tool_result(&context.messages, id).contains("PRESENTATION_INSPECTION_REQUIRED"));
    }
    let plans = adapter.seen_plans.borrow();
    record("images", &plans);
    let next = &plans[4];
    assert_eq!(next.preview_images.len(), images.len());
    let envelope: Value = serde_json::from_str(tool_result(&next.input, "preview")).unwrap();
    let receipt = &envelope["model_body"];
    assert_eq!(receipt["reading"]["action_step"], 4);
    assert_eq!(receipt["reading"]["page_state"]["values"]["active"], "B");
    assert!(receipt["reading"]["text"]
        .as_str()
        .unwrap()
        .contains("6.50"));
    let (native, _) = crate::native_chat_request_projection("ex14-controlled", next);
    let react = crate::react_chat_request_projection("ex14-controlled", next);
    for wire in [native, react] {
        let messages = wire["messages"].as_array().unwrap();
        let content = messages.last().unwrap()["content"].as_array().unwrap();
        for image in &images {
            assert!(content.iter().any(|part| part["image_url"]["url"]
                == format!("data:image/png;base64,{}", image.png_base64)));
            assert!(content.iter().any(|part| part["text"]
                .as_str()
                .is_some_and(|s| s.contains(&image.caption))));
        }
    }
    let critical = &next.preview_images[4];
    assert_eq!(critical.candidate_id.as_deref(), Some("c1"));
    assert_eq!(
        critical.environment_name.as_deref(),
        Some("desktop-content")
    );
    assert!(critical.caption.contains("step 4"));
    assert!(critical
        .caption
        .contains("semantic step 6, transition 0.500"));
    assert!(plans[5].preview_images.is_empty());
}

#[test]
fn ex14_review_scene_guidance_survives_compaction() {
    check_framework_survives_mid_turn_compaction("review", json!(["continuous_scene"]));
}

#[test]
fn ex14_visual_static_revision_loads_local_and_keeps_scripted_patch_scope() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../docs/performance/presentation-critical-moments-ex14/materials/N3/version.json"
    ))
    .unwrap();
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex14-static-revision");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "local",
                "local",
                "保留两个例子和版式",
                "只修订充分条件措辞",
                json!(["editing"]),
            ),
            author(
                "patch",
                json!({"operation":"patch","reference":baseline["reference"],
            "edits":[{"old_text":"只要 A 就 B","new_text":"A 成立时 B 一定成立"}]}),
            ),
            author(
                "preview",
                json!({"operation":"preview","candidate_id":"c1"}),
            ),
            prepare(
                "review",
                "review",
                "保留两个例子和版式",
                "核对措辞与原例子",
                json!(["editing"]),
            ),
            author(
                "deliver",
                json!({"operation":"deliver","candidate_id":"c1"}),
            ),
            turn_final("措辞已修订。"),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    run_context(&b, &mut port, &adapter, &mut context, &snapshot,
        &ResidentTurnResources::default(), None, &mut EphemeralCompactionCheckpointSink::default(),
        "请检查这张必要条件与充分条件说明页，把‘只要 A 就 B’改成‘A 成立时 B 一定成立’，保留原来的两个例子和版式。", "t0").unwrap();
    let plans = adapter.seen_plans.borrow();
    record("static-revision", &plans);
    phase(&plans[2], "local", &["editing"]);
    assert!(serde_json::to_string(&plans[2].ordered_messages())
        .unwrap()
        .contains("已有页面的措辞小修订保持原例子和版式"));
    assert!(!serde_json::to_string(&plans[2].ordered_messages())
        .unwrap()
        .contains("presentationScene"));
    let calls: Vec<Value> = context
        .messages
        .iter()
        .flat_map(|m| &m.tool_calls)
        .filter(|c| c.name == "presentation.author")
        .map(|c| serde_json::from_str(&c.arguments).unwrap())
        .collect();
    assert_eq!(
        calls
            .iter()
            .map(|c| c["operation"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["prepare", "patch", "preview", "prepare", "deliver"]
    );
    assert_eq!(calls[1]["reference"], baseline["reference"]);
    assert_eq!(calls[1]["edits"].as_array().unwrap().len(), 1);
    assert_eq!((port.writes, port.deliveries), (1, 1));
    // Scripted actions prove the existing route accepts a bounded edit with C
    // loaded. Natural model choice and real host delivery are separate evidence.
}
