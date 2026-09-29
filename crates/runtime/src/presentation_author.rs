//! Resident authoring contract. The host owns files and browser lifetime.
use crate::{
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
        reference: PresentationRef,
        #[serde(default)]
        file: Option<String>,
        #[serde(default)]
        offset: usize,
    },
    Write {
        #[serde(default)]
        based_on: Option<PresentationRef>,
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

#[derive(Debug, Clone, Copy, Deserialize)]
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
    ToolSpec {
        name: "presentation.author".into(),
        description: "Create or revise rich answers or interactive HTML. render_animation runs Manim 0.21.0/Cairo (180s timeout) with class PresentationAnimation(Scene), JSON data, optional size (default 1280x720, even dimensions) and cues [{id,label,at_seconds}]. Inspect returned real frames, then use asset_path in video src, poster_path in poster, and include asset_ref in write.asset_refs. Fixed movies cannot recompute parameters. read returns animation metadata and source/data, never media bytes. render_plot runs Python/Matplotlib for a static SVG and returns its image for inspection plus an asset_ref and asset_path. Use the returned asset_path in an img src and pass asset_ref to write; put chart meaning, units and sources in readable_content. read requires an exact reference and returns saved code or plot files on demand. To revise an undelivered candidate, write a new candidate with the full revised HTML and omit based_on. Use based_on only when revising a delivered presentation reference returned by deliver or read; revisions never replace old answers. state_contract declares compatible scalar page parameters. write requires title, html and readable_content and saves an immutable candidate. Set libraries=[konva] to use bundled Konva 10.7.0; the host saves the library and MIT license and inserts its script before page scripts. Never copy library source or add a CDN. read returns library metadata, never library text. Preserve host-managed script references verbatim when editing read HTML; write removes them and reassembles libraries explicitly requested for this revision, also with based_on. preview executes real actions and returns environment-bound DOM, layout issues and screenshots. For a meaningful intermediate animation frame, expose window.presentationScene with seek({semantic_state,transition_progress}) and snapshot() returning those fields plus playing=false after seek; use a seek preview action to inspect that frozen frame. semantic_state counts completed real steps; transition_progress in [0,1) is visual progress to the next step. For a new candidate, preview exactly these three required viewports: 320x420 touch, 640x240 touch, and 960x720 mouse; fix any failed environment before deliver. The legacy width field remains available for old callers and cannot be combined with viewport. deliver saves a candidate only after the applicable preview contract is complete. Correct errors by writing a new candidate and previewing again. HTML must be self-contained with inline CSS/JS and version assets. No external dependencies. Sources use data-source-ref buttons and refs from source.present, or unchanged refs from the based_on version. Never pass source binding metadata.".into(),
        parameters: json!({"type":"object","properties":{
            "operation":{"type":"string","enum":["render_plot","render_animation","read","write","preview","deliver"]},
            "code":{"type":"string","description":"render_plot: statements using plt/fig/ax/data. render_animation: define PresentationAnimation(Scene), with Manim symbols and JSON data available; host renders Cairo MP4 at 30fps."},
            "data":{"description":"JSON data available as the Python variable data"},
            "size":{"type":"object","properties":{"width":{"type":"integer","minimum":320,"maximum":1600},"height":{"type":"integer","minimum":240,"maximum":1200}},"required":["width","height"],"additionalProperties":false},
            "cues":{"type":"array","maxItems":16,"items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"},"at_seconds":{"type":"number","minimum":0}},"required":["id","label","at_seconds"],"additionalProperties":false}},
            "reference":{"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer"}},"required":["presentation_id","revision"],"additionalProperties":false},
            "based_on":{"type":"object","properties":{"presentation_id":{"type":"string"},"revision":{"type":"integer"}},"required":["presentation_id","revision"],"additionalProperties":false},
            "state_contract":{"type":"object","description":"Parameter name -> semantic definition string, including units and allowed range. Preserve a definition only when old values retain exactly the same meaning and domain. Saved page parameters with identical definitions and JSON types replace matching initial_state fields; all others keep new defaults."},
            "file":{"type":"string","description":"read: logical file name; defaults to entrypoint"},
            "offset":{"type":"integer","minimum":0,"description":"read: character offset; follow next_offset until null to read the complete file"},
            "title":{"type":"string"}, "html":{"type":"string"},
            "readable_content":{"type":"string"},
            "libraries":{"type":"array","items":{"type":"string","enum":["konva"]},"description":"Optional fixed host libraries. Explicitly select dependencies on every write, including based_on revisions; omitted means none."},
            "asset_refs":{"type":"array","items":{"type":"string"},"description":"Refs returned by render_plot/render_animation or listed by read; include each asset used by html."},
            "source_ref_ids":{"type":"array","items":{"type":"string"}},
            "assumptions":{"type":"array","items":{"type":"string"}},
            "initial_state":{}, "candidate_id":{"type":"string"},
            "read_selector":{"type":"string","description":"preview: CSS selector matching one visible result region. After the final action, returns its complete rendered text together with live page state, input values and scene position as reading. Use this page-computed reading for numerical claims; keep each value with its parameters and semantic step. action_step counts preview actions, not mathematical iterations. For another state, run another preview with the required actions. A playing scene must be paused/seeked first. Narrow the selected region if its full context exceeds the result budget."},
            "width":{"type":"integer","minimum":240,"maximum":1920,"description":"preview viewport width in CSS pixels; defaults to 960. Use 340 to inspect narrow layout on this same candidate."},
            "viewport":{"type":"object","description":"Explicit content-container environment. Do not provide width at the same time.","properties":{
                "width":{"type":"integer","minimum":240,"maximum":1920},
                "height":{"type":"integer","minimum":160,"maximum":2160},
                "input":{"type":"string","enum":["mouse","touch"]}
            },"required":["width","height","input"],"additionalProperties":false},
            "actions":{"type":"array","maxItems":4,"items":{"type":"object","properties":{
                "kind":{"type":"string","enum":["click","key","seek"]}, "selector":{"type":"string","description":"Required click target; optional key target to focus before pressing. Without a key target, the current focus receives the key."},
                "key":{"type":"string","enum":["ArrowLeft","ArrowRight","ArrowUp","ArrowDown","Home","End","Enter","Tab"]},
                "semantic_state":{"type":"integer","minimum":0,"maximum":1000,"description":"seek: completed semantic step"},
                "transition_progress":{"type":"number","minimum":0,"exclusiveMaximum":1,"description":"seek: visual progress toward the next step"}
            },"required":["kind"],"additionalProperties":false}}
        },"required":["operation"],"additionalProperties":false}),
    }
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
                    call.arguments = json!({"operation":value["operation"],"title":value["title"],"reference":value["reference"],"based_on":value["based_on"],"libraries":value["libraries"]}).to_string();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tool_exposure::*, ModelRuntimeProfile, ProviderToolProtocol};

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
        assert_eq!(spec().parameters["properties"]["read_selector"]["type"], "string");
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
