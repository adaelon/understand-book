use super::*;

#[test]
fn installed_skill_is_scoped_and_changes_future_turns() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("teach-evidence");
    fs::create_dir_all(source.join("references")).unwrap();
    fs::write(source.join("SKILL.md"), "---\r\nname: teach-evidence\r\ndescription: >\r\n  Compare evidence\r\n  side by side.\r\n---\r\nRead the methods and results before asking the learner.").unwrap();
    fs::write(source.join("references/example.md"), "Compare trials").unwrap();
    let a = skill_workspace(root.path(), "book:alpha");
    let b = skill_workspace(root.path(), "book:beta");
    // ACP clients may pass cwd through URL decoding (including on Windows).
    assert!(!a.file_name().unwrap().to_str().unwrap().contains('%'));
    assert_ne!(a, skill_workspace(root.path(), "book~3aalpha"));
    let (staged, _) = skills::stage(&a, source.to_str().unwrap(), "").unwrap();
    assert_eq!(skills::list(&a).unwrap()["skills"], json!([]));
    drop(staged);
    let imported = skills::import(&a, source.to_str().unwrap(), "").unwrap();
    assert_eq!(imported["skills"][0]["enabled"], false);
    assert!(skills::context(&a).unwrap().is_empty());
    skills::change(&a, "teach-evidence", Some(true)).unwrap();
    assert!(skill_context(root.path(), "book:alpha")
        .unwrap()
        .contains("Read the methods and results"));
    assert!(a
        .join(".agents/skills/teach-evidence/references/example.md")
        .exists());
    assert!(skills::context(&b).unwrap().is_empty());
    assert!(skills::import(&a, source.to_str().unwrap(), "").is_err());
    skills::change(&a, "teach-evidence", Some(false)).unwrap();
    assert!(skills::context(&a).unwrap().is_empty());
    assert_eq!(
        skills::change(&a, "teach-evidence", None).unwrap()["skills"],
        json!([])
    );
    assert!(source.join("SKILL.md").exists());
}

#[test]
fn metadata_only_record_reopens_with_its_origin() {
    let root = tempfile::tempdir().unwrap();
    let mut state = crate::tests::state_named("extension-metadata-only");
    state.services.library_root = Some(root.path().join("books"));
    let src = json!({"provider":"agentero","vault":"test-vault","ref":"papers/trial","metadata":{"id":"paper-trial","path":"papers/trial","authors":["A"],"title":"Trial"}});
    let result = import(
        &mut state,
        root.path(),
        Some("trial"),
        Some(src.clone()),
        None,
        "5",
    )
    .unwrap();
    assert_eq!(result["status"], "metadata_only");
    assert_eq!(
        metadata(&state, root.path(), "trial").unwrap()["source"],
        src
    );
    assert!(metadata(&state, root.path(), "unrelated").is_err());
    let wrong = import(
        &mut state,
        root.path(),
        Some("trial"),
        Some(json!({"ref":"other"})),
        None,
        "6",
    );
    assert!(wrong.is_err());
    assert_eq!(
        metadata(&state, root.path(), "trial").unwrap()["source"],
        src
    );
}

#[test]
#[ignore = "requires access to the selected public GitHub Skill"]
fn live_github_skill_lifecycle() {
    let source = std::env::var("UNDERSTAND_BOOK_TEST_GITHUB_SKILL").unwrap();
    let root = tempfile::tempdir().unwrap();
    let workspace = skill_workspace(root.path(), "workspace:community");
    let imports = skills::import(&workspace, &source, "").unwrap();
    let name = imports["skills"][0]["name"].as_str().unwrap();
    assert_eq!(imports["skills"][0]["enabled"], false);
    skills::change(&workspace, name, Some(true)).unwrap();
    let context = skill_context(root.path(), "workspace:community").unwrap();
    assert!(context.contains(name));
    assert!(skill_context(root.path(), "book:other").unwrap().is_empty());
    eprintln!(
        "Community Skill {name} at {source}, {} prompt bytes",
        context.len()
    );
    skills::change(&workspace, name, Some(false)).unwrap();
    assert!(skills::context(&workspace).unwrap().is_empty());
    assert_eq!(
        skills::change(&workspace, name, None).unwrap()["skills"],
        json!([])
    );
}

#[test]
#[ignore = "requires an explicitly selected local Agentero Vault and catalog paper"]
fn live_agentero_paper_import() {
    let reference = std::env::var("UNDERSTAND_BOOK_TEST_AGENTERO_PAPER").unwrap();
    let mut client = agentero::Agentero::connect().unwrap();
    let found = client.tool("paper_list", json!({})).unwrap();
    assert!(found["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["path"] == reference || r["id"] == reference));
    let (origin, pdf) = client.select(Some(&reference), None).unwrap();
    let pdf = pdf.expect("select a catalog paper with a PDF");
    let root = tempfile::tempdir().unwrap();
    let mut state = crate::tests::state_named("extension-live-paper");
    let original = state.workspace.book_dir.clone();
    state.services.library_root = Some(root.path().join("books"));
    let receipt = import(
        &mut state,
        root.path(),
        Some("live-paper"),
        Some(origin.clone()),
        Some(pdf.clone()),
        "1000",
    )
    .unwrap();
    assert_eq!(state.workspace.book_dir, original);
    assert_eq!(
        fs::read(root.path().join("books/live-paper/paper.pdf")).unwrap(),
        fs::read(&pdf).unwrap()
    );
    assert_eq!(
        metadata(&state, root.path(), "live-paper").unwrap()["source"],
        origin
    );
    let local = import(
        &mut state,
        root.path(),
        Some("local-pdf"),
        None,
        Some(pdf.clone()),
        "1001",
    )
    .unwrap();
    assert_eq!(local["readiness"]["route"], receipt["readiness"]["route"]);
    if let Ok(citation) = std::env::var("UNDERSTAND_BOOK_TEST_AGENTERO_CITATION") {
        let (source, absent) = client.select(Some(&citation), None).unwrap();
        assert!(absent.is_none());
        let pending = import(
            &mut state,
            root.path(),
            Some("citation"),
            Some(source.clone()),
            None,
            "1002",
        )
        .unwrap();
        assert_eq!(pending["status"], "metadata_only");
        let attached = import(
            &mut state,
            root.path(),
            Some("citation"),
            None,
            Some(pdf),
            "1003",
        )
        .unwrap();
        assert_eq!(attached["source"], source);
        assert_eq!(
            metadata(&state, root.path(), "citation").unwrap()["source"],
            source
        );
        eprintln!("Attached PDF to metadata-only Agentero record {citation}; preserved its source");
    }
    eprintln!(
        "Agentero title={} authors={} workbench={}",
        origin["metadata"]["title"], origin["metadata"]["authors"], receipt["readiness"]["status"]
    );
}

#[test]
fn catalog_origin_follows_the_imported_directory() {
    let root = tempfile::tempdir().unwrap();
    let imported = root.path().join("imported");
    let other = root.path().join("same-id-other-library");
    fs::create_dir_all(&imported).unwrap();
    fs::create_dir_all(&other).unwrap();
    let state = crate::tests::state_named("catalog-copies");
    let book = &state.workspace.book;
    let source = json!({"provider":"agentero","metadata":{"title":"Only this copy"}});
    save_origin(
        &origin_path(root.path(), &book.base.book_id),
        &source,
        Some(&imported),
    )
    .unwrap();
    assert_eq!(
        prepared_metadata(root.path(), book, &imported).unwrap()["source"],
        source
    );
    assert!(prepared_metadata(root.path(), book, &other).unwrap()["source"].is_null());
}
