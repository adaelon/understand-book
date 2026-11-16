use super::mu5_tests::Fixture;
use serde_json::{json, Value};

fn fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.control.set_reader_admin("B",true).unwrap();
    fixture
}
pub(super) fn publication(f: &Fixture, operation:&str, expected:u32) -> Value {
    json!({"operation_id":operation,"release_id":"exam-course","expected_version":expected,"visible_to_users":["A","B"],"manifest":{
        "name":"期末冲刺公共资料","publisher":"课程运营","public_goal":"完成两份材料的课程复习","applicability":"本课程期末考试范围","prerequisites":"基础代数",
        "materials":[{"published_book_ref":f.x,"required_capabilities":["text","tutor"]},{"published_book_ref":f.y,"required_capabilities":["text"]}],
        "topic_links":[{"title":"两份材料中的共同概念","description":"先概念再应用","materials":[f.x,f.y]}],
        "suggested_route":[{"title":"从概念开始","description":"阅读第一份资料再对照第二份资料","materials":[f.x,f.y]}]
    }})
}
fn publish(f:&Fixture,input:Value)->Value { f.ok(&f.b,"POST","/api/admin/workspace-releases",input) }
fn detail(f:&Fixture,token:&str,version:u32)->Value { f.ok(token,"GET",&format!("/api/workspace-releases/exam-course/versions/{version}"),Value::Null) }

fn book_filter_publication(f:&Fixture, id:&str, expected:u32, sources:&[&crate::published_library::PublishedBookRef])->Value {
    let mut input = publication(f, &format!("{id}-v{}",expected+1), expected);
    input["release_id"] = json!(id);
    let mut course:Value = serde_json::from_str(include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json")).unwrap();
    let original = course["lessons"][0].clone();
    let lessons:Vec<_> = sources.iter().enumerate().map(|(index,reference)| {
        let book = f.access.library.lock().unwrap().load("A",reference).unwrap();
        let mut lesson = original.clone();
        lesson["summary"]["id"] = json!(format!("lesson-{index}"));
        lesson["summary"]["source"] = json!({"publication":reference,"source_revision":book.book.source_fingerprint(),
            "start_lid":"1.1","end_lid":"1.3","label":"书籍课程筛选测试来源","structure_refs":["fixture"]});
        lesson["assessment"]["source_lids"] = json!(["1.2"]);
        lesson
    }).collect();
    course["routes"][0]["lesson_ids"] = json!(lessons.iter().map(|lesson|&lesson["summary"]["id"]).collect::<Vec<_>>());
    course["lessons"] = json!(lessons);
    input["manifest"]["inquiry_course"] = course;
    input
}

#[test]
fn s1_book_filter_applies_before_pagination_and_counts_matching_courses() {
    let f = fixture();
    publish(&f,book_filter_publication(&f,"a-other",0,&[&f.y]));
    publish(&f,book_filter_publication(&f,"b-single",0,&[&f.x]));
    publish(&f,book_filter_publication(&f,"c-cross",0,&[&f.x,&f.y]));
    let mut materials_only = publication(&f,"d-materials-only",0);
    materials_only["release_id"] = json!("d-materials-only");
    publish(&f,materials_only);
    f.control.connection.execute("UPDATE workspace_releases SET created_at=100",[]).unwrap();
    let costs_before:i64 = f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|row|row.get(0)).unwrap();
    let first = f.ok(&f.a,"GET","/api/workspace-releases?limit=1&offset=0",Value::Null);
    assert_eq!(first["items"][0]["release_id"],"a-other");
    assert_eq!(f.ok(&f.a,"GET","/api/workspace-releases?limit=1&offset=1",Value::Null)["items"][0]["release_id"],"b-single");
    let path = format!("/api/workspace-releases?book_id={}&limit=1",f.x.book_id);
    let first = f.ok(&f.a,"GET",&path,Value::Null);
    assert_eq!(first["total"],2);
    assert_eq!(first["limit"],1);
    assert_eq!(first["offset"],0);
    assert_eq!(first["items"][0]["release_id"],"b-single");
    let second = f.ok(&f.a,"GET",&format!("{path}&offset=1"),Value::Null);
    assert_eq!(second["total"],2);
    assert_eq!(second["offset"],1);
    assert_eq!(second["items"][0]["release_id"],"c-cross");
    let lessons = second["items"][0]["manifest"]["inquiry_course"]["lessons"].as_array().unwrap();
    assert_eq!(lessons.len(),2,"matching a book preserves the cross-book course directory");
    assert_eq!(lessons[0]["source"]["publication"],json!(f.x));
    assert_eq!(lessons[1]["source"]["publication"],json!(f.y));
    let empty = f.ok(&f.a,"GET","/api/workspace-releases?book_id=missing-book&limit=7&offset=0",Value::Null);
    assert_eq!(empty,json!({"items":[],"total":0,"limit":7,"offset":0}));
    let exhausted = f.ok(&f.a,"GET",&format!("{path}&offset=2"),Value::Null);
    assert_eq!(exhausted["total"],2);
    assert_eq!(exhausted["items"],json!([]));
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM workspace_release_instances",[],|row|row.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|row|row.get::<_,i64>(0)).unwrap(),costs_before);
}

