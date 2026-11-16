use super::*;

fn rates() -> ModelRates {
    serde_json::from_value(json!({"rates":[{"version":"first","provider":"deepseek","model":"test","starts_at":0,"expires_at":1000,
        "input_micro_cny_per_million":1,"cached_input_micro_cny_per_million":1,"output_micro_cny_per_million":1,
        "usage_contract":"deep_seek","image_meter":"unsupported","source":{"currency":"CNY","original_prices":"test","cny_conversion":"1","checked_on":"2026-11-08","official_url":"https://example.com"}}]})).unwrap()
}
#[test]
fn health_uses_loaded_configuration_and_contiguous_coverage() {
    let mut r=rates();
    let mut next=r.rates[0].clone(); next.version="next".into(); next.starts_at=1000; next.expires_at=1_000_000;
    r.rates.push(next);
    let health=|r:&ModelRates,at| model_health(Some(r),None,true,Some(("deepseek","test")),at);
    assert_eq!(health(&r,999)["coverage_until"],1_000_000);
    assert_eq!(health(&r,1000)["status"],"ready");
    assert_eq!(health(&r,1_000_000-7*86400)["status"],"warning");
    assert_eq!(health(&r,1_000_000-86400)["status"],"critical");
    assert_eq!(health(&r,1_000_000)["code"],"MODEL_RATE_EXPIRED");
    r.rates[1].starts_at=1001;
    assert_eq!(health(&r,999)["coverage_until"],1000);
    assert_eq!(health(&r,1000)["code"],"MODEL_RATE_COVERAGE_GAP");
    assert_eq!(model_health(Some(&r),None,true,Some(("other","test")),1)["code"],"MODEL_RATE_MODEL_MISMATCH");
    assert_eq!(model_health(None,Some("MODEL_RATE_FILE_UNREADABLE"),true,Some(("deepseek","test")),1)["code"],"MODEL_RATE_FILE_UNREADABLE");
    assert_eq!(model_health(Some(&r),None,false,Some(("deepseek","test")),1)["can_answer"],false);
    assert_eq!(model_health(Some(&r),None,true,None,1)["code"],"PROVIDER_NOT_CONFIGURED");
}
#[test]
fn file_diagnostics_separate_read_parse_and_invalid_intervals() {
    let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("rates.json");
    assert_eq!(ModelRates::load_diagnostic(&path).unwrap_err(),"MODEL_RATE_FILE_UNREADABLE");
    std::fs::write(&path,"{").unwrap();
    assert_eq!(ModelRates::load_diagnostic(&path).unwrap_err(),"MODEL_RATE_FILE_INVALID_JSON");
    let mut r=rates(); r.rates[0].expires_at=0;
    std::fs::write(&path,serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(ModelRates::load_diagnostic(&path).unwrap_err(),"MODEL_RATE_CONFIGURATION_INVALID");
}
#[test]
fn outcomes_are_idempotent_repairable_and_only_admin_probes_are_excluded() {
    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE users(user_id TEXT PRIMARY KEY,is_admin INTEGER); INSERT INTO users VALUES('reader',0),('admin',1);
        CREATE TABLE run_admissions(owner_user_id TEXT,client_request_id TEXT,turn_id TEXT,book_id TEXT);
        CREATE TABLE admin_usage_events(user_id TEXT,event_ref TEXT,kind TEXT,occurred_at INTEGER);").unwrap();
    db.execute_batch(include_str!("operational_outcomes.sql")).unwrap();
    db.execute_batch(include_str!("operational_recovery.sql")).unwrap();
    for owner in ["reader","admin"] {
        db.execute("INSERT INTO run_admissions VALUES(?,'ops-probe-example',?,'book')",params![owner,owner]).unwrap();
        write_turn(&db,owner,owner,"unsaved","save",Some("TURN_UNSAVED"),100).unwrap();
        write_turn(&db,owner,owner,"unsaved","save",Some("TURN_UNSAVED"),101).unwrap();
    }
    assert_eq!(db.query_row("SELECT count(*) FROM operational_outcomes WHERE finished_at=100",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(db.query_row("SELECT count(*) FROM operational_outcomes WHERE is_probe=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    write_turn(&db,"reader","reader","delivered","delivery",None,102).unwrap();
    assert_eq!(db.query_row("SELECT outcome FROM operational_outcomes WHERE user_id='reader'",[],|r|r.get::<_,String>(0)).unwrap(),"delivered");
    let err=crate::user_storage_paths::error("MODEL_ALLOWANCE_EXPIRED","unavailable","private message must not be retained");
    rejection(&db,"reader",&json!({"client_request_id":"new"}),&err,103).unwrap();
    rejection(&db,"reader",&json!({"client_request_id":"ops-probe-example"}),&err,104).unwrap();
    assert_eq!(db.query_row("SELECT count(*) FROM operational_outcomes",[],|r|r.get::<_,i64>(0)).unwrap(),3);
    assert_eq!(db.query_row("SELECT outcome FROM operational_outcomes WHERE request_id='new'",[],|r|r.get::<_,String>(0)).unwrap(),"account_blocked");
}

#[test]
fn recovery_scopes_do_not_merge_board_video_save_or_model_failures() {
    assert_eq!(recovery_scope(Some("BOARD_PREVIEW_UNAVAILABLE"),"delivery","u","b"),"board");
    assert_eq!(recovery_scope(Some("TURN_UNSAVED"),"save","u","b"),"save:u");
    assert_eq!(recovery_scope(Some("VIDEO_INDEX_INVALID"),"execution","u","b"),"video:b");
    assert_eq!(recovery_scope(Some("PROVIDER_ERROR"),"model","u","b"),"model");
}
#[test]
fn web_monitor_and_billing_agree_on_contiguous_coverage_and_invalid_rates() {
    use crate::web_usage::{health,rate,WebServiceKind};
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("web-rates.json");
    let mut r=json!({"rates":[{"version":"a","provider":"deepseek","service_kind":"web_search","starts_at":0,"expires_at":100,"cost_micro_cny_per_request":5000,"source":"fixture"},
        {"version":"b","provider":"deepseek","service_kind":"web_search","starts_at":100,"expires_at":1000000,"cost_micro_cny_per_request":5000,"source":"fixture"}]});
    std::fs::write(&path,r.to_string()).unwrap();
    assert_eq!(health(dir.path(),99)[0]["coverage_until"],1000000);
    assert_eq!(health(dir.path(),999999)[0]["status"],"critical");
    assert!(rate(dir.path(),WebServiceKind::Search,999999).is_ok());
    assert_eq!(health(dir.path(),1000000)[0]["status"],"unavailable");
    assert!(rate(dir.path(),WebServiceKind::Search,1000000).is_err());
    assert_eq!(health(dir.path(),99)[1]["status"],"unavailable");
    r["rates"][1]["starts_at"]=json!(99);std::fs::write(&path,r.to_string()).unwrap();
    assert_eq!(health(dir.path(),50)[0]["status"],"unavailable");
    assert!(rate(dir.path(),WebServiceKind::Search,50).is_err());
}

#[test]
fn saved_text_does_not_publish_board_or_video_recovery_evidence() {
    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE users(user_id TEXT PRIMARY KEY,is_admin INTEGER); INSERT INTO users VALUES('reader',0);
        CREATE TABLE run_admissions(owner_user_id TEXT,client_request_id TEXT,turn_id TEXT,book_id TEXT);
        INSERT INTO run_admissions VALUES('reader','request','turn','book');
        CREATE TABLE admin_usage_events(user_id TEXT,event_ref TEXT,kind TEXT,occurred_at INTEGER);").unwrap();
    db.execute_batch(include_str!("operational_outcomes.sql")).unwrap();
    db.execute_batch(include_str!("operational_recovery.sql")).unwrap();
    let session:AgentChatSession=serde_json::from_value(json!({"id":"s","book_id":"book","title":"test","created_at":"t","updated_at":"t","turns":[],"messages":[]})).unwrap();
    let mut turn:AgentChatTurn=serde_json::from_value(json!({"turn_id":"turn","user":"test","status":"completed",
        "outcome":{"answer":"done","answer_view":{"parts":[{"kind":"markdown","text":"done"}],"sources":[]},"incomplete":false,"turns":1,"tokens_spent":1,"effects":[],"trace":[],
            "profile_usage":runtime::orchestrator::ProfileUsageTrace::default(),"memory_updates":[]}})).unwrap();
    save_outcome(&db,"reader",&session,&turn,100).unwrap();
    let scopes=||db.prepare("SELECT scope FROM operational_successes ORDER BY scope").unwrap().query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(scopes(),vec!["execution:book","model","save:reader"]);
    turn.run_summary=Some(crate::agent_run::AgentRunSummary { usage:None,effects:vec![],trace:vec![runtime::orchestrator::TraceStep {
        tool:"video.search".into(),args:"{}".into(),result_digest:"candidate only".into(),model_tool_loop:None,query_audit:None}],activities:Some(vec![]),last_seq:None });
    save_outcome(&db,"reader",&session,&turn,101).unwrap();
    assert!(!scopes().contains(&"video:book".into()),"candidate search is not a verified video read");
    turn.run_summary.as_mut().unwrap().activities=Some(vec![serde_json::from_value(json!({
        "step_id":1,"kind":"tool","name":"book.text_batch","label":"Read returned video locations","status":"succeeded"})).unwrap()]);
    save_outcome(&db,"reader",&session,&turn,102).unwrap();
    assert!(scopes().contains(&"video:book".into()),"a successful batch read is a real video source read");
    turn.board_delivery=Some(memory::boards::BoardDelivery {board_id:"b".into(),commit_id:Some("c".into()),before_revision:0,after_revision:1,item_ids:vec!["i".into()],block_ids:vec![],origin_session_id:"s".into(),origin_turn_id:"turn".into(),parent_item_id:None});
    save_outcome(&db,"reader",&session,&turn,102).unwrap();
    assert!(scopes().contains(&"board".into()));
    assert_eq!(db.query_row("SELECT finished_at FROM operational_successes WHERE scope='board'",[],|r|r.get::<_,i64>(0)).unwrap(),100,"replaying a saved outcome must not invent a later recovery");
}
