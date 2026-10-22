//! Live operations on an already delivered, open content version.
use crate::{presentation::PresentationRef, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum PresentationActionContract {
    SetParameters { parameters: Vec<String> },
    Invoke { action_id: String, label: String },
    Highlight { region_id: String },
    Reveal { region_id: String },
    Annotate { region_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum PresentationAction {
    SetParameters {
        #[ts(type = "Record<string, unknown>")]
        values: BTreeMap<String, Value>,
    },
    Invoke { action_id: String },
    Highlight { region_id: String },
    Reveal { region_id: String },
    Annotate { region_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum InteractRequest {
    Observe { reference: PresentationRef },
    Act { reference: PresentationRef, based_on: String, action: PresentationAction },
}
impl InteractRequest {
    pub fn reference(&self) -> &PresentationRef {
        match self { Self::Observe { reference } | Self::Act { reference, .. } => reference }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationCommand {
    pub command_id: String,
    pub run_turn_id: String,
    pub session_id: String,
    pub delivery_turn_id: String,
    pub request: InteractRequest,
    pub page_id: Option<String>,
    #[ts(type = "Record<string, unknown>")]
    pub expected_values: BTreeMap<String, Value>,
    #[ts(type = "Record<string, number>")]
    pub expected_edits: BTreeMap<String, u64>,
    pub available_actions: Vec<PresentationActionContract>,
}

pub fn error(code: &str, message: impl Into<String>) -> read_tools::ToolError {
    read_tools::ToolError { error_code: code.into(), category: "execution".into(), message: message.into() }
}
pub fn unavailable() -> read_tools::ToolError {
    error("PRESENTATION_INTERACTION_UNAVAILABLE", "Open this delivered presentation in the current Reader before interacting.")
}

pub fn parameter_names(contract: &[PresentationActionContract]) -> Vec<String> {
    contract.iter().flat_map(|entry| match entry {
        PresentationActionContract::SetParameters { parameters } => parameters.clone(),
        _ => vec![],
    }).collect()
}
pub fn validate_action(action: &PresentationAction, contract: &[PresentationActionContract]) -> Result<(), read_tools::ToolError> {
    use PresentationAction as A;
    use PresentationActionContract as C;
    let declared = match action {
        A::SetParameters { values } => !values.is_empty() && values.keys().all(|key| parameter_names(contract).contains(key))
            && values.values().all(|v| v.is_string() || v.is_boolean() || v.is_number() || v.is_null()),
        A::Invoke { action_id } => contract.iter().any(|c| matches!(c, C::Invoke { action_id: id, .. } if id == action_id)),
        A::Highlight { region_id } => contract.iter().any(|c| matches!(c, C::Highlight { region_id: id } if id == region_id)),
        A::Reveal { region_id } => contract.iter().any(|c| matches!(c, C::Reveal { region_id: id } if id == region_id)),
        A::Annotate { region_id } => contract.iter().any(|c| matches!(c, C::Annotate { region_id: id } if id == region_id)),
    };
    if declared { Ok(()) } else { Err(error("PRESENTATION_ACTION_UNDECLARED", "Choose an action and parameters from available_actions; new content requires presentation.author.")) }
}

pub fn spec() -> ToolSpec {
    let reference = json!({"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer","minimum":1}},"required":["presentation_id","revision"],"additionalProperties":false});
    let action = book_tool_contracts::schema_value::<PresentationAction>();
    crate::tool_schema::complete_spec(ToolSpec {
        name: "presentation.interact".into(),
        description: "Observe or operate the exact delivered presentation currently open in the Reader. Observe first; act uses its saved_state_ref and declared actions. Only applied with a saved receipt confirms completion. Chain a local demonstration from each actual receipt, leaving the same page available for learner edits. teaching.action_ref records teacher usage and teaching.assistance_refs identify the visible help saved with that scene; these are not learner answers or independent mastery. On conflict, cancellation or timeout observe again; never automatically retry an action. Parameters change runtime state, not the content version.".into(),
        parameters: json!({"type":"object","properties":{
            "operation":{"type":"string","enum":["observe","act"]},"reference":reference,
            "based_on":{"type":"string"},"action":action
        },"required":["operation","reference"],"additionalProperties":false,
        "anyOf":[
            {"properties":{"operation":{"enum":["observe"]}},"not":{"required":["action"]}},
            {"properties":{"operation":{"enum":["act"]}},"required":["based_on","action"]}
        ],
        "examples":[
            {"operation":"observe","reference":{"presentation_id":"PRESENTATION_ID","revision":1}},
            {"operation":"act","reference":{"presentation_id":"PRESENTATION_ID","revision":1},"based_on":"SAVED_STATE_REF","action":{"kind":"set_parameters","values":{"input_speed_b_m_s":2}}},
            {"operation":"act","reference":{"presentation_id":"PRESENTATION_ID","revision":1},"based_on":"SAVED_STATE_REF","action":{"kind":"invoke","action_id":"apply"}}
        ]}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn om4_contract_requires_an_observation_and_declared_actions_without_new_annotation_text() {
        let registry = crate::orchestrator::resident_tool_registry();
        let tool = registry.registration("presentation.interact").unwrap();
        for example in tool.spec.parameters["examples"].as_array().unwrap() {
            tool.validate_arguments(&example.to_string()).unwrap();
            serde_json::from_value::<InteractRequest>(example.clone()).unwrap();
        }
        for input in [
            json!({"operation":"act","reference":{"presentation_id":"p","revision":1},"action":{"kind":"invoke","action_id":"apply"}}),
            json!({"operation":"observe","reference":{"presentation_id":"p","revision":1},"based_on":"old"}),
            json!({"operation":"act","reference":{"presentation_id":"p","revision":1},"based_on":"saved","action":{"kind":"annotate","region_id":"r","text":"new unreviewed explanation"}}),
        ] {
            assert!(tool.validate_arguments(&input.to_string()).is_err());
            assert!(serde_json::from_value::<InteractRequest>(input).is_err());
        }
        let contract = vec![PresentationActionContract::SetParameters { parameters:vec!["speed".into()] }];
        assert!(validate_action(&PresentationAction::SetParameters { values:BTreeMap::from([("unknown".into(),json!(2))]) }, &contract).is_err());
    }
}