#[test]
fn s1_book_filter_preserves_visibility_versions_archives_and_tier_access() {
    let f = fixture();
    publish(&f,book_filter_publication(&f,"visible-course",0,&[&f.x]));
    let mut newer = book_filter_publication(&f,"visible-course",1,&[&f.x]);
    newer["visible_to_users"] = json!(["B"]);
    publish(&f,newer);
    let mut hidden = book_filter_publication(&f,"hidden-course",0,&[&f.x]);
    hidden["visible_to_users"] = json!(["B"]);
    publish(&f,hidden);
    let mut licensed = book_filter_publication(&f,"licensed-course",0,&[&f.x]);
    licensed["visible_to_users"] = json!([]);
    licensed["allowed_tiers"] = json!(["basic"]);
    publish(&f,licensed);
    let path = format!("/api/workspace-releases?book_id={}",f.x.book_id);
    let visible = f.ok(&f.a,"GET",&path,Value::Null);
    assert_eq!(visible["total"],1);
    assert_eq!(visible["items"][0]["release_id"],"visible-course");
    assert_eq!(visible["items"][0]["version"],1);
    assert!(!visible.to_string().contains("hidden-course"));
    assert!(!visible.to_string().contains("licensed-course"));
    let operator = f.ok(&f.b,"GET",&path,Value::Null);
    assert_eq!(operator["total"],3,"explicit readers see both versions and the separately visible release");
    f.ok(&f.b,"POST","/api/admin/workspace-releases/visible-course/archive",json!({"operation_id":"archive-book-filter","archived":true}));
    let archived = f.ok(&f.a,"GET",&path,Value::Null);
    assert_eq!(archived["total"],1);
    assert_eq!(archived["items"][0]["archived"],true);
    super::cloud_course_e2e_tests::configure_product(&f);
    super::cloud_course_e2e_tests::purchase(&f,"A");
    let licensed = f.ok(&f.a,"GET",&path,Value::Null);
    assert_eq!(licensed["total"],2);
    assert!(licensed["items"].as_array().unwrap().iter().any(|item|item["release_id"]=="licensed-course"));
    assert!(!licensed.to_string().contains("hidden-course"));
}

#[test]
fn s1_book_filter_public_views_project_only_course_overview_and_admin_keeps_lessons() {
    let f = fixture();
    let input = book_filter_publication(&f,"overview-course",0,&[&f.x,&f.y]);
    let course:memory::course::InquiryCourse = serde_json::from_value(input["manifest"]["inquiry_course"].clone()).unwrap();
    let overview = json!(course.overview());
    publish(&f,input);
    let detail_path = "/api/workspace-releases/overview-course/versions/1";
    let list = f.ok(&f.a,"GET",&format!("/api/workspace-releases?book_id={}",f.x.book_id),Value::Null);
    let detail = f.ok(&f.a,"GET",detail_path,Value::Null);
    let export = f.ok(&f.a,"GET",&format!("{detail_path}/export"),Value::Null);
    for view in [&list["items"][0],&detail,&export] {
        assert_eq!(view["manifest"]["inquiry_course"],overview);
        for lesson in view["manifest"]["inquiry_course"]["lessons"].as_array().unwrap() {
            for key in ["summary","introduction","prediction","experiment","explanation","task","hint","assessment","solution"] {
                assert!(lesson.get(key).is_none(),"public course overview exposes {key}");
            }
        }
    }
    let admin = f.ok(&f.b,"GET","/api/admin/workspace-releases/overview-course/versions/1",Value::Null);
    let admin_list = f.ok(&f.b,"GET","/api/admin/workspace-releases",Value::Null);
    assert_eq!(admin["manifest"]["inquiry_course"],json!(course));
    assert_eq!(admin_list["items"][0]["manifest"]["inquiry_course"],json!(course));
}

