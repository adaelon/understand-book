//! Immutable compiled method assets, shared by the prompt directory and Author.
use serde_json::{json, Value};
use std::sync::OnceLock;
use crate::presentation::{PresentationMethodObservations, PresentationMethodRead};

include!(concat!(env!("OUT_DIR"), "/presentation_methods.rs"));

pub fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(CATALOG_JSON).expect("compiled method catalog"))
}

pub fn directory() -> crate::InstructionModule {
    crate::InstructionModule::new(
        "resident-agent.presentation.methods",
        format!("se3.v1.catalog.{}", catalog()["revision"]),
        format!("Presentation method directory:\nChoose zero or more methods by the current understanding obstacle and boundary. Technical needs are selected separately with prepare. Read a listed exact id/revision using presentation.author read_method; omitted file reads card.md. Read its example source and adaptation guide before adapting it. File offsets and lengths count Unicode characters. For read_method, length is a page size and end_offset is the actual returned page end. Follow the returned next_offset with the same page size through the whole file until null (EOF), including after output-budget truncation; do not guess or skip offsets. Declare actual uses with write/patch method_refs (method_id, revision, optional scope). Observe a read in a later sampling before declaring a new method. patch omission inherits; a supplied list replaces all references, including [] to clear. Full write never implicitly inherits: explicitly redeclare when rewriting a version with recorded methods. read returns the version's recorded references; null means historical use was not recorded. No method grants book-source, preview or delivery authority.\n{}", catalog()),
    )
}

pub fn source(
    method_id: &str,
    revision: u64,
    file: &str,
) -> Result<(&'static Value, &'static str), read_tools::ToolError> {
    let error = |code: &str, message: String| read_tools::ToolError {
        error_code: code.into(),
        category: "validation".into(),
        message,
    };
    let method = catalog()["methods"].as_array().unwrap().iter()
        .find(|method| method["id"] == method_id && method["revision"] == revision)
        .ok_or_else(|| error("PRESENTATION_METHOD_NOT_FOUND", format!("Method {method_id}@{revision} is not in this build's directory; use a listed exact id/revision")))?;
    let source = FILES
        .iter()
        .find(|(id, rev, name, _)| *id == method_id && *rev == revision && *name == file)
        .map(|(_, _, _, text)| *text)
        .ok_or_else(|| {
            error(
                "PRESENTATION_METHOD_FILE_NOT_FOUND",
                format!(
                    "File {file} is not registered for {method_id}@{revision}; files: {}",
                    method["files"]
                ),
            )
        })?;
    Ok((method, source))
}

pub fn read_metadata(method: &Value, file: &str) -> Value {
    json!({"status":"method_read", "catalog_revision":catalog()["revision"],
        "method_id":method["id"], "revision":method["revision"], "file":file,
        "entrypoint":method["entrypoint"], "files":method["files"]})
}

impl PresentationMethodObservations {
    pub fn merge(&mut self, other: &Self) {
        if other.catalog.is_some() { self.catalog = other.catalog.clone(); }
        for read in &other.reads {
            if !self.reads.contains(read) { self.reads.push(read.clone()); }
        }
    }

    pub fn is_empty(&self) -> bool { self.catalog.is_none() && self.reads.is_empty() }

    /// Only current-run tool results that survived request projection count as reads.
    pub fn newly_sampled(
        &self,
        plan: &crate::AgentRequestPlan,
        current_reads: &std::collections::HashSet<String>,
    ) -> Self {
        let mut observed = Self::default();
        if self.catalog.is_none() && plan.instruction_assets.iter().any(|asset| asset.asset_id == "resident-agent.presentation.methods") {
            observed.catalog = Some(catalog().clone());
        }
        for message in plan.ordered_messages() {
            if message.role != crate::Role::Tool { continue; }
            let Some(id) = message.tool_call_id.filter(|id| current_reads.contains(id)) else { continue; };
            let Some(value) = message.content.and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { continue; };
            let body = &value["model_body"];
            if body["status"] != "method_read" { continue; }
            let (Some(method_id), Some(revision), Some(file), Some(text), Some(offset), Some(end), Some(total)) = (
                body["method_id"].as_str(), body["revision"].as_u64(), body["file"].as_str(), body["text"].as_str(),
                body["offset"].as_u64(), body["end_offset"].as_u64(), body["total_characters"].as_u64(),
            ) else { continue; };
            if text.is_empty() { continue; }
            let read = PresentationMethodRead { tool_call_id: id, method_id: method_id.into(), revision,
                file: file.into(), offset: offset as usize, characters: text.chars().count(),
                end_offset: end as usize, total_characters: total as usize };
            if !self.reads.contains(&read) && !observed.reads.contains(&read) { observed.reads.push(read); }
        }
        observed
    }
}

