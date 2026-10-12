// Shared deterministic model boundary for Runtime and local MCP contract tests.
// Book parsing, retrieval, dispatch and citation validation remain real.
#[derive(Default)]
struct ExampleAdapter {
    requests: std::sync::Mutex<Vec<runtime::CompletionRequest>>,
}

impl runtime::ModelAdapter for ExampleAdapter {
    fn complete(&self, request: runtime::CompletionRequest) -> Result<runtime::ParsedResponse, runtime::AdapterError> {
        self.requests.lock().unwrap().push(request);
        Ok(runtime::ParsedResponse {
            sufficient: true,
            answer: Some("Alpha requires > 10; beta includes equality.".into()),
            citations: vec![
                runtime::RawCitation { lid: "1.1".into(), text: "greater than 10".into(), role: "support".into() },
                runtime::RawCitation { lid: "1.2".into(), text: "at least 10".into(), role: "support".into() },
            ],
            model_supplement: vec![],
        })
    }

    fn complete_structured(&self, request: runtime::CompletionRequest) -> Result<serde_json::Value, runtime::AdapterError> {
        let planning = request.system.contains("PlanGate");
        self.requests.lock().unwrap().push(request);
        Ok(if planning {
            serde_json::json!({
                "plan_gate":{"valid":true,"missing_requirements":[],"target_issues":[]},
                "candidate_fits":[
                    {"target_index":0,"candidate_id":"concept:alpha","fit":"direct_match","reason":"named alpha"},
                    {"target_index":0,"candidate_id":"concept:beta","fit":"reject","reason":"other target"},
                    {"target_index":1,"candidate_id":"concept:beta","fit":"direct_match","reason":"named beta"},
                    {"target_index":1,"candidate_id":"concept:alpha","fit":"reject","reason":"other target"}
                ],"probes":[]
            })
        } else {
            serde_json::json!({
                "answer":"Alpha requires > 10; beta includes equality.",
                "assessments":[
                    {"obligation_index":0,"verdict":"supported","citation_lids":["1.1","1.2"],"support_note":"Both conditions read"},
                    {"obligation_index":1,"verdict":"supported","citation_lids":["1.1","1.2"],"support_note":"Equality differs"}
                ],
                "citations":[{"lid":"1.1","text":"greater than 10","role":"support"},{"lid":"1.2","text":"at least 10","role":"support"}],
                "model_supplement":[]
            })
        })
    }

    fn chat(&self, _: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        panic!("Example dispatch does not call an outer model")
    }
}
