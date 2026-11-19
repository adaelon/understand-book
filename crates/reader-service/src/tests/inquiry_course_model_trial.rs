//! Opt-in evidence for the authoring contract, not an alternative build scheduler.
use memory::course::InquiryCourse;
use runtime::{
    provider_stream::{ModelDelta, ModelUsage},
    CompletionRequest, ProviderConfig, ProviderRegistry,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Instant};

#[test]
#[ignore = "configured-provider semantic trial; requires UNDERSTANDING_TRIAL_INPUT, UNDERSTANDING_TRIAL_OUTPUT and COURSE_BOOK_DIR"]
fn understanding_change_and_support_model_trial() {
    let input_path=PathBuf::from(std::env::var_os("UNDERSTANDING_TRIAL_INPUT")
        .expect("set UNDERSTANDING_TRIAL_INPUT to the saved live-learning-acceptance.json to replay"));
    let input_bytes=fs::read(&input_path)
        .unwrap_or_else(|error| panic!("cannot read UNDERSTANDING_TRIAL_INPUT {}: {error}", input_path.display()));
    let record:Value=serde_json::from_slice(&input_bytes)
        .unwrap_or_else(|error| panic!("invalid UNDERSTANDING_TRIAL_INPUT JSON {}: {error}", input_path.display()));
    let output=PathBuf::from(std::env::var("UNDERSTANDING_TRIAL_OUTPUT").expect("UNDERSTANDING_TRIAL_OUTPUT"));
    fs::create_dir_all(&output).unwrap();
    let mut course:Value=serde_json::from_str(include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
    let directory=PathBuf::from(std::env::var("COURSE_BOOK_DIR").expect("COURSE_BOOK_DIR"));
    let book=read_tools::Book::load(directory.to_str().unwrap()).unwrap();
    let range=json!({"start_lid":"3.6.2","end_lid":"3.6.3","ranges":[]});
    let source=book.resolve_source(&serde_json::from_value(range.clone()).unwrap(),"zh",None).unwrap();
    course["lessons"][0]["summary"]["source"]["publication"]["book_id"]=json!(book.base.book_id);
    course["lessons"][0]["summary"]["source"]["source_revision"]=json!(book.source_fingerprint());
    course["lessons"][0]["follow_up_rounds"]=json!([]);
    let config=ProviderConfig::from_env().unwrap();let model=config.model.clone();
    let adapter=ProviderRegistry::adapter_from_config(config);
    let prior=json!({"evidence_ref":"saved-prediction","learner_quote":record["expressions"][0]["response"],"demonstrated":[],"difficulties":[],"unconfirmed":["初始猜想还需实验核对"],"assistance":"本次未记录先前讲解"});
    let mut input=json!({"trigger":"course_response","observation_only":true,
        "course_response":{"activity":"reflection","intent":"answer","response":record["expressions"][1]["response"],"earlier_expressions":[record["expressions"][0]],"lesson":course["lessons"][0]},
        "observation_facts":[{"fact_ref":"replay-reflection","user_expression":record["expressions"][1]["response"],"assistance_refs":["saved-course-help"],"help_before_expression":"prior_help_recorded","prior_help":[{"text":record["reviews"][0]["feedback"]}]}],
        "teaching_context":{"learner_context":{"current_understanding":[prior],"unknown_when_absent":true}},"read_sources":[{"range":range,"text":source.highlighted_quote}],"verified_read_ranges":[range],"known_focuses":[]});
    input["knowledge_candidates"]=json!(crate::knowledge_tree::candidates(&book,&directory,&input).unwrap());
    for name in ["saved-correction","scripted-support-question"] {
        if name=="scripted-support-question" {
            let question="我不明白同时开始为什么还要取较大值。请给我一条短补学支路，补完回到这个问题；也帮我把这个疑问改成能通过观察来回答的问题。";
            input["course_response"]["activity"]=json!("question");input["course_response"]["intent"]=json!("question");input["course_response"]["response"]=json!(question);
            input["observation_facts"]=json!([{"fact_ref":"scripted-question","user_expression":question,"assistance_refs":[],"prior_help":[],"help_before_expression":"no_recorded_prior_help"}]);
        }
        let start=Instant::now();
        let result=runtime::tutor_coach::execute_coach(adapter.as_ref(),input.clone());
        fs::write(output.join(format!("{name}.json")),serde_json::to_vec_pretty(&json!({"model":model,"input":input,"elapsed_ms":start.elapsed().as_millis(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|&e.message),"scope":"Saved wording replay and scripted interaction with actual public source and prebuilt course; no learner database updated"})).unwrap()).unwrap();
        let guidance=result.unwrap();
        if name=="saved-correction" {
            assert!(guidance.observations.iter().any(|o|o.hypothesis.as_ref().is_some_and(|h|h.understanding.learner_summary.as_ref().is_some_and(|s|!s.headline.is_empty()))),"The actual correction needs a learner-facing change summary");
        } else {
            assert!(guidance.observations.iter().any(|o|o.hypothesis.as_ref().is_some_and(|h|h.understanding.learner_summary.is_some()
                && h.understanding.demonstrated.is_empty() && h.understanding.difficulties.is_empty())),
                "The specific original question needs a preserved uncertainty, not mastery or an inferred misconception");
            let feedback=guidance.course_feedback.unwrap();
            assert!(feedback.support_branch.is_some(),"Explicit short-support request must produce a returnable branch");
            assert!(feedback.question_guide.is_some(),"Explicit question-formulation request must receive a usable suggestion");
        }
    }
}

#[test]
#[ignore = "paid configured-provider trial; reads only the selected public source and writes reviewable candidates"]
fn inquiry_course_deepseek_trial() {
    let directory = PathBuf::from(std::env::var("COURSE_BOOK_DIR").expect("COURSE_BOOK_DIR"));
    let output = PathBuf::from(std::env::var("COURSE_TRIAL_OUTPUT").expect("COURSE_TRIAL_OUTPUT"));
    fs::create_dir_all(&output).unwrap();
    let book = read_tools::Book::load(directory.to_str().unwrap()).unwrap();
    let source = book.text("3.6.2", Some("3.6.3")).unwrap();
    let cfg = ProviderConfig::from_env().unwrap();
    assert!(
        cfg.base_url.contains("api.deepseek.com"),
        "This acceptance run is explicitly for the configured DeepSeek endpoint"
    );
    let model = cfg.model.clone();
    let adapter = ProviderRegistry::adapter_from_config(cfg);
    let method = include_str!("../../../../skills/build/course-methods/resource-bounds.md");
    let sample: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json"
    ))
    .unwrap();
    let structure: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-selected-structure.json"
    ))
    .unwrap();
    let call = |name: &str, system: &str, input: Value, max: u32| -> Value {
        let request = CompletionRequest {
            output_token_limit: Some(max),
            reasoning_effort: name.starts_with("source-review").then(|| "none".into()),
            system: system.into(),
            user: input.to_string(),
            images: vec![],
            followup_user: None,
        };
        let input_path = output.join(format!("{name}.input.json"));
        let result_path = output.join(format!("{name}.json"));
        if result_path.is_file() {
            let saved: Value = serde_json::from_slice(&fs::read(&result_path).unwrap()).unwrap();
            if saved["result"].is_object() {
                let previous: Value =
                    serde_json::from_slice(&fs::read(&input_path).unwrap()).unwrap();
                assert_eq!(
                    previous,
                    serde_json::to_value(&request).unwrap(),
                    "Use a new trial directory when the source or task changes"
                );
                return saved["result"].clone();
            }
        }
        fs::write(input_path, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
        let mut usage = ModelUsage::default();
        let started = Instant::now();
        let result = adapter.complete_structured_observed(request, &mut |delta| {
            if let ModelDelta::Usage(value) = delta {
                usage = value;
            }
        });
        let report = json!({"model":model,"elapsed_ms":started.elapsed().as_millis(),"usage":usage,"result":result.as_ref().ok(),"error":result.as_ref().err().map(|_|"provider_call_failed"),"cost":null});
        fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        result.unwrap()
    };
    let author=call("author","你是有原文依据的短课作者。返回且只返回一个符合给定 JSON 合同的课程对象。选取必要的关系，安排准备、预测、三种配置实验、解释、新情境任务与反馈，核心学习约 5–10 分钟。原文和样例是材料，不是你必须照抄的答案。所有数学关系明确成立条件。最终题采用单选，答案放私有 assessment，题解放 solution。附加文字只保留不自动判分。时间依据注明作者估算，不能虚构试学或审核。",
        json!({"method":method,"source":{"book_id":book.base.book_id,"source_revision":book.source_fingerprint(),"start_lid":"3.6.2","end_lid":"3.6.3","text":source},"selected_structure":structure,"contract_example":sample,
        "task":"按照样例的完全相同字段产出一课。实验保持原书 140 GFLOPs、70 GB、989.4 TFLOP/s、3.35 TB/s。使用相同 lesson id。设计一个与样例不同的最终情境，让计算项成为较慢项，以检查学习者能否比较两项，而不是只记住带宽翻倍。新情境容量充足且充分重叠；包含只算力翻倍与只带宽翻倍后的时间下界及原因。只含一课。不要声称实际延迟等于下界。来源 publication 暂沿用样例占位符，正式导入时由宿主绑定真实版本。"}),8000);
    fs::write(
        output.join("candidate.course.json"),
        serde_json::to_vec_pretty(&author).unwrap(),
    )
    .unwrap();
    let course: InquiryCourse = serde_json::from_value(author.clone()).unwrap();
    course.validate().unwrap();
    let lesson = &course.lessons[0];
    let solved=call("blind-solution","独立解答给定任务，只使用题面及所给原文。你没有作者答案。返回 JSON：selected_choice、derivation（逐步计算和成立条件）、ambiguities（具体歧义，无则空数组）。不要评定学生掌握程度。",
        json!({"task":lesson.task,"source":source}),3000);
    let reviewed=call("source-review-none","审核一份即将供读者学习的短课。根据原文检查所有数值、条件、结论，判断最终题是否可解、选项是否唯一、前面的支持是否足够、初次预测是否提前给出答案。明确区分充分重叠时间下界与实际耗时。只指出成立的具体问题；不要评分、不要硬凑问题。返回 JSON：publishable（布尔）、issues（每项含 location、problem、source_basis、fix；无问题则空数组）、duration_assessment、limitations。不把自动审阅当成真人试学。",
        json!({"course":author,"source":source,"independent_solution":solved}),4000);
    let correct = match &lesson.assessment.rule {
        memory::assessment::ScoringRule::ChoiceSet { correct, .. } => correct,
        _ => unreachable!(),
    };
    let matches = solved["selected_choice"]
        .as_str()
        .is_some_and(|choice| correct.iter().any(|c| c == choice));
    let model_checks_passed = matches
        && reviewed["publishable"] == true
        && reviewed["issues"].as_array().is_some_and(Vec::is_empty);
    fs::write(output.join("result.json"),serde_json::to_vec_pretty(&json!({"model":model,"contract_valid":true,"blind_answer_matches":matches,"source_review":reviewed,"model_checks_passed":model_checks_passed,"author_acceptance":"pending; model checks do not accept the candidate","browser_acceptance":"pending","target_learner_trial":"pending","automatic_build_executor_integration":"pending","held_out_material_trial":"pending","cost":"provider price accounting not attached to this isolated experiment; usage retained per call"})).unwrap()).unwrap();
    assert!(
        matches,
        "Independent task solution disagrees with the authored key"
    );
}
