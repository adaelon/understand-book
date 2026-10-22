//! Deliberate public work updates, separate from answer drafts and evidence.
use crate::orchestrator::{compile_answer_preview, AgentAnswerPart, AnswerProvenanceLedger};
use crate::ToolSpec;
use serde::Deserialize;
use serde_json::json;

pub(crate) const POLICY: &str = "Public work updates:
- At the start of substantial work and after meaningful progress or a change of direction, call agent.progress with a brief user-facing update in the user's language. Explain what you will do next, what actually completed, or why the approach changed in one to three sentences.
- Include it in the same tool_calls batch as the actual work. Do not request a separate model turn just to report progress, narrate every tool, repeat generic waiting messages, or follow a fixed reporting interval. A short direct answer needs no update.
- These updates are immediately public. Do not include private reasoning, tool arguments, internal IDs or LIDs, source/presentation markers, teaching content, or an unreviewed answer. Report observed work without claiming success before its result. Progress reporting is not evidence, a deliverable, or substantive task progress. Ordinary answers still follow all source compilation and Tutor review requirements.";

pub(crate) fn spec() -> ToolSpec {
    ToolSpec {
        name: "agent.progress".into(),
        description: "Share one to three brief user-facing sentences about the next action, actual completed work, or a changed approach. Call alongside actual work in the same batch. No private reasoning, arguments, internal IDs, source markers, teaching content or unreviewed answers. This update supplies no evidence or delivery.".into(),
        parameters: json!({"type":"object","properties":{
            "message":{"type":"string","minLength":1,"description":"Brief public progress update in the user's language."}
        },"required":["message"],"additionalProperties":false}),
    }
}

pub(crate) fn public_message(
    arguments: &str,
    provenance: &AnswerProvenanceLedger,
) -> Result<String, &'static str> {
    #[derive(Deserialize)]
    struct Input {
        message: String,
    }
    let input: Input = serde_json::from_str(arguments)
        .map_err(|_| "agent.progress requires a non-empty message")?;
    let message = input.message.trim();
    if message.is_empty() {
        return Err("agent.progress requires a non-empty message");
    }
    let view = compile_answer_preview(message, &[], provenance)
        .filter(|view| view.sources.is_empty()
            && !view.parts.is_empty()
            && view.parts.iter().all(|part| matches!(part, AgentAnswerPart::Markdown { .. })))
        .ok_or("Progress updates must be public text without internal locators, source references or presentation markers")?;
    Ok(view
        .parts
        .into_iter()
        .filter_map(|part| match part {
            AgentAnswerPart::Markdown { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_progress_requires_public_nonempty_text() {
        let provenance = AnswerProvenanceLedger::default();
        assert_eq!(
            public_message(
                r#"{"message":"  先查看这一节的结构，再读取关键段落。  "}"#,
                &provenance
            )
            .unwrap(),
            "先查看这一节的结构，再读取关键段落。"
        );
        for message in [
            "",
            " \n ",
            "查看这里。[[source:missing]]",
            "[source:missing]",
        ] {
            assert!(
                public_message(&json!({"message":message}).to_string(), &provenance).is_err(),
                "{message:?}"
            );
        }
        assert!(public_message(r#"{"message":1}"#, &provenance).is_err());
    }

    #[test]
    fn agent_progress_is_available_to_native_and_react_with_public_update_policy() {
        use crate::tool_exposure::{
            ArtifactExposureContext, ToolExposureContext, ToolExposurePlan, ToolExposureState,
            ToolPermissions,
        };
        let registry = crate::orchestrator::resident_tool_registry();
        for protocol in [
            crate::ProviderToolProtocol::Native,
            crate::ProviderToolProtocol::ReAct,
        ] {
            let profile = crate::ModelRuntimeProfile::fallback("test", protocol);
            let exposure = ToolExposurePlan::build(
                &registry,
                &profile,
                &ToolExposureContext {
                    content_profile: read_tools::ContentProfileId::TechnicalLearning,
                    permissions: ToolPermissions::default(),
                    evidence_state: crate::tool_exposure::EvidenceState::Unlocated,
                    artifact: ArtifactExposureContext::no_overlay(),
                },
                &ToolExposureState::default(),
            );
            assert!(exposure.is_visible("agent.progress"));
            let modules = crate::agent_prompt::policy_modules_for_tools(&exposure.visible_tools);
            assert!(modules
                .iter()
                .any(|module| module.asset_id == "resident-agent.policy.progress"));
            let plan = crate::AgentRequestPlan::for_agent_turn(
                profile,
                &[crate::Message::user("Explain this")],
                &exposure.visible_tools,
            );
            let (native, names) = crate::native_chat_request_projection("test", &plan);
            assert!(names.values().any(|name| name == "agent.progress"));
            assert!(native["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["function"]["parameters"] == spec().parameters));
            let react = crate::react_chat_request_projection("test", &plan);
            assert!(react["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("agent.progress"));
        }
    }
}