#[test]
fn s1_publishes_two_exact_ready_materials_without_private_state_or_new_costs() {
    let f = fixture();
    let private = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"private-goal-not-for-release"},"members":[f.x,f.y]}));
    let costs_before:i64 = f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get(0)).unwrap();
    let input = publication(&f,"s1-publish",0);
    assert_eq!(f.call(&f.a,"POST","/api/admin/workspace-releases",input.clone()).0,403);
    let result = publish(&f,input);
    assert_eq!(result["workspace_release"]["readiness_at_publication"]["status"],"ready");
    let visible = detail(&f,&f.a,1);
    assert_eq!(visible["availability"]["status"],"ready");
    assert_eq!(visible["manifest"]["materials"][0]["published_book_ref"],json!(f.x));
    assert_eq!(visible["manifest"]["materials"][1]["published_book_ref"],json!(f.y));
    assert!(visible.get("visible_to_users").is_none());
    assert_eq!(visible["availability"]["audience"].as_array().unwrap().len(),1);
    assert_eq!(visible["availability"]["audience"][0]["user_id"],"A");
    let exported = f.ok(&f.b,"GET","/api/admin/workspace-releases/exam-course/versions/1/export",Value::Null);
    assert_eq!(exported.as_object().unwrap().len(),4);
    assert_eq!(exported["manifest"],result["workspace_release"]["manifest"]);
    for private_text in ["private-goal-not-for-release",private["learning_workspace_id"].as_str().unwrap(),"input_directory","visible_to_users","BuildIntent","rate_snapshot"] {
        assert!(!exported.to_string().contains(private_text),"export contains {private_text}");
    }
    let costs_after:i64 = f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get(0)).unwrap();
    assert_eq!(costs_before,costs_after);
    assert_eq!(f.ok(&f.a,"GET","/api/workspace-releases",Value::Null)["total"],1);
}

#[test]
fn s1_incomplete_materials_capabilities_and_grants_never_appear_ready() {
    let f = fixture();
    f.access.library.lock().unwrap().revoke("A",&f.y).unwrap();
    let mut input = publication(&f,"s1-incomplete",0);
    input["manifest"]["materials"][0]["required_capabilities"] = json!(["text","unavailable-video-transcript"]);
    input["manifest"]["materials"].as_array_mut().unwrap().push(json!({"published_book_ref":{"book_id":"missing-source","publication_id":"missing-version"},"required_capabilities":["text"]}));
    let release = &publish(&f,input)["workspace_release"];
    assert_eq!(release["availability"]["status"],"incomplete");
    assert_eq!(release["availability"]["materials"][0]["missing_capabilities"],json!(["unavailable-video-transcript"]));
    assert_eq!(release["availability"]["materials"][2]["publication_available"],false);
    assert!(release["availability"]["audience"][0]["missing_grants"].as_array().unwrap().contains(&json!(f.y)));
    assert_eq!(detail(&f,&f.a,1)["availability"]["status"],"incomplete");
    assert_ne!(f.call(&f.a,"GET",&f.y.url("text"),json!({})).0,200,"release visibility never grants material access");
}

#[test]
fn s1_new_versions_keep_old_compositions_and_operation_receipts_immutable() {
    let f = fixture();
    let initial = publication(&f,"s1-first",0);
    let saved = publish(&f,initial.clone());
    let old = detail(&f,&f.a,1);
    let mut updated = publication(&f,"s1-second",1);
    updated["manifest"]["public_goal"] = json!("新版复习目标");
    updated["manifest"]["suggested_route"][0]["description"] = json!("新路线先应用后概念");
    updated["visible_to_users"] = json!(["B"]);
    publish(&f,updated);
    assert_eq!(detail(&f,&f.a,1)["manifest"],old["manifest"]);
    assert_eq!(detail(&f,&f.a,1)["readiness_at_publication"],old["readiness_at_publication"]);
    assert_eq!(detail(&f,&f.a,1)["latest_version"],2);
    assert_eq!(publish(&f,initial),saved,"retries retain original receipt even after a newer version");
    assert_eq!(f.ok(&f.b,"GET","/api/admin/operations/s1-first",Value::Null),saved);
    let new = detail(&f,&f.b,2);
    assert_eq!(new["manifest"]["public_goal"],"新版复习目标");
    assert_eq!(f.call(&f.a,"GET","/api/workspace-releases/exam-course/versions/2",Value::Null).0,404);
    assert_eq!(f.call(&f.a,"GET","/api/workspace-releases/exam-course/versions/2/export",Value::Null).0,404);
    assert_eq!(f.ok(&f.a,"GET","/api/workspace-releases",Value::Null)["total"],1);
    assert_eq!(f.ok(&f.b,"GET","/api/admin/workspace-releases?limit=1&offset=1",Value::Null)["items"][0]["version"],1);
    assert_eq!(f.call(&f.b,"POST","/api/admin/workspace-releases",publication(&f,"s1-stale",1)).1["error_code"],"WORKSPACE_RELEASE_VERSION_CONFLICT");
    let mut changed = publication(&f,"s1-first",0); changed["manifest"]["public_goal"] = json!("changed replay");
    assert_eq!(f.call(&f.b,"POST","/api/admin/workspace-releases",changed).1["error_code"],"ADMIN_OPERATION_CONFLICT");
    let count:i64 = f.control.connection.query_row("SELECT count(*) FROM workspace_releases",[],|r|r.get(0)).unwrap();
    assert_eq!(count,2);
}

