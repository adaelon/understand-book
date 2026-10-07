//! Trusted offline publication tool. Never exposed as a reader HTTP endpoint.
use server::{
    control_store::{ControlStore, ServiceWriter},
    published_library::{PublishedBookRef, PublishedLibrary},
};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = (|| -> Result<serde_json::Value, String> {
        if args.len() < 3 {
            return Err("usage: publish_book <absolute-service-root> import <book-directory> | default <book-id> <publication-id> | grant|revoke <user-id> <book-id> <publication-id>".into());
        }
        let writer = ServiceWriter::acquire(Path::new(&args[0])).map_err(|e| e.message)?;
        let mut library = PublishedLibrary::new(ControlStore::open(writer).map_err(|e| e.message)?);
        match args[1].as_str() {
            "import" if args.len() == 3 => library
                .publish(Path::new(&args[2]))
                .map(|m| serde_json::json!(m))
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
