//! Trusted publication tool: local service channel or exclusive offline writer.
use server::{
    control_store::{ControlStore, ServiceWriter},
    published_library::{PublishedBookRef, PublishedLibrary},
};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = (|| -> Result<serde_json::Value, String> {
        if args.len() < 3 {
            return Err("usage: publish_book <absolute-service-root> online-import <book-directory> --operation-id <id> --grant <user> [--grant <user> ...] | import <book-directory> | default|grant-all <book-id> <publication-id> | grant|revoke <user-id> <book-id> <publication-id>".into());
        }
        if args[1] == "online-import" {
            let mut operation_id = None;
            let mut grant_users = Vec::new();
            let mut options = args[3..].chunks_exact(2);
            for option in &mut options {
                match option[0].as_str() {
                    "--operation-id" if operation_id.is_none() => operation_id = Some(option[1].clone()),
                    "--grant" => grant_users.push(option[1].clone()),
                    _ => return Err("Expected --operation-id <id> and --grant <user>".into()),
                }
            }
            if !options.remainder().is_empty() { return Err("Missing option value".into()); }
            grant_users.sort();
            let request = server::online_publication::PublicationRequest {
                operation_id: operation_id.ok_or("--operation-id is required; retain it when retrying")?,
                input_directory: std::path::absolute(&args[2]).map_err(|e| e.to_string())?,
                grant_users,
            };
            return server::online_publication::submit(Path::new(&args[0]), &request).map(|m| serde_json::json!(m));
        }
        let writer = ServiceWriter::acquire(Path::new(&args[0])).map_err(|e| e.message)?;
        let mut library = PublishedLibrary::new(ControlStore::open(writer).map_err(|e| e.message)?);
        match args[1].as_str() {
            "import" if args.len() == 3 => library
                .publish(Path::new(&args[2]))
                .map(|m| serde_json::json!(m))
                .map_err(|e| e.message),
            "grant-all" if args.len() == 4 => library
                .grant_all(&PublishedBookRef { book_id: args[2].clone(), publication_id: args[3].clone() })
                .map(|_| serde_json::json!({"ok":true,"existing_and_future_accounts":true}))
                .map_err(|e| e.message),
            "default" if args.len() == 4 => library
                .set_default(&PublishedBookRef {
                    book_id: args[2].clone(),
                    publication_id: args[3].clone(),
                })
                .map(|_| serde_json::json!({"ok":true}))
                .map_err(|e| e.message),
            "revoke" if args.len() == 5 => library
                .revoke(&args[2], &PublishedBookRef { book_id: args[3].clone(), publication_id: args[4].clone() })
                .map(|_| serde_json::json!({"ok":true})).map_err(|e| e.message),
            "grant" if args.len() == 5 => library
                .grant(
                    &args[2],
                    &PublishedBookRef {
                        book_id: args[3].clone(),
                        publication_id: args[4].clone(),
                    },
                )
                .map(|_| serde_json::json!({"ok":true}))
                .map_err(|e| e.message),
            _ => Err("invalid publication command".into()),
        }
    })();
    match result {
        Ok(value) => println!("{value}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
