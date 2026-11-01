//! Tool definitions retain their first declaration and activation position.
//! Execution authority remains the current ToolExposurePlan, never this history.
use crate::{deepseek_messages::{ToolDeclaration, ToolHistoryProjection, ToolUpdate, ToolUpdateMode}, AgentRequestPlan, Message, ToolSpec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolDeclarationHistory {
    mode: Option<ToolUpdateMode>,
    model: String,
    instructions: String,
    active_tools: Vec<ToolSpec>,
    projection: Option<ToolHistoryProjection>,
    // The active model projection persists these messages once. Restore from it.
    #[serde(skip)]
    previous_input: Vec<Message>,
}

impl ToolDeclarationHistory {
    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn resume_input(&mut self, input: &[Message]) {
        self.previous_input = input.to_vec();
    }

    pub fn project(&mut self, plan: &mut AgentRequestPlan, mode: Option<ToolUpdateMode>) {
        let Some(mode) = mode else {
            self.reset();
            plan.tool_history = None;
            return;
        };
        let changed_definition = self.projection.as_ref().is_some_and(|history|
            plan.tools.iter().any(|tool| history.declarations.iter()
                .any(|old| old.spec.name == tool.name && old.spec != *tool)));
        let rebuild = self.mode != Some(mode)
            || self.model != plan.runtime_profile.matched_model
            || self.instructions != plan.instructions
            || !plan.input.starts_with(&self.previous_input)
            || changed_definition || self.projection.is_none();
        if rebuild {
            self.projection = Some(ToolHistoryProjection {
                declarations: plan.tools.iter().cloned().map(|spec| ToolDeclaration { spec, defer_loading: false }).collect(),
                updates: Vec::new(),
            });
        } else {
            let history = self.projection.as_mut().expect("existing declaration series");
            let added: Vec<_> = plan.tools.iter().filter(|tool|
                !self.active_tools.iter().any(|old| old.name == tool.name))
                .map(|tool| tool.name.clone()).collect();
            let removed: Vec<_> = self.active_tools.iter().filter(|tool|
                !plan.tools.iter().any(|current| current.name == tool.name))
                .map(|tool| tool.name.clone()).collect();
            for name in &added {
                if !history.declarations.iter().any(|declaration| declaration.spec.name == *name) {
                    let spec = plan.tools.iter().find(|tool| tool.name == *name).unwrap().clone();
                    history.declarations.push(ToolDeclaration { spec, defer_loading: true });
                }
            }
            let retained_removals = if mode == ToolUpdateMode::AdditionOnly {
                history.declarations.retain(|declaration| !removed.contains(&declaration.spec.name));
                for update in &mut history.updates {
                    update.added.retain(|name| !removed.contains(name));
                    update.removed.clear();
                }
                history.updates.retain(|update| !update.added.is_empty());
                Vec::new()
            } else { removed };
            if !added.is_empty() || !retained_removals.is_empty() {
                history.updates.push(ToolUpdate {
                    message_index: plan.input.len(), added, removed: retained_removals,
                });
            }
        }
        if self.projection.as_ref().is_some_and(|history|
            !history.declarations.is_empty() && history.declarations.iter().all(|tool| tool.defer_loading)) {
            // A tools-disabled finalization can persist an empty series.
            // Reopening it must declare the current authorized tools eagerly;
            // an all-deferred request has no entry point and is rejected by
            // the provider. This is the same series boundary as a first call.
            self.projection = Some(ToolHistoryProjection {
                declarations: plan.tools.iter().cloned().map(|spec| ToolDeclaration {spec,defer_loading:false}).collect(),
                updates:Vec::new(),
            });
        }
        self.mode = Some(mode);
        self.model = plan.runtime_profile.matched_model.clone();
        self.instructions = plan.instructions.clone();
        self.active_tools = plan.tools.clone();
        self.previous_input = plan.input.clone();
        plan.tool_history = self.projection.clone();
    }
}

impl ToolHistoryProjection {
    /// Keep declarations whose activation occurs before the retained message cut.
    /// A deferred definition without a retained addition cannot be replayed.
    pub fn truncated_at(&self, input_len: usize) -> Self {
        let updates: Vec<_> = self.updates.iter().filter(|update| update.message_index < input_len).cloned().collect();
        let declarations = self.declarations.iter().filter(|declaration|
            !declaration.defer_loading || updates.iter().any(|update| update.added.contains(&declaration.spec.name)))
            .cloned().collect();
        Self { declarations, updates }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tool(name: &str) -> ToolSpec {
        ToolSpec { name: name.into(), description: name.into(), parameters: serde_json::json!({"type":"object"}) }
    }
    fn plan(names: &[&str]) -> AgentRequestPlan {
        AgentRequestPlan::for_ad_hoc(crate::ModelRuntimeCatalog::default().resolve("deepseek-flash", crate::ProviderToolProtocol::Native, None),
            &[Message::system("rules"), Message::user("read")], &names.iter().map(|name| tool(name)).collect::<Vec<_>>())
    }
    #[test]
    fn additions_keep_original_definitions_and_activate_once_at_tail() {
        let mut history = ToolDeclarationHistory::default();
        let mut first = plan(&["read"]);
        history.project(&mut first, Some(ToolUpdateMode::AdditionOnly));
        let mut next = first.clone();
        next.input.push(Message::user("edit"));
        next.tools.push(tool("edit"));
        history.project(&mut next, Some(ToolUpdateMode::AdditionOnly));
        let projection = next.tool_history.clone().unwrap();
        assert_eq!(projection.declarations[0].spec, first.tools[0]);
        assert!(!projection.declarations[0].defer_loading);
        assert!(projection.declarations[1].defer_loading);
        assert_eq!(projection.updates[0].message_index, 2);
        history.project(&mut next, Some(ToolUpdateMode::AdditionOnly));
        assert_eq!(next.tool_history.unwrap(), projection);
    }
    #[test]
    fn removed_definitions_follow_route_capability_without_changing_active_tools() {
        for mode in [ToolUpdateMode::AdditionOnly, ToolUpdateMode::InHistory] {
            let mut history = ToolDeclarationHistory::default();
            let mut request = plan(&["read", "edit"]);
            history.project(&mut request, Some(mode));
            request.tools.pop();
            request.input.push(Message::user("read only"));
            history.project(&mut request, Some(mode));
            let projection = request.tool_history.unwrap();
            assert_eq!(request.tools.len(), 1);
            assert_eq!(projection.declarations.len(), if mode == ToolUpdateMode::InHistory { 2 } else { 1 });
            if mode == ToolUpdateMode::InHistory { assert_eq!(projection.updates[0].removed, vec!["edit"]); }
        }
    }
    #[test]
    fn schema_or_history_replacement_starts_a_new_declaration_series() {
        let mut history = ToolDeclarationHistory::default();
        let mut request = plan(&["read"]);
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        request.tools.push(tool("edit"));
        request.input.push(Message::user("next"));
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        request.tools[0].description = "changed".into();
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        assert!(request.tool_history.as_ref().unwrap().updates.is_empty());
        request.input[0] = Message::user("checkpoint");
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        assert!(request.tool_history.as_ref().unwrap().declarations.iter().all(|tool| !tool.defer_loading));
    }
    #[test]
    fn prefix_cut_omits_definitions_without_retained_activation() {
        let projection = ToolHistoryProjection {
            declarations: vec![ToolDeclaration { spec: tool("read"), defer_loading: false }, ToolDeclaration { spec: tool("edit"), defer_loading: true }],
            updates: vec![ToolUpdate { message_index: 2, added: vec!["edit".into()], removed: vec![] }],
        };
        assert_eq!(projection.truncated_at(2).declarations.len(), 1);
        assert_eq!(projection.truncated_at(3), projection);
    }
    #[test]
    fn persisted_history_resumes_against_the_saved_input_without_reduplicating_it() {
        let mut history = ToolDeclarationHistory::default();
        let mut request = plan(&["read"]);
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        request.tools.push(tool("edit"));
        request.input.push(Message::user("next"));
        history.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        let data = serde_json::to_value(&history).unwrap();
        assert!(data.get("previous_input").is_none());
        let mut resumed: ToolDeclarationHistory = serde_json::from_value(data).unwrap();
        resumed.resume_input(&request.input);
        let expected = request.tool_history.clone();
        resumed.project(&mut request, Some(ToolUpdateMode::AdditionOnly));
        assert_eq!(request.tool_history, expected);
    }

    #[test]
    fn board_tools_resume_eagerly_after_persisted_tools_disabled_finalization() {
        let names=["board.author","tutor.step","workspace.records.read","book.query","book.synthesize",
            "book.search_text","book.search","book.text","book.text_batch","tool.search","source.present",
            "book.context","book.concept","book.structure"];
        let registry=crate::orchestrator::resident_tool_registry();
        let tools=names.iter().map(|name|registry.registration(name).unwrap().spec.clone()).collect::<Vec<_>>();
        let mut history=ToolDeclarationHistory::default();
        let mut request=plan(&names);
        request.tools=tools.clone();
        history.project(&mut request,Some(ToolUpdateMode::AdditionOnly));
        request.input.push(Message::user("Finish with available text; tools are disabled."));
        request.tools.clear();
        history.project(&mut request,Some(ToolUpdateMode::AdditionOnly));
        assert!(request.tool_history.as_ref().unwrap().declarations.is_empty());
        let mut resumed:ToolDeclarationHistory=serde_json::from_value(serde_json::to_value(history).unwrap()).unwrap();
        resumed.resume_input(&request.input);
        request.input.push(Message::user("Regenerate the pending board edit using the current branch."));
        request.tools=tools;
        resumed.project(&mut request,Some(ToolUpdateMode::AdditionOnly));
        let projection=request.tool_history.as_ref().unwrap();
        assert!(projection.declarations.iter().any(|tool|tool.spec.name=="board.author" && !tool.defer_loading));
        assert!(projection.updates.is_empty(),"the tools-disabled series has ended");
        assert_eq!(projection.declarations.iter().map(|tool|tool.spec.clone()).collect::<Vec<_>>(),request.tools,"only the current authorized route is declared");
        let (wire,executable)=crate::deepseek_messages::request_projection("deepseek-v4-flash",&request,
            crate::deepseek_messages::MessagesCapabilities{tool_update:ToolUpdateMode::AdditionOnly,system_in_history:true}).unwrap();
        let board_name=executable.iter().find(|(_,name)|name.as_str()=="board.author").unwrap().0;
        let wire_board=wire["tools"].as_array().unwrap().iter().find(|tool|tool["name"]==board_name.as_str()).unwrap();
        assert_ne!(wire_board["defer_loading"],serde_json::json!(true),"the actual provider request has an executable eager board entry");
        let repaired=request.tool_history.clone();
        resumed.project(&mut request,Some(ToolUpdateMode::AdditionOnly));
        assert_eq!(request.tool_history,repaired,"the next unchanged request keeps the declaration prefix");
        request.input.push(Message::user("Read an additional permitted source."));
        request.tools.push(tool("library.read"));
        resumed.project(&mut request,Some(ToolUpdateMode::AdditionOnly));
        assert!(request.tool_history.unwrap().declarations.last().unwrap().defer_loading,"ordinary later activation remains deferred");
    }
}
