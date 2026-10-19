//! Offline migration, backup and recovery. Never changes an HTTP/systemd entry point.
use server::reader_maintenance::*;
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        match args.first().map(String::as_str) {
            Some("video-readers") if args.len() >= 4 => {
                server::video_media::set_readers(Path::new(&args[1]), &base_schema::VideoSourceRef { media_id: args[2].clone(), media_revision: args[3].clone() }, &args[4..])
                    .map(|_|serde_json::json!({"ok":true})).map_err(|e|format!("{}: {}",e.error_code,e.message).into())
            }
            Some("register-video") if args.len() >= 4 => server::video_media::register(Path::new(&args[1]), Path::new(&args[2]), &args[3..])
                .map_err(|e| format!("{}: {}", e.error_code, e.message).into()),
            Some("preview" | "migrate") if args.len() >= 2 => {
                let plan: MigrationPlan = serde_json::from_slice(&std::fs::read(&args[1])?)?;
                match (args[0].as_str(), args.len(), args.get(2).map(String::as_str)) {
                    ("preview", 2, _) => migration_preview(&plan),
                    ("migrate", 3, Some("--stopped")) => migrate(&plan, true),
                    _ => Err("migrate requires exactly <plan.json> --stopped".into()),
                }
            },
            Some("backup") if args.len()==3 => backup_service(Path::new(&args[1]), Path::new(&args[2])),
            Some("restore") if args.len()==3 => restore_service(Path::new(&args[1]), Path::new(&args[2])),
            Some("export-user") if args.len()==4 => export_user(Path::new(&args[1]), &args[2], Path::new(&args[3])),
            _ => Err("usage: reader_maintenance register-video <service-root> <workspace> <user-id>... | video-readers <service-root> <media-id> <revision> [user-id...] | preview <plan.json> | migrate <plan.json> --stopped | backup <service-root> <new-snapshot> | restore <snapshot> <new-root> | export-user <service-root> <user-id> <new-root>".into()),
        }
    })();
    match result {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