impl PresentationMethodObservations {
    pub(crate) fn record_sample(&mut self, current_reads: &std::collections::HashSet<String>, events: &crate::run_events::RunEvents, state: &mut impl crate::run_context::ResidentStatePort, plan: &crate::AgentRequestPlan) -> Result<(), read_tools::ToolError> {
        let observed = self.newly_sampled(plan, current_reads);
        if observed.is_empty() { return Ok(()); }
        state.record_presentation_methods(&observed)?;
        if let Some(catalog) = &observed.catalog {
            let activity = events.begin("presentation", "presentation.methods.catalog", "演示方法目录已提供", false);
            events.output(activity.step_id, || catalog.clone());
            events.finish(activity, crate::run_events::ActivityStatus::Succeeded, None, None,
                Some(catalog["methods"].as_array().map_or(0, |m| m.len()) as u32));
        }
        for read in &observed.reads {
            let activity = events.begin("presentation", "presentation.methods.read", "演示方法正文已提供", false);
            events.output(activity.step_id, || json!(read));
            events.finish(activity, crate::run_events::ActivityStatus::Succeeded, None, None, Some(read.characters as u32));
        }
        self.merge(&observed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se3_observations_use_only_current_projected_source_and_deduplicate() {
        let current = std::collections::HashSet::from(["read-now".into()]);
        let mut message = crate::Message::user("unused");
        message.role = crate::Role::Tool;
        message.tool_call_id = Some("read-now".into());
        message.content = Some(json!({"status":"partial","model_body":{
            "status":"method_read","method_id":"discrete-update","revision":1,
            "file":"example/index.html","offset":80,"text":"实际片段",
            "end_offset":84,"total_characters":2000,"next_offset":84
        }}).to_string());
        let mut plan = crate::AgentRequestPlan::for_agent_turn(
            crate::ModelRuntimeProfile::fallback("test", crate::ProviderToolProtocol::Native), &[message.clone()], &[]);
        plan.instruction_assets.push(crate::InstructionAssetRef { asset_id:"resident-agent.presentation.methods".into(), revision:"se3".into() });
        let mut observed = PresentationMethodObservations::default();
        let delta = observed.newly_sampled(&plan, &current);
        assert_eq!(delta.catalog.as_ref(), Some(catalog()));
        assert_eq!(delta.reads.len(), 1);
        assert_eq!((delta.reads[0].offset, delta.reads[0].characters, delta.reads[0].end_offset), (80,4,84));
        observed.merge(&delta);
        assert!(observed.newly_sampled(&plan, &current).is_empty());
        assert!(PresentationMethodObservations::default().newly_sampled(&plan, &Default::default()).reads.is_empty(), "historical reads are not current-run evidence");
        message.content = Some(json!({"status":"partial","model_body":{"error_code":"PRESENTATION_SOURCE_BUDGET"}}).to_string());
        let plan = crate::AgentRequestPlan::for_agent_turn(
            crate::ModelRuntimeProfile::fallback("test", crate::ProviderToolProtocol::Native), &[message], &[]);
        assert!(observed.newly_sampled(&plan, &current).is_empty(), "a budget receipt supplies no source");
    }

    #[test]
    fn se2_compiled_catalog_and_sources_match_registered_assets() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = std::env::var_os("UNDERSTAND_BOOK_METHOD_CATALOG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "skills/presentation/methods/catalog.json".into());
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(repo.join(manifest)).unwrap()).unwrap();
        assert_eq!(
            catalog(),
            &expected,
            "compiled combination must match the selected build input"
        );
        let methods = catalog()["methods"].as_array().unwrap();
        eprintln!("SE2 compiled methods: {}", methods.len());
        for method in methods {
            for file in method["files"].as_array().unwrap() {
                let name = file.as_str().unwrap();
                let (registered, text) = source(
                    method["id"].as_str().unwrap(),
                    method["revision"].as_u64().unwrap(),
                    name,
                )
                .unwrap();
                assert_eq!(registered, method);
                assert_eq!(
                    text,
                    std::fs::read_to_string(repo.join(method["root"].as_str().unwrap()).join(name))
                        .unwrap()
                );
            }
        }
        assert_eq!(
            FILES.len(),
            methods
                .iter()
                .map(|m| m["files"].as_array().unwrap().len())
                .sum::<usize>()
        );
        assert!(directory().text.ends_with(&catalog().to_string()));
        assert_eq!(
            source("absent", 1, "card.md").unwrap_err().error_code,
            "PRESENTATION_METHOD_NOT_FOUND"
        );
        for method in methods {
            let id = method["id"].as_str().unwrap();
            let revision = method["revision"].as_u64().unwrap();
            assert_eq!(
                source(id, revision + 100, "card.md")
                    .unwrap_err()
                    .error_code,
                "PRESENTATION_METHOD_NOT_FOUND"
            );
            assert_eq!(
                source(id, revision, "absent.md").unwrap_err().error_code,
                "PRESENTATION_METHOD_FILE_NOT_FOUND"
            );
        }
    }
}
