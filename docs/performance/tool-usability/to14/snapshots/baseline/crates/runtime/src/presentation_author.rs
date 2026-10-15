//! Resident authoring contract. The host owns files and browser lifetime.
use crate::{
    agent_prompt::presentation::{PresentationGuidance, PresentationNeed, PresentationPhase},
    orchestrator::SourceBinding,
    presentation::PresentationRef,
    presentation_preview::{PreviewAction, PreviewViewport},
    ToolSpec,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationLibrary {
    Konva,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthorRequest {
    Prepare {
        phase: PresentationPhase,
        #[serde(default)]
        framework: Option<String>,
        #[serde(default)]
        focus: Option<String>,
        needs: Vec<PresentationNeed>,
    },
    RenderAnimation {
        code: String,
        #[serde(default)]
        data: Value,
        #[serde(default)]
        size: Option<PlotSize>,
        #[serde(default)]
        cues: Vec<crate::presentation::AnimationCue>,
    },
    RenderPlot {
        code: String,
        #[serde(default)]
        data: Value,
        #[serde(default)]
        size: Option<PlotSize>,
    },
    Read {
        #[serde(default)]
        reference: Option<PresentationRef>,
        #[serde(default)]
        candidate_id: Option<String>,
        #[serde(default)]
        file: Option<String>,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        length: Option<usize>,
    },
    Search {
        #[serde(default)]
        reference: Option<PresentationRef>,
        #[serde(default)]
        candidate_id: Option<String>,
        #[serde(default)]
        file: Option<String>,
        query: String,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        max_matches: Option<usize>,
    },
    Patch {
        #[serde(default)]
        reference: Option<PresentationRef>,
        #[serde(default)]
        candidate_id: Option<String>,
        edits: Vec<PresentationTextEdit>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        readable_content: Option<String>,
        #[serde(default)]
        state_contract: Option<Value>,
        #[serde(default)]
        initial_state: Option<Value>,
    },
    Write {
        #[serde(default)]
        based_on: Option<PresentationRef>,
        #[serde(default)]
        new_object: bool,
        #[serde(default)]
        state_contract: Value,
        title: String,
        html: String,
        readable_content: String,
        #[serde(default)]
        asset_refs: Vec<String>,
        #[serde(default)]
        libraries: Vec<PresentationLibrary>,
        #[serde(default)]
        source_ref_ids: Vec<String>,
        #[serde(default)]
        assumptions: Vec<String>,
        #[serde(default)]
        initial_state: Value,
    },
    Preview {
        candidate_id: String,
        #[serde(default)]
        read_selector: Option<String>,
        #[serde(default)]
        width: Option<u32>,
        #[serde(default)]
        viewport: Option<PreviewViewport>,
        #[serde(default)]
        actions: Vec<PreviewAction>,
    },
    Deliver {
        candidate_id: String,
    },
}

/// A complete replacement of this run's design data. It carries no evidence or
/// candidate/inspection authority, and is never restored from tool history.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PresentationAuthoringContext {
    pub phase: PresentationPhase,
    pub framework: Option<String>,
    pub focus: Option<String>,
    pub needs: Vec<PresentationNeed>,
}

impl PresentationAuthoringContext {
    pub fn guidance(&self) -> PresentationGuidance {
        PresentationGuidance { phase: self.phase, needs: self.needs.clone() }
    }

    pub fn fragment(&self) -> crate::context_fragment::ContextFragment {
        use crate::context_fragment::{ContextFragment, FragmentScope, FragmentSensitivity};
        ContextFragment::new(
            "presentation.authoring_context", FragmentScope::Dynamic, crate::Role::User,
            format!("Current presentation design data (replaces earlier prepare contexts):\n{}",
                serde_json::to_string(self).expect("authoring context is serializable")),
            FragmentSensitivity::Private,
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationTextEdit {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotSize {
    pub width: u32,
    pub height: u32,
}

pub struct AuthorResult {
    pub body: Value,
    pub images: Vec<PreviewImage>,
    pub previewed_candidate: Option<String>,
    pub delivered: Option<PresentationRef>,
}

#[derive(Debug, Clone)]
pub struct PreviewImage {
    pub caption: String,
    pub png_base64: String,
    /// Present only for browser previews; binds the image to delivery inspection.
    pub candidate_id: Option<String>,
    pub environment_name: Option<String>,
}

pub fn unavailable() -> read_tools::ToolError {
    read_tools::ToolError {
        error_code: "PRESENTATION_UNAVAILABLE".into(),
        category: "unavailable".into(),
        message: "This host has no private presentation authoring storage".into(),
    }
}

pub fn spec() -> ToolSpec {
    let source_properties = json!({
            "new_object":{"type":"boolean","description":"write: true explicitly creates a separate presentation, including from a follow-up. Use only when the user wants a new object. Cannot be true together with based_on. Defaults to false; without a follow-up or based_on, ordinary creation is unchanged."},
            "phase":{"type":"string","enum":["global","local","review"],"description":"Required for prepare: current design responsibility."},
            "framework":{"type":"string","description":"prepare: brief current whole-work design, as task data. Omission clears the previous framework."},
            "focus":{"type":"string","description":"prepare: current local purpose and its relation to the whole. Omission clears the previous focus."},
            "needs":{"type":"array","items":{"type":"string","enum":["editing","static_plot","state","continuous_scene","konva","manim"]},"description":"Required for prepare, including []: technical guidance needed for the next sampling."},
            "length":{"type":"integer","minimum":1,"description":"read: maximum characters requested from offset; omitted reads to file end, within model output budget."},
            "query":{"type":"string","minLength":1,"description":"search: literal case-sensitive source text, not a regular expression."},
            "max_matches":{"type":"integer","minimum":1,"maximum":50,"description":"search: default 20. Follow next_offset to find subsequent occurrences."},
            "edits":{"type":"array","minItems":1,"items":{"type":"object","properties":{"old_text":{"type":"string","minLength":1},"new_text":{"type":"string"}},"required":["old_text","new_text"],"additionalProperties":false},"description":"patch: ordered exact replacements in the entrypoint; every old_text must match once. All edits succeed or no candidate is saved. Unchanged bytes and version resources are preserved."}
    });
    let mut tool = ToolSpec {
        name: "presentation.author".into(),
        description: "Create or revise rich answers. prepare replaces the complete run-local phase/framework/focus/needs; phase and needs are required. Omitted framework/focus clear previous values. Call prepare alone; selected guidance applies at the next sampling. It does not create or deliver content. read/search/patch require exactly one delivered reference or current-run candidate_id. read returns a file or offset/length range; search returns literal matches and character offsets. patch applies ordered edits, each old_text matching exactly once, atomically producing a new immutable candidate while preserving other content/resources; optional metadata replaces only supplied fields. write saves a complete candidate and requires title, html and readable_content; based_on selects an exact delivered version, never a candidate. render_plot returns a static SVG asset; render_animation returns a fixed movie asset. Include returned asset_refs and selected libraries on every write. preview executes actions and returns candidate/environment-bound DOM, errors and screenshots; width and viewport are mutually exclusive. deliver saves only a candidate meeting the preview and subsequent-observation contract; changed candidates need new previews.".into(),
        parameters: json!({"type":"object","properties":{
            "operation":{"type":"string","enum":["prepare","render_plot","render_animation","read","search","patch","write","preview","deliver"]},
            "code":{"type":"string","description":"render_plot: statements using plt/fig/ax/data. render_animation: define PresentationAnimation(Scene), with Manim symbols and JSON data available."},
            "data":{"description":"JSON data available as the Python variable data"},
            "size":{"type":"object","properties":{"width":{"type":"integer","minimum":320,"maximum":1600},"height":{"type":"integer","minimum":240,"maximum":1200}},"required":["width","height"],"additionalProperties":false},
            "cues":{"type":"array","maxItems":16,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"at_seconds":{"type":"number","minimum":0}},"required":["id","label","at_seconds"],"additionalProperties":false}},
            "reference":{"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer"}},"required":["presentation_id","revision"],"additionalProperties":false},
            "based_on":{"type":"object","description":"write: exact delivered revision to revise. With a presentation follow-up receipt, must match its reference. Omit only for a new object; a follow-up then requires new_object=true.","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer"}},"required":["presentation_id","revision"],"additionalProperties":false},
            "state_contract":{"type":"object","description":"Parameter name -> semantic definition string, including units and allowed range. Preserve a definition only when old values retain exactly the same meaning and domain. Saved page parameters with identical definitions and JSON types replace matching initial_state fields; all others keep new defaults."},
            "file":{"type":"string","description":"read/search: logical file name; defaults to entrypoint. Use readable_content for the version's explanatory prose with offset/length or query, without HTML. Omit file to read entrypoint source plus full readable_content metadata. Managed libraries return metadata only."},
            "offset":{"type":"integer","minimum":0,"description":"read/search: zero-based Unicode character offset (not bytes or UTF-16). For truncated reads follow next_offset and remaining length until null."},
            "title":{"type":"string"}, "html":{"type":"string"},
            "readable_content":{"type":"string"},
            "libraries":{"type":"array","items":{"type":"string","enum":["konva"]},"description":"Optional fixed host libraries. Explicitly select dependencies on every write, including based_on revisions; omitted means none."},
            "asset_refs":{"type":"array","items":{"type":"string"},"description":"Refs returned by render_plot/render_animation or listed by read; include each asset used by html."},
            "source_ref_ids":{"type":"array","items":{"type":"string"},"description":"write: explicitly list every source ref used in HTML data-source-ref or readable_content [[source:ref]], including refs already observed through source.present or preserved by based_on. Observing a ref does not automatically attach it to the candidate. patch: omission preserves the candidate's sources."},
            "assumptions":{"type":"array","items":{"type":"string"}},
            "initial_state":{}, "candidate_id":{"type":"string"},
            "read_selector":{"type":"string","description":"preview: CSS selector matching one visible result region. After the final action, returns its complete rendered text together with live page state, input values and scene position as reading. Use this page-computed reading for numerical claims; keep each value with its parameters and semantic step. action_step counts preview actions, not mathematical iterations. For another state, run another preview with the required actions. A playing scene must be paused/seeked first. Narrow the selected region if its full context exceeds the result budget."},
            "width":{"type":"integer","minimum":240,"maximum":1920,"description":"Legacy preview width in CSS pixels; defaults to 960. Mutually exclusive with viewport."},
            "viewport":{"type":"object","description":"Explicit content-container environment. Do not provide width at the same time.","properties":{
                "width":{"type":"integer","minimum":240,"maximum":1920},
                "height":{"type":"integer","minimum":160,"maximum":2160},
                "input":{"type":"string","enum":["mouse","touch"]}
            },"required":["width","height","input"],"additionalProperties":false},
            "actions":{"type":"array","maxItems":4,"items":{"type":"object","properties":{
                "kind":{"type":"string","enum":["click","key","seek","scroll"]}, "selector":{"type":"string","description":"Required click target; optional key target to focus before pressing. Without a key target, the current focus receives the key."},
                "y":{"type":"integer","minimum":0,"maximum":4294967295u64,"description":"Required for scroll: absolute document vertical position in CSS pixels, clamped at the page bottom. Changes reading position without focusing or activating controls. Observations report the action, actual scroll position, viewport and visible_text. Each preview starts with a fresh page."},
                "key":{"type":"string","enum":["ArrowLeft","ArrowRight","ArrowUp","ArrowDown","Home","End","Enter","Tab"]},
                "semantic_state":{"type":"integer","minimum":0,"maximum":1000,"description":"seek: completed semantic step"},
                "transition_progress":{"type":"number","minimum":0,"exclusiveMaximum":1,"description":"seek: visual progress toward the next step"}
            },"required":["kind"],"additionalProperties":false}}
        },"required":["operation"],"additionalProperties":false}),
    };
    tool.parameters["properties"].as_object_mut().unwrap().extend(source_properties.as_object().unwrap().clone());
    crate::tool_schema::complete_spec(tool)
}