#[test]
fn s1_current_material_access_is_separate_from_the_publication_snapshot() {
    let f = fixture();
    publish(&f,publication(&f,"s1-ready",0));
    f.access.library.lock().unwrap().revoke("A",&f.y).unwrap();
    let after = detail(&f,&f.a,1);
    assert_eq!(after["readiness_at_publication"]["status"],"ready");
    assert_eq!(after["availability"]["status"],"incomplete");
    assert_eq!(detail(&f,&f.b,1)["availability"]["status"],"ready");
    f.access.library.lock().unwrap().grant("A",&f.y).unwrap();
    assert_eq!(detail(&f,&f.a,1)["availability"]["status"],"ready");
}

#[test]
fn s1_public_manifest_rejects_private_payloads_and_outside_route_materials() {
    let f = fixture();
    let mut private = publication(&f,"s1-private",0);
    private["manifest"]["private_build_intent"] = json!({"learning_workspace_id":"private","goal":"private"});
    assert_eq!(f.call(&f.b,"POST","/api/admin/workspace-releases",private).0,400);
    let mut outside = publication(&f,"s1-outside",0);
    outside["manifest"]["topic_links"][0]["materials"] = json!([{"book_id":"not-selected","publication_id":"old"}]);
    assert_eq!(f.call(&f.b,"POST","/api/admin/workspace-releases",outside).1["error_code"],"WORKSPACE_RELEASE_INVALID");
    let mut unknown = publication(&f,"s1-unknown-user",0); unknown["visible_to_users"] = json!(["not-a-user"]);
    assert_eq!(f.call(&f.b,"POST","/api/admin/workspace-releases",unknown).1["error_code"],"WORKSPACE_RELEASE_READER_UNAVAILABLE");
    assert_eq!(f.ok(&f.b,"GET","/api/admin/workspace-releases",Value::Null)["total"],0);
}

