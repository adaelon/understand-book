use super::*;

#[test]
fn source_marker_single_bracket_colon_adjacent_mixed_groups_keep_every_source_once() {
    let bindings = [
        source_binding_fixture("source_ref_one", "1.1"),
        source_binding_fixture("source_ref_two", "1.1"),
        source_binding_fixture("source_ref_three", "1.1"),
    ];
    for answer in [
        "A claim[source:source_ref_one][source:source_ref_two][[source:source_ref_three]][source:source_ref_one]。",
        "A claim[[source:source_ref_three]][source:source_ref_one][source:source_ref_two][source:source_ref_two]。",
    ] {
        let compiled = compile_agent_answer(answer, &bindings, &AnswerProvenanceLedger::default()).unwrap();
        assert_eq!(compiled.answer, "A claim。");
        assert_eq!(compiled.bindings.len(), 3);
        assert_eq!(compiled.view.sources.len(), 3);
        let groups: Vec<_> = compiled.view.parts.iter().filter_map(|part| match part {
            AgentAnswerPart::Sources { source_ref_ids } => Some(source_ref_ids),
            _ => None,
        }).collect();
        assert_eq!(groups.len(), 1);
        let expected = if answer.starts_with("A claim[[source:") {
            vec!["source_ref_three", "source_ref_one", "source_ref_two"]
        } else {
            vec!["source_ref_one", "source_ref_two", "source_ref_three"]
        };
        assert_eq!(groups[0].iter().map(String::as_str).collect::<Vec<_>>(), expected);
    }
}