pub fn bindings_for(
    ids: &[String],
    bindings: &[SourceBinding],
) -> Result<Vec<SourceBinding>, read_tools::ToolError> {
    ids.iter()
        .map(|id| {
            bindings
                .iter()
                .find(|b| &b.source_ref_id == id)
                .cloned()
                .ok_or_else(|| read_tools::ToolError {
                    error_code: "PRESENTATION_SOURCE_UNKNOWN".into(),
                    category: "validation".into(),
                    message: format!("Use a source.present ref observed in this run: {id}"),
                })
        })
        .collect()
}

/// Preserve identities and receipts, not generated source code, in future conversation context.
pub fn redact_history(messages: &mut [crate::Message]) {
    for message in messages {
        for call in &mut message.tool_calls {
            if call.name == "presentation.author" {
                if let Ok(value) = serde_json::from_str::<Value>(&call.arguments) {
                    let mut locator = json!({"operation":value["operation"],"title":value["title"],"reference":value["reference"],"based_on":value["based_on"],"libraries":value["libraries"]});
                    // Do not rewrite older persisted calls merely to add a new
                    // optional field: checkpoints identify their exact bytes.
                    if let Some(choice) = value.get("new_object") {
                        locator["new_object"] = choice.clone();
                    }
                    call.arguments = locator.to_string();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ex13_redaction_preserves_pre_new_object_history_bytes() {
        let arguments = serde_json::json!({"operation":"write","title":"existing","reference":null,
            "based_on":null,"libraries":null}).to_string();
        let mut message = crate::Message::user("existing history");
        message.tool_calls.push(crate::ToolCall {id:"saved".into(),name:"presentation.author".into(),arguments:arguments.clone()});
        super::redact_history(std::slice::from_mut(&mut message));
        assert_eq!(message.tool_calls[0].arguments, arguments, "adding an absent field invalidates persisted checkpoint source IDs");
    }

    #[test]
    fn ex13_write_new_object_is_explicit_and_preserved_in_history() {
        let base = serde_json::json!({"operation":"write","title":"t","html":"<p>t</p>","readable_content":"t"});
        assert!(matches!(serde_json::from_value::<super::AuthorRequest>(base.clone()).unwrap(), super::AuthorRequest::Write { new_object:false, .. }));
        let mut args = base;
        args["new_object"] = serde_json::json!(true);
        assert!(matches!(serde_json::from_value::<super::AuthorRequest>(args.clone()).unwrap(), super::AuthorRequest::Write { new_object:true, .. }));
        let registry = crate::orchestrator::resident_tool_registry();
        registry.registration("presentation.author").unwrap().validate_arguments(&args.to_string()).unwrap();
        assert_eq!(super::spec().parameters["properties"]["new_object"]["type"], "boolean");
        let mut message = crate::Message::user("new example");
        message.tool_calls.push(crate::ToolCall { id:"new".into(), name:"presentation.author".into(), arguments:args.to_string() });
        super::redact_history(std::slice::from_mut(&mut message));
        let historical: serde_json::Value = serde_json::from_str(&message.tool_calls[0].arguments).unwrap();
        assert_eq!(historical["new_object"], true);
        assert!(historical.get("html").is_none());
        args["new_object"] = serde_json::json!("true");
        assert!(serde_json::from_value::<super::AuthorRequest>(args).is_err());
    }

    #[test]
    fn ex12_scroll_schema_and_request_use_absolute_css_pixels() {
        let request: super::AuthorRequest = serde_json::from_str(r#"{"operation":"preview","candidate_id":"c","actions":[{"kind":"scroll","y":1200}]}"#).unwrap();
        assert!(matches!(request, super::AuthorRequest::Preview { actions, .. } if matches!(actions.as_slice(), [crate::presentation_preview::PreviewAction::Scroll { y:1200 }])));
        for action in [r#"{"kind":"scroll"}"#, r#"{"kind":"scroll","y":-1}"#, r#"{"kind":"scroll","y":1.5}"#] {
            assert!(serde_json::from_str::<crate::presentation_preview::PreviewAction>(action).is_err());
        }
        let schema = super::spec().parameters["properties"]["actions"].clone();
        assert_eq!(schema["maxItems"], 4);
        assert!(schema["items"]["properties"]["kind"]["enum"].as_array().unwrap().contains(&serde_json::json!("scroll")));
        assert_eq!(schema["items"]["properties"]["y"]["minimum"], 0);
    }
    use super::*;
    use crate::{tool_exposure::*, ModelRuntimeProfile, ProviderToolProtocol};

    #[test]
    fn ex12_prepare_contract_requires_explicit_phase_and_needs() {
        let args = json!({"operation":"prepare","phase":"local","needs":["editing","static_plot","state","continuous_scene","konva","manim"]});
        assert!(matches!(serde_json::from_str::<AuthorRequest>(&args.to_string()).unwrap(), AuthorRequest::Prepare { framework:None, focus:None, needs,.. } if needs.len() == 6));
        let registry = crate::orchestrator::resident_tool_registry();
        let registration = registry.registration("presentation.author").unwrap();
        registration.validate_arguments(&args.to_string()).unwrap();
        for invalid in [
            json!({"operation":"prepare","needs":[]}),
            json!({"operation":"prepare","phase":"local"}),
            json!({"operation":"prepare","phase":"publish","needs":[]}),
            json!({"operation":"prepare","phase":"local","needs":["invented"]}),
            json!({"operation":"prepare","phase":"local","needs":[],"candidate_id":"old"}),
        ] { assert!(serde_json::from_str::<AuthorRequest>(&invalid.to_string()).is_err(), "{invalid}"); }
        let mut message = crate::Message::user("task");
        message.tool_calls.push(crate::ToolCall { id:"p".into(), name:"presentation.author".into(), arguments:json!({"operation":"prepare","phase":"local","framework":"PRIVATE_FRAMEWORK","focus":"PRIVATE_FOCUS","needs":[]}).to_string() });
        super::redact_history(std::slice::from_mut(&mut message));
        assert!(!message.tool_calls[0].arguments.contains("PRIVATE_"));
    }

    #[test]
    fn animation_schema_and_history_keep_only_handles() {
        let args=json!({"operation":"render_animation","code":"class PresentationAnimation(Scene): pass","data":{"distance":6},"cues":[{"id":"middle","label":"Midpoint","at_seconds":1.0}]});
        assert!(matches!(serde_json::from_value::<AuthorRequest>(args.clone()).unwrap(),AuthorRequest::RenderAnimation {size:None,cues,..} if cues.len()==1));
        // Provider calls arrive as JSON text. Internally tagged serde enums must
        // also accept fractional cue times with workspace arbitrary_precision.
        for seconds in [0.0, 0.6, 1.1, 4.3, 11.5] {
            let mut wire = args.clone();
            wire["cues"][0]["at_seconds"] = json!(seconds);
            let AuthorRequest::RenderAnimation { cues, .. } = serde_json::from_str(&wire.to_string()).unwrap() else { unreachable!() };
            assert_eq!(cues[0].at_seconds, seconds);
        }
        for invalid in [json!("1.1"), json!({"time":1.1}), json!(null)] {
            let mut wire = args.clone();
            wire["cues"][0]["at_seconds"] = invalid;
            assert!(serde_json::from_str::<AuthorRequest>(&wire.to_string()).is_err());
        }
        assert!(spec().parameters["properties"]["operation"]["enum"].as_array().unwrap().contains(&json!("render_animation")));
        let mut message=crate::Message::user("motion");
        message.tool_calls.push(crate::ToolCall{id:"c".into(),name:"presentation.author".into(),arguments:args.to_string()});
        super::redact_history(std::slice::from_mut(&mut message));
        assert!(!message.tool_calls[0].arguments.contains("PresentationAnimation"));
    }

    #[test]
    fn ex11_library_schema_and_default() {
        let base = json!({"operation":"write","title":"t","html":"<p>t</p>","readable_content":"t"});
        assert!(matches!(serde_json::from_value::<AuthorRequest>(base.clone()).unwrap(), AuthorRequest::Write {libraries,..} if libraries.is_empty()));
        let mut named = base;
        named["libraries"] = json!(["konva"]);
        assert!(matches!(serde_json::from_value::<AuthorRequest>(named.clone()).unwrap(), AuthorRequest::Write {libraries,..} if libraries == vec![PresentationLibrary::Konva]));
        named["libraries"] = json!(["other"]);
        assert!(serde_json::from_value::<AuthorRequest>(named).is_err());
        assert_eq!(spec().parameters["properties"]["libraries"]["items"]["enum"], json!(["konva"]));
    }

    #[test]
    fn selected_result_preview_accepts_selector() {
        let request = r##"{"operation":"preview","candidate_id":"c","read_selector":"#result"}"##;
        assert!(serde_json::from_str::<AuthorRequest>(request).is_ok());
        assert_eq!(spec().parameters["properties"]["read_selector"]["type"], json!(["string", "null"]));
    }

    #[test]
    fn presentation_edit_contract_exposes_search_ranges_and_exact_patch() {
        let properties = spec().parameters["properties"].clone();
        for operation in ["read", "search", "patch"] {
            assert!(properties["operation"]["enum"].as_array().unwrap().contains(&json!(operation)));
        }
        for wire in [
            json!({"operation":"read","candidate_id":"c","offset":10,"length":200}),
            json!({"operation":"search","reference":{"presentation_id":"p","revision":1},"query":"function seek"}),
            json!({"operation":"patch","candidate_id":"c","edits":[{"old_text":"old","new_text":"new"}]}),
        ] { assert!(serde_json::from_str::<AuthorRequest>(&wire.to_string()).is_ok()); }
        assert!(serde_json::from_value::<AuthorRequest>(json!({"operation":"patch","candidate_id":"c","edits":[{"old_text":"old","new_text":"new","fuzzy":true}]})).is_err());
        assert!(spec().description.contains("exactly once"));
    }

    #[test]
    fn presentation_history_keeps_versions_without_stale_candidate_handles() {
        let reference = json!({"presentation_id":"p","revision":2});
        let mut message = crate::Message::user("edit");
        message.tool_calls.push(crate::ToolCall { id:"call".into(), name:"presentation.author".into(), arguments:json!({"operation":"write","candidate_id":"old-candidate","based_on":reference,"html":"old code"}).to_string() });
        super::redact_history(std::slice::from_mut(&mut message));
        let args: Value = serde_json::from_str(&message.tool_calls[0].arguments).unwrap();
        assert_eq!(args["based_on"], reference);
        assert!(args.get("candidate_id").is_none());
        assert!(args.get("html").is_none());
    }

    #[test]
    fn presentation_author_discovery_is_deferred_and_operation_scoped() {
        let registry = crate::orchestrator::resident_tool_registry();
        let profile = ModelRuntimeProfile::fallback("test", ProviderToolProtocol::Native);
        let context = ToolExposureContext {
            content_profile: read_tools::ContentProfileId::TechnicalLearning,
            permissions: ToolPermissions::default(),
            evidence_state: EvidenceState::Unlocated,
            artifact: ArtifactExposureContext::no_overlay(),
        };
        let mut state = ToolExposureState::default();
        let before = ToolExposurePlan::build(&registry, &profile, &context, &state);
        assert!(!before.is_visible("presentation.author"));
        let result = search_and_activate(&json!({"task":"interactive HTML explanation", "required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}).to_string(),&context,&before,&registry,&mut state).unwrap();
        assert_eq!(result.activated, vec!["presentation.author"]);
        assert!(
            ToolExposurePlan::build(&registry, &profile, &context, &state)
                .is_visible("presentation.author")
        );
    }

    #[test]
    fn presentation_images_reach_native_and_react_without_entering_history() {
        let profile = ModelRuntimeProfile::fallback("test", ProviderToolProtocol::Native);
        let mut plan = crate::AgentRequestPlan::for_agent_turn(
            profile,
            &[crate::Message::user("explain")],
            &[],
        );
        plan.preview_images.push(PreviewImage {
            caption: "candidate c1 narrow-content step 0 preview_failed".into(),
            png_base64: "test-png".into(),
            candidate_id: Some("c1".into()),
            environment_name: Some("narrow-content".into()),
        });
        plan.preview_images.push(PreviewImage {
            caption: "candidate c1 desktop-content step 1 preview_environment_recorded".into(),
            png_base64: "second-png".into(),
            candidate_id: Some("c1".into()),
            environment_name: Some("desktop-content".into()),
        });
        for body in [
            crate::native_chat_request_projection("test", &plan).0,
            crate::react_chat_request_projection("test", &plan),
        ] {
            let last = body["messages"].as_array().unwrap().last().unwrap();
            assert_eq!(
                last["content"][0]["text"],
                "candidate c1 narrow-content step 0 preview_failed"
            );
            assert_eq!(
                last["content"][1]["image_url"]["url"],
                "data:image/png;base64,test-png"
            );
            assert_eq!(
                last["content"][2]["text"],
                "candidate c1 desktop-content step 1 preview_environment_recorded"
            );
            assert_eq!(
                last["content"][3]["image_url"]["url"],
                "data:image/png;base64,second-png"
            );
        }
        assert!(!serde_json::to_string(&plan.ordered_messages())
            .unwrap()
            .contains("test-png"));
        assert!(!serde_json::to_string(&plan.ordered_messages())
            .unwrap()
            .contains("second-png"));
    }

    #[test]
    fn presentation_author_schema_keeps_legacy_width_and_describes_explicit_viewport() {
        let parameters = spec().parameters;
        let properties = parameters["properties"].as_object().unwrap();
        assert_eq!(properties["width"]["minimum"], 240);
        assert_eq!(properties["width"]["maximum"], 1920);
        assert_eq!(
            properties["viewport"]["properties"]["height"]["minimum"],
            160
        );
        assert_eq!(
            properties["viewport"]["properties"]["input"]["enum"],
            json!(["mouse", "touch"])
        );
        assert_eq!(
            properties["viewport"]["required"],
            json!(["width", "height", "input"])
        );
        assert!(properties["operation"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("render_plot")));
        assert_eq!(properties["size"]["properties"]["width"]["maximum"], 1600);
        assert_eq!(properties["asset_refs"]["type"], "array");
    }
}