#[test]
fn s1_schema16_upgrade_retains_ledger_publications_and_reopens_immutable_releases() {
    let f = fixture();
    f.ok(&f.b,"POST","/api/admin/build-tasks/s1-existing-cost/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"existing platform task"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/s1-existing-cost/admit",json!({"execution_id":"s1-original-execution","call_id":"s1-original-call","logical_call_id":"s1-original-logical","attempt":1,"granularity":"execution_total","reserved_micro_cny":20,"provider":"fixture","model":"fixture","purpose":"prebuild","budget_enforced":true,"receipt_capability":"complete"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/s1-existing-cost/receipts",json!({"call_id":"s1-original-call","status":"confirmed","provider_cost_micro_cny":12,"evidence":{"fixture":"confirmed"},"succeeded":true}));
    let publications:Vec<(String,String)> = f.control.connection.prepare("SELECT publication_id,manifest FROM book_publications ORDER BY publication_id").unwrap().query_map([],|row|Ok((row.get(0)?,row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
    let budget_before = f.ok(&f.b,"GET","/api/admin/build-tasks/s1-existing-cost",Value::Null);
    f.control.connection.execute_batch(include_str!("remove_schema19.sql")).unwrap();
    f.control.connection.execute_batch("DROP TABLE workspace_release_instance_private_materials; DROP TABLE workspace_release_instance_operations; DROP TABLE workspace_release_instance_history; DROP TABLE workspace_release_instances; DROP TABLE workspace_release_archives; DROP TABLE operator_builds; DROP TABLE workspace_releases; PRAGMA user_version=16; CREATE TABLE workspace_releases(original TEXT); INSERT INTO workspace_releases VALUES('keep');").unwrap();
    assert!(crate::control_store::ControlStore::open(f.control.writer.clone()).is_err());
    assert_eq!(f.control.connection.pragma_query_value(None,"user_version",|row|row.get::<_,i64>(0)).unwrap(),16);
    assert_eq!(f.control.connection.query_row("SELECT original FROM workspace_releases",[],|row|row.get::<_,String>(0)).unwrap(),"keep");
    f.control.connection.execute_batch("DROP TABLE workspace_releases").unwrap();
    let reopened = crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap();
    assert_eq!(reopened.connection.pragma_query_value(None,"user_version",|row|row.get::<_,i64>(0)).unwrap(),crate::control_store::CONTROL_SCHEMA_VERSION);
    let after:Vec<(String,String)> = reopened.connection.prepare("SELECT publication_id,manifest FROM book_publications ORDER BY publication_id").unwrap().query_map([],|row|Ok((row.get(0)?,row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(after,publications);
    assert_eq!(f.ok(&f.b,"GET","/api/admin/build-tasks/s1-existing-cost",Value::Null),budget_before);
    assert_eq!(reopened.connection.query_row("SELECT provider_cost_micro_cny FROM model_call_charges WHERE call_id='s1-original-call'",[],|row|row.get::<_,i64>(0)).unwrap(),12);
    let release = publish(&f,publication(&f,"s1-after-upgrade",0));
    let reopened = crate::admin_store::AdminStore::new(crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(reopened.workspace_release(None,"exam-course",1,false).unwrap(),release["workspace_release"]);
    assert_eq!(reopened.operation("B","s1-after-upgrade").unwrap(),release);
}

#[test]
fn s1_missing_optional_asset_does_not_block_text_but_missing_tutor_product_does() {
    let f = fixture();
    // The supported runtime has independent text and Tutor capabilities.
    let directory = f.access.library.lock().unwrap().load("A",&f.x).unwrap().directory().to_path_buf();
    let mut manifest:Value = serde_json::from_str(&f.control.connection.query_row("SELECT manifest FROM book_publications WHERE book_id=? AND publication_id=?",rusqlite::params![f.x.book_id,f.x.publication_id],|row|row.get::<_,String>(0)).unwrap()).unwrap();
    manifest["files"]["assets/unavailable-optional-image.png"] = json!(10);
    f.control.connection.execute("UPDATE book_publications SET manifest=? WHERE book_id=? AND publication_id=?",rusqlite::params![manifest.to_string(),f.x.book_id,f.x.publication_id]).unwrap();
    publish(&f,publication(&f,"s1-with-optional-image",0));
    assert_eq!(detail(&f,&f.a,1)["availability"]["status"],"ready");
    let original = directory.join("book_structure.json");
    let removed = directory.join("book_structure.unavailable.json");
    std::fs::rename(&original,&removed).unwrap();
    let after = detail(&f,&f.a,1);
    assert_eq!(after["availability"]["materials"][0]["missing_capabilities"],json!(["tutor"]));
    assert!(after["availability"]["materials"][0]["available_capabilities"].as_array().unwrap().contains(&json!("text")));
    std::fs::rename(removed,&original).unwrap();
    let bytes = std::fs::read(&original).unwrap();
    let permissions = std::fs::metadata(&original).unwrap().permissions();
    let mut writable = permissions.clone(); writable.set_readonly(false);
    std::fs::set_permissions(&original,writable).unwrap();
    std::fs::write(&original,b"{}").unwrap();
    assert_eq!(detail(&f,&f.a,1)["availability"]["materials"][0]["missing_capabilities"],json!(["tutor"]),"an existing but invalid product cannot inherit a ready snapshot");
    publish(&f,publication(&f,"s1-invalid-product-snapshot",1));
    assert_eq!(detail(&f,&f.a,2)["readiness_at_publication"]["status"],"incomplete");
    std::fs::write(&original,bytes).unwrap();
    std::fs::set_permissions(&original,permissions).unwrap();
    // A previously saved preparing verdict is historical; current admission follows the products.
    manifest["tutor_readiness"]["status"] = json!("preparing");
    f.control.connection.execute("UPDATE book_publications SET manifest=? WHERE book_id=? AND publication_id=?",rusqlite::params![manifest.to_string(),f.x.book_id,f.x.publication_id]).unwrap();
    let restored = detail(&f,&f.a,2);
    assert_eq!(restored["availability"]["status"],"ready");
    assert_eq!(restored["readiness_at_publication"]["status"],"incomplete");
}
