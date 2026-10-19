//! Compile one method combination. The same manifest drives discovery and file reads.
use std::{collections::HashSet, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=UNDERSTAND_BOOK_METHOD_CATALOG");
    let repo = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let manifest = env::var_os("UNDERSTAND_BOOK_METHOD_CATALOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("skills/presentation/methods/catalog.json"));
    let manifest = repo.join(manifest);
    println!("cargo:rerun-if-changed={}", manifest.display());
    let catalog: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&manifest).expect("read presentation method catalog"),
    )
    .expect("parse presentation method catalog");
    let mut generated = format!(
        "const CATALOG_JSON: &str = {:?};\nconst FILES: &[(&str, u64, &str, &str)] = &[\n",
        catalog.to_string()
    );
    let mut versions = HashSet::new();
    for method in catalog["methods"].as_array().expect("catalog methods") {
        let id = method["id"].as_str().expect("method id");
        let revision = method["revision"].as_u64().expect("method revision");
        assert!(
            versions.insert((id, revision)),
            "duplicate method {id}@{revision}"
        );
        let root = repo.join(method["root"].as_str().expect("method root"));
        let files = method["files"].as_array().expect("method files");
        assert!(
            files.contains(&serde_json::json!("card.md")),
            "method must register card.md"
        );
        assert!(
            files.contains(&method["entrypoint"]),
            "method must register its entrypoint"
        );
        let mut names = HashSet::new();
        for file in files {
            let name = file.as_str().expect("method file name");
            assert!(names.insert(name), "duplicate method file {name}");
            let path = root.join(name);
            println!("cargo:rerun-if-changed={}", path.display());
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            generated.push_str(&format!("({id:?}, {revision}, {name:?}, {text:?}),\n"));
        }
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("presentation_methods.rs"),
        generated,
    )
    .unwrap();
}
