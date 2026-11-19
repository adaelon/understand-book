//! WB1 browser acceptance uses the product routes and isolated persistent files.
use super::*;
use memory::{learning_evidence::LearningEvidence, teaching::TeachingEvent};
use runtime::presentation::PresentationContent;

pub const SOURCE: &str = "平均速度是总路程除以总时间。把一次旅程拆成几段时，应先把路程相加，再把时间相加。只有各段用时相同，各段速度的算术平均才等于全程平均速度。\n\n例如，同样走 60 米，去程每秒 3 米，用时 20 秒；回程每秒 1 米，用时 60 秒。全程平均速度是 120 米除以 80 秒，等于每秒 1.5 米，而不是每秒 2 米。";
pub const QUOTE: &str = "我明白了，慢的那段花的时间更多，所以不能把两个速度直接相加再除以二。";
pub const NOW: &str = "2026-10-25T08:00:00Z";

pub const PRESENTATION: &str = r#"<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>同样路程，不同用时</title>
<style>body{margin:0;padding:28px;color:#233e40;background:#f5f4ed;font:16px/1.65 sans-serif}main{max-width:660px;margin:auto}h1{font-size:24px}label{display:block;margin:20px 0}input{width:100%;accent-color:#18857e}button{font:inherit;border:1px solid #afc4bf;background:white;color:#233e40;border-radius:9px;padding:9px 16px}output{font-weight:700}.track{height:22px;background:#dfebe6;border-radius:5px;margin:8px 0 18px}.bar{height:100%;background:#18857e;border-radius:5px}.return{background:#b98335}</style>
<main><h1>同样路程，不同用时</h1><p>去程与回程各 60 米。去程速度固定为 3 m/s，试着改变回程速度。</p>
<label>回程速度：<output id="speed-label"></output> m/s<input id="return-speed" aria-label="回程速度" type="range" min="1" max="3" step="0.5" value="1" data-presentation-parameter="return_speed"></label>
<p>去程用时：20 秒</p><div class="track"><div class="bar" style="width:33.333%"></div></div><p>回程用时：<output id="return-time"></output> 秒</p><div class="track"><div id="return-bar" class="bar return"></div></div>
<p id="result"></p><button id="compare" data-presentation-action="compare">显示比较结论</button><p id="conclusion" hidden>慢的路段占用更长时间。平均速度要用总路程除以总时间。</p></main>
<script>
const model={return_speed:1,revealed:false,...window.presentation.initialState};
const byId=id=>document.getElementById(id);
function render(){const seconds=60/model.return_speed;byId('return-speed').value=model.return_speed;byId('speed-label').textContent=model.return_speed;byId('return-time').textContent=seconds.toFixed(1);byId('return-bar').style.width=(seconds/60*100)+'%';byId('result').textContent='全程平均速度：120 ÷ (20 + '+seconds.toFixed(1)+') = '+(120/(20+seconds)).toFixed(2)+' m/s';byId('conclusion').hidden=!model.revealed;}
byId('return-speed').oninput=event=>{model.return_speed=Number(event.target.value);render();window.presentation.commitState();};
byId('compare').onclick=()=>{model.revealed=true;render();window.presentation.commitState();};
window.presentation.registerStateReader(()=>({values:{return_speed:model.return_speed,revealed:model.revealed},visible_step:model.revealed?'comparison':'explore',observation:{summary:byId('result').textContent,state:{return_speed:model.return_speed,revealed:model.revealed},rendered:{return_time:60/model.return_speed,average_speed:120/(20+60/model.return_speed)}}}));
window.presentation.registerStateRestorer(scene=>{Object.assign(model,scene.values.page);render();});
render();
</script></html>"#;

/// Shared by Board API tests and this opt-in browser host; no model is called.
pub fn fixture() -> (tempfile::TempDir, AppState, Value) {
    let mut base = sample_base();
    base.book_id = "wb1-average-speed".into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    for node in &mut base.lid_nodes {
        node.span = Span { start: 0, end: SOURCE.encode_utf16().count() };
    }
    let (root, mut state) = super::tutor_loop_tests::fixture_with_book(
        true, Book::new(base, SOURCE), SOURCE,
    );
    let source_id = state.workspace.book.base.book_id.clone();
    let source_revision = state.workspace.book.source_fingerprint().to_owned();
    let map_path = root.path().join("teaching/versions/v1/map.json");
    let mut map: Value = serde_json::from_slice(&std::fs::read(&map_path).unwrap()).unwrap();
    let object = &mut map["objects"]["objects"][0];
    object["kind"] = json!("concept");
    object["meaning"] = json!("平均速度：总路程 ÷ 总时间");
    object["aliases"] = json!(["全程平均速度"]);
    object["conditions"] = json!(["总时间大于零", "只有各段用时相同，才可以直接平均各段速度"]);
    std::fs::write(map_path, serde_json::to_vec_pretty(&map).unwrap()).unwrap();

    let turn = precommit_agent_turn(&mut state, &source_id,
        "为什么同样的去程和回程，平均速度不是 2 m/s？".into(), None, None, None, NOW).unwrap();
    let range = EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] };
    let resolved = state.workspace.book.resolve_source(&range, "zh-CN", None).unwrap();
    let binding = SourceBinding {
        source_ref_id: "source-wb1".into(), book_id: source_id.clone(), publication_id: None,
        source_revision: Some(source_revision.clone()), evidence_range: Some(range),
        evidence_text_digest: resolved.evidence_text_digest, media: None, music: None,
        label_snapshot: resolved.label, preview_snapshot: resolved.preview,
    };
    let content = PresentationContent {
        title: "同样路程，不同用时".into(),
        content_files: BTreeMap::from([("index.html".into(), PRESENTATION.into())]),
        entrypoint: "index.html".into(),
        readable_content: "去程和回程各 60 米；去程速度为 3 m/s，回程速度可从 1 调到 3 m/s。比较两段用时，并用总路程除以总时间得到平均速度。".into(),
        source_bindings: vec![binding.clone()], assumptions: vec!["两段路程相同，每段各自保持匀速".into()],
        state_contract: json!({"return_speed":"number from 1 to 3 in steps of 0.5","revealed":"whether the comparison conclusion is visible"}),
        initial_state: json!({"return_speed":1,"revealed":false}),
        action_contract: serde_json::from_value(json!([
            {"kind":"set_parameters","parameters":["return_speed"]},
            {"kind":"invoke","action_id":"compare","label":"显示比较结论"}
        ])).unwrap(),
        teaching_binding: None, method_refs: None, method_observations: Default::default(),
        animation_assets: Default::default(),
    };
    let candidate = state.private_context().create_presentation_candidate(
        &turn.session_id, &turn.turn_id, None, content).unwrap();
    let reference = super::presentation_store_tests::persist(&mut state, &turn, &candidate);
    let explanation = "去程 60 米，速度为 3 m/s，所以需要 20 秒；回程同样 60 米，速度为 1 m/s，却需要 60 秒。\n\n全程路程是 120 米，总时间是 80 秒，因此平均速度为 **1.5 m/s**。直接把 3 和 1 平均得到 2，相当于错误地假设两段用时相同。\n\n比较时先确认保持不变的是路程还是时间。慢的路段占用了更多时间，这就是它对平均速度影响更大的原因。 [[source:source-wb1]]";
    let mut outcome = super::presentation_store_tests::outcome(&reference);
    outcome.answer = Some(explanation.into());
    outcome.answer_view.as_mut().unwrap().parts.insert(0, AgentAnswerPart::Markdown { text: explanation.into() });
    outcome.source_bindings = vec![binding];
    let messages = state.workspace.messages.clone();
    finalize_agent_turn_completed(&mut state, &turn, &outcome, &messages, NOW).unwrap();

    let mut learning = state.user.learning_store().unwrap();
    let tutor_session = learning.state().unwrap().control.current_tutor_session_id.unwrap();
    let fact: TeachingEvent = serde_json::from_value(json!({
        "event_id":"wb1-learner-expression","binding":{
            "tutor_session_id":tutor_session,"session_revision":1,"control_revision":1,
            "source_id":source_id,"source_revision":source_revision,"map_revision":"v1",
            "chat_session_id":turn.session_id,"turn_id":turn.turn_id},
        "kind":"usage_observed","causal_refs":[],"occurred_at":NOW,
        "payload":{"usage":{"kind":"user_message"},"response":QUOTE}
    })).unwrap();
    learning.append_teaching(&fact).unwrap();
    let evidence: LearningEvidence = serde_json::from_value(json!({
        "evidence_id":"wb1-understanding","action_ref":fact.event_id,
        "session_id":tutor_session,"source_id":source_id,"source_revision":source_revision,
        "map_revision":"v1","nature":"hypothesis","fact_refs":[fact.event_id],
        "teaching_implication":"后续可以用等时间的对照例子，检查能否辨别算术平均的适用条件。",
        "label":"平均速度与用时权重","capability":"explanation","assistance_refs":[],
        "attempt":0,"status":"unassessed","interpretation":"已经指出慢速段用时更长，并能解释直接平均两个速度的问题。",
        "learner_quote":QUOTE,"interpreter_version":"wb1-fixture","source_quotes":[],
        "target":{"learning_focus":"平均速度与用时权重","expected_performance":"解释为什么等路程的两段不能直接平均速度",
            "capability":"explanation","source_bindings":[{"source_id":source_id,"source_revision":source_revision,"start_lid":"1.1","end_lid":"1.1"}],
            "object_refs":[{"source_id":source_id,"object_id":"speed","object_revision":1}]}
    })).unwrap();
    learning.append_evidence(&evidence).unwrap();
    drop(learning);
    let metadata = json!({
        "book_id":source_id,"session_id":turn.session_id,"turn_id":turn.turn_id,
        "reference":reference,"evidence_id":evidence.evidence_id,"learner_quote":QUOTE,
        "sources":{
            "prebuild":{"kind":"prebuild","object_id":"speed","object_revision":1,"map_revision":"v1"},
            "answer":{"kind":"answer","session_id":turn.session_id,"turn_id":turn.turn_id},
            "presentation":{"kind":"presentation","session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference},
            "understanding":{"kind":"understanding","evidence_id":evidence.evidence_id}
        }
    });
    (root, state, metadata)
}

#[test]
#[ignore = "real local HTTP WB1 fixture; POST /test/shutdown to stop"]
fn board_browser_host() {
    let (root, mut state, mut metadata) = fixture();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb1");
    std::fs::create_dir_all(&output).unwrap();
    let server = tiny_http::Server::http("127.0.0.1:4218").unwrap();
    metadata["backend"] = json!("http://127.0.0.1:4218");
    metadata["root"] = json!(root.path());
    std::fs::write(output.join("fixture.json"), serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
    println!("WB1_BROWSER_READY http://127.0.0.1:4218");
    let deadline = std::time::Instant::now() + Duration::from_secs(3600);
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else { continue };
        let path = request.url().to_owned();
        if path == "/test/shutdown" && request.method().as_str() == "POST" {
            request.respond(tiny_http::Response::from_string("stopped")).unwrap();
            break;
        }
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        let reply = if path == "/test/fixture" {
            ok_json(&metadata)
        } else if path == "/test/reopen" && request.method().as_str() == "POST" {
            state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
            state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
            ok_json(&json!({"reopened":true}))
        } else {
            route(&mut state, Req { method: request.method().as_str(), url: &path, body: &body, now: NOW })
        };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
    }
}