#[test]
fn source_marker_single_bracket_colon_real_answer_keeps_three_clickable_sources_without_repair() {
    let ids = [
        "source_ref_ce6a0d647a0841e4",
        "source_ref_3ac057ba55a8af95",
        "source_ref_6c4fce31795efda1",
    ];
    let mut bindings: Vec<_> = ids
        .iter()
        .map(|id| source_binding_fixture(id, "1.1"))
        .collect();
    bindings.push(source_binding_fixture("source_ref_unused", "1.1"));
    // The single-bracket markers and these IDs occurred in cross-02's real final answer.
    let answer = format!(
        "**一个在抬高分子 I，一个在压低分母 H**[source:{}]。\n\n\
        **压低分母 H(Y)，把容量留给真信号**[source:{}]。\n\n\
        你只在乎它把该买的排在前面、该卖的排在后面[source:{}]。\n\n\
        两处原文支持相对排序。[source:{}] [[source:{}]]",
        ids[0], ids[1], ids[2], ids[1], ids[0]
    );
    let adapter = FakeAdapter::new(vec![], vec![]);
    let delivery = deliver_agent_answer(
        &answer,
        &bindings,
        &AnswerProvenanceLedger::default(),
        &adapter,
        &ModelRuntimeProfile::fallback("test", ProviderToolProtocol::Native),
        None,
        None,
        &[],
    )
    .unwrap();
    assert!(!delivery.incomplete);
    assert_eq!(delivery.extra_turns, 0);
    assert_eq!(delivery.extra_tokens, 0);
    assert!(delivery.diagnostics.is_none());
    assert_eq!(delivery.compiled.bindings.len(), 3);
    assert_eq!(
        delivery
            .compiled
            .view
            .sources
            .iter()
            .map(|source| source.source_ref_id.as_str())
            .collect::<Vec<_>>(),
        ids
    );
    let groups: Vec<_> = delivery
        .compiled
        .view
        .parts
        .iter()
        .filter_map(|part| match part {
            AgentAnswerPart::Sources { source_ref_ids } => Some(
                source_ref_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        groups,
        vec![
            vec![ids[0]],
            vec![ids[1]],
            vec![ids[2]],
            vec![ids[1], ids[0]]
        ]
    );
    assert!(!delivery.compiled.answer.contains("source_ref_"));
    assert!(!delivery.compiled.answer.contains("[source:"));
    assert!(delivery
        .compiled
        .answer
        .contains("**压低分母 H(Y)，把容量留给真信号**。"));
    let presentation = compile_presentation_text(&answer, &bindings, &[]).unwrap();
    assert_eq!(presentation.sources.len(), 3);
    assert_eq!(presentation.parts, delivery.compiled.view.parts);
}

#[test]
fn source_marker_single_bracket_colon_unknown_and_malformed_are_delivery_errors() {
    let bindings = [source_binding_fixture("source_ref_one", "1.1")];
    for (answer, expected) in [
        ("A claim.[source:source_ref_missing]", "UNKNOWN_SOURCE_REF"),
        ("A claim.[source:]", "INVALID_SOURCE_MARKER"),
        ("A claim.[source:source_ref_one", "INVALID_SOURCE_MARKER"),
        (
            "A claim.[source:source_ref_one bad]",
            "INVALID_SOURCE_MARKER",
        ),
    ] {
        let error = compile_agent_answer(answer, &bindings, &AnswerProvenanceLedger::default())
            .expect_err("an attempted citation must not silently become an unsourced success");
        assert_eq!(error.issues[0].error_code, expected, "{answer}");
    }
}

#[test]
fn source_marker_single_bracket_colon_uses_existing_one_repair_then_failure_policy() {
    let bindings = [source_binding_fixture("source_ref_one", "1.1")];
    for (repair, succeeds) in [
        ("A supported claim.[[source:source_ref_one]]", true),
        ("Still unknown.[source:source_ref_missing]", false),
    ] {
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final(repair)], vec![]);
        let delivery = deliver_agent_answer(
            "A claim.[source:source_ref_missing]",
            &bindings,
            &AnswerProvenanceLedger::default(),
            &adapter,
            &ModelRuntimeProfile::fallback("test", ProviderToolProtocol::Native),
            None,
            None,
            &[],
        )
        .unwrap();
        assert_eq!(adapter.seen_plans.borrow().len(), 1);
        assert_eq!(delivery.extra_turns, 1);
        let diagnostics = delivery.diagnostics.as_ref().unwrap();
        assert_eq!(
            diagnostics.initial.issues[0].error_code,
            "UNKNOWN_SOURCE_REF"
        );
        if succeeds {
            assert!(!delivery.incomplete);
            assert_eq!(delivery.compiled.bindings.len(), 1);
            assert_eq!(delivery.compiled.answer, "A supported claim.");
            assert!(diagnostics.repair.as_ref().unwrap().issues.is_empty());
        } else {
            assert!(delivery.incomplete);
            assert_eq!(
                delivery.compiled.answer,
                SOURCE_PRESENTATION_FAILURE_MESSAGE
            );
            assert!(delivery.compiled.bindings.is_empty());
            assert!(delivery.compiled.view.sources.is_empty());
            assert_eq!(
                diagnostics.repair.as_ref().unwrap().issues[0].error_code,
                "UNKNOWN_SOURCE_REF"
            );
        }
    }
}

#[test]
fn source_marker_single_bracket_colon_preserves_existing_literal_exclusions() {
    let bindings = [source_binding_fixture("source_ref_one", "1.1")];
    for answer in [
        "Example `[source:source_ref_one]`",
        "Example `` `[source:source_ref_one].` ``",
        "Example `[source:source_ref_missing]`",
        "原文写着“标记 [source:source_ref_one]。”",
        "```text\n[source:source_ref_one]\n```",
        "~~~text\n[source:source_ref_missing]\n~~~",
        "> quoted [source:source_ref_one]",
        "    [source:source_ref_one]",
        "标记 [source:source_ref_one] 的语法",
        r"A claim.\[source:source_ref_one]",
        "A claim.[source:source_ref_one](https://example.com)",
        "A claim.[source:source_ref_missing](https://example.com)",
    ] {
        let compiled = compile_agent_answer(answer, &bindings, &AnswerProvenanceLedger::default())
            .unwrap_or_else(|error| panic!("literal must remain literal: {answer}: {error:?}"));
        assert_eq!(compiled.answer, answer);
        assert!(compiled.bindings.is_empty(), "{answer}");
        assert!(compiled.view.sources.is_empty(), "{answer}");
    }
}
