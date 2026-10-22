use super::mu5_tests::Fixture;
use serde_json::{json, Value};
use std::{sync::Arc, time::{Duration, Instant}};

/// Opt-in local browser fixture. Its products and payments live only in the
/// temporary database; no commercial defaults or production content are seeded.
#[test]
#[ignore = "actual HTTP cloud-learning browser acceptance; stop through 4201/stop"]
fn cloud_learning_browser_host() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let text = super::cloud_course_e2e_tests::prepare_text(&f);
    let (_, video, _) = super::cloud_course_e2e_tests::prepare_video(&f);
    super::cloud_course_e2e_tests::course(&f, &text, &video, 0);
    for (tier, label, multiplier) in [("basic", "基础版", 1), ("plus", "进阶版", 2), ("pro", "专业版", 3)] {
        for (duration, days) in [("day1", "1天"), ("day3", "3天"), ("day7", "7天"), ("month", "月度")] {
            let id = format!("browser-{tier}-{duration}");
            f.ok(&f.b, "POST", "/api/admin/cloud-plans", json!({
                "operation_id":format!("seed-{id}"),"product_id":id,"revision":0,
                "title":format!("{label}{days}（浏览器验收）"),"tier":tier,"duration":duration,
                "enabled":true,"test_only":false,"amount_fen":123 * multiplier,
                "basis":{"version":"browser-fixture-only","basis_micro_cny":1000000},
                "grant_milli_percent":100000 * multiplier,
                "storage_bytes":131072 * multiplier,"max_file_bytes":98304,
                "retention_seconds":86400,"instructions":"隔离验收数据库；测试核款，无真实支付"
            }));
        }
    }
    f.ok(&f.b, "POST", "/api/admin/cloud-plans", json!({
        "operation_id":"seed-browser-draft","product_id":"browser-draft","revision":0,
        "title":"尚未定价草稿（不可出售）","tier":"basic","duration":"day1",
        "enabled":true,"test_only":false,"amount_fen":null,"basis":null,
        "grant_milli_percent":null,"storage_bytes":null,"max_file_bytes":null,
        "retention_seconds":null,"instructions":"等待确定真实商业配置"
    }));
    if std::env::var("CLOUD_EXAM_REAL_PROVIDER").as_deref() == Ok("1") {
        let config = runtime::ProviderConfig::from_env().unwrap();
        // Admission-only fixture prices keep the isolated allowance sufficient.
        // They must never be reported as the upstream provider's actual price.
        std::fs::write(f.root.path().join("model-rates.json"), json!({"rates":[{
            "version":"cloud-browser-admission-fixture","provider":config.base_url,"model":config.model,
            "starts_at":0,"expires_at":i64::MAX,
            "input_micro_cny_per_million":1,"cached_input_micro_cny_per_million":1,"output_micro_cny_per_million":1,
            "usage_contract":"deep_seek","image_meter":"deep_seek1024",
            "source":{"currency":"CNY","original_prices":"fixture-only admission rate; actual supplier cost unknown",
                "cny_conversion":"1","checked_on":"2026-10-20","official_url":"https://example.invalid/cloud-admission-fixture"}
        }]}).to_string()).unwrap();
        Arc::get_mut(&mut f.access).unwrap().spend = Arc::new(crate::model_spend_store::ModelSpendStore::new(
            crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap()
        ));
        f.access.runs.configure(config).unwrap();
    }
    let origin = "https://127.0.0.1:4200";
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:4200".parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new(origin).unwrap()),
        f.access.clone(),
    ).unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4201").unwrap();
    println!("CLOUD_LEARNING_BROWSER_READY");
    let until = Instant::now() + Duration::from_secs(3600);
    while Instant::now() < until {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else { continue };
        if request.url() == "/stop" {
            request.respond(tiny_http::Response::from_string("stopped")).unwrap();
            break;
        }
        let result = match request.url() {
            "/fixture" => json!({
                "origin":origin,"reader_user_id":"A","admin_user_id":"B",
                "reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),
                "admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),
                "reader_csrf":crate::auth::csrf(&f.a),"admin_csrf":crate::auth::csrf(&f.b),
                "text":text,"video":video,"release_id":"cloud-original-fixture",
                "title":"资料引用与时间片段：原创验收样例"
            }),
            "/counts" => {
                let db = &f.control.connection;
                json!({
                    "model_calls":db.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get::<_,i64>(0)).unwrap(),
                    "publications":db.query_row("SELECT count(*) FROM book_publications",[],|r|r.get::<_,i64>(0)).unwrap(),
                    "receipts":db.query_row("SELECT count(*) FROM manual_receipts",[],|r|r.get::<_,i64>(0)).unwrap()
                })
            },
            _ => Value::Null,
        };
        request.respond(tiny_http::Response::from_string(result.to_string()).with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()
        )).unwrap();
    }
    server.shutdown();
}
