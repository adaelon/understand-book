//! Explicit live calibration of the same after-turn observer used by note thinking paths.
use serde_json::{json, Value};
struct Captured<'a> {
    inner: &'a dyn runtime::ModelAdapter,
    outputs: std::cell::RefCell<Vec<Value>>,
    request_sizes: std::cell::RefCell<Vec<Value>>,
}
impl runtime::ModelAdapter for Captured<'_> {
    fn complete(
        &self,
        request: runtime::CompletionRequest,
    ) -> Result<runtime::ParsedResponse, runtime::AdapterError> {
        self.inner.complete(request)
    }
    fn chat(
        &self,
        request: &runtime::AgentRequestPlan,
    ) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        self.inner.chat(request)
    }
    fn complete_structured(
        &self,
        request: runtime::CompletionRequest,
    ) -> Result<Value, runtime::AdapterError> {
        self.request_sizes.borrow_mut().push(json!({"system_utf8_bytes":request.system.len(),"user_utf8_bytes":request.user.len(),"output_token_limit":request.output_token_limit}));
        let result = self.inner.complete_structured(request);
        if let Ok(value) = &result {
            self.outputs.borrow_mut().push(value.clone());
        }
        result
    }
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn set_model_purpose(&self, purpose: &str) {
        self.inner.set_model_purpose(purpose)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("Pass an output directory")?);
    std::fs::create_dir_all(&directory)?;
    let config = runtime::ProviderConfig::from_env().map_err(|e| e.message)?;
    let adapter = runtime::ProviderRegistry::adapter_from_config(config.clone());
    let source="全程平均速度等于总路程除以从出发到到达的总时间，停车时间也属于这段时间。只统计运动时间得到的是运动期间的平均速度，回答的问题不同。相同路程下，总时间越长，全程平均速度越小。10 千米的行程中运动 10 分钟、停车 5 分钟，总时间为 15 分钟即 0.25 小时，全程平均速度是 40 千米每小时。";
    let note = "停车的时候没运动，这段时间应该不算吧？";
    let target = json!({"focus_ref":"focus:elapsed","learning_focus":"全程平均速度使用包含停车的总时间","expected_performance":"依据行程起止判断时间范围并解释变化","capability":"application","source_bindings":[{"source_id":"book","source_revision":"v1","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]});
    let cases=[
        ("question","停车的时间也算吗？我还不确定。",false),
        ("correct_reason","同样走 10 千米，若途中停车变长，从出发到到达的时间更长，所以全程平均速度更小。",false),
        ("right_answer_wrong_reason","这次平均速度是 40 千米每小时，因为平均速度只看运动时间，停车不算。",false),
        ("arithmetic_slip","停车属于出发到到达这段行程，所以总时间是 10 加 5 等于 15 分钟，也就是 0.25 小时；10 除以 0.25 我算成 25。",false),
        ("assisted","按照刚才的讲解，停车还没有到达终点，所以它属于全程时间。",true),
        ("revised","我前面用是否移动来决定计时不对。现在我用出发和到达来定范围：停车还在途中，必须计入。这也意味着停车更长时全程平均速度更小。",true),
    ];
    let mut reports = Vec::new();
    let selected = std::env::args().nth(2);
    for (name, words, helped) in cases {
        if selected.as_deref().is_some_and(|value| value != name) {
            continue;
        }
        let prior_help: Vec<Value> = if helped {
            vec![
                json!({"event_id":"help-before","answer":"全程按出发到到达计时，停车仍在途中。","occurred_at":"2026-11-01T01:00:00Z"}),
            ]
        } else {
            vec![]
        };
        let input = json!({"trigger":"after_turn","source_turn_ref":name,"observation_only":true,"teaching_context":{"status":"observing"},
            "known_focuses":[target],"read_sources":[{"range":{"start_lid":"1.1","end_lid":"1.1"},"text":source}],
            "personal_notes":{"references":[{"note_id":"note","revision":1}],"explicit":[{"reference":{"note_id":"note","revision":1},"content":note}]},
            "observation_facts":[{"fact_ref":name,"fact_refs":[name],"user_expression":words,"prior_help":prior_help,"help_before_expression":if helped{"prior_help_recorded"}else{"no_recorded_prior_help"},"already_interpreted":false}],
            "actual_delivery":{"relative_to_expression":"after","answer":"你提到了时间范围。我们沿这段行程继续看。"}});
        let started = std::time::Instant::now();
        let captured = Captured {
            inner: adapter.as_ref(),
            outputs: Default::default(),
            request_sizes: Default::default(),
        };
        let result = runtime::tutor_coach::execute_coach(&captured, input.clone());
        let report = json!({"case":name,"elapsed_ms":started.elapsed().as_millis(),"model":config.model,"input":input,
            "result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|&e.message),"public_outputs":captured.outputs.into_inner(),"request_sizes":captured.request_sizes.into_inner()});
        std::fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!(
            "{name}: {}",
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
        );
        reports.push(report);
    }
    std::fs::write(
        directory.join("report.json"),
        serde_json::to_vec_pretty(&reports)?,
    )?;
    if reports.iter().any(|r| !r["error"].is_null()) {
        return Err("At least one observation failed; inspect retained output".into());
    }
    Ok(())
}
