use super::mu5_tests::Fixture;
use super::*;
use runtime::run_context::{CancellationToken, ResidentStatePort};

#[test]
fn mu7_missing_executor_rejects_all_author_operations_and_http_aliases() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "page");
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let mut port = f.run(&w, "page");
    assert_eq!(
        f.ok(&f.a, "GET", "/api/auth/me", json!({}))["capabilities"]["presentation"]["authoring"],
        false
    );
    for input in [
        json!({"operation":"render_plot","code":"raise Exception('must not execute')"}),
        json!({"operation":"render_animation","code":"raise Exception('must not execute')"}),
        json!({"operation":"preview","candidate_id":"x"}),
        json!({"operation":"write","title":"x","html":"<p>x</p>","readable_content":"x"}),
        json!({"operation":"read","candidate_id":"x"}),
        json!({"operation":"search","candidate_id":"x","query":"x"}),
        json!({"operation":"patch","candidate_id":"x","edits":[]}),
        json!({"operation":"deliver","candidate_id":"x"}),
    ] {
        let result = port.author_presentation(
            serde_json::from_value(input.clone()).unwrap(),
            &[],
            &[],
            &CancellationToken::default(),
        );
        assert_eq!(
            result.err().unwrap().error_code,
            "PRESENTATION_SANDBOX_UNAVAILABLE",
            "{input}"
        );
        for path in [
            "/agent/presentation.author",
            "/api/agent/presentation.author",
            "/api/me/presentation.author",
        ] {
            assert_eq!(
                f.call(&f.a, "POST", path, input.clone()).1["error_code"],
                "CAPABILITY_FORBIDDEN"
            );
        }
    }
    assert_eq!(
        f.get(&f.b, &f.create(&f.b, &f.y, "other"))["published_book_ref"],
        json!(f.y)
    );
}

#[test]
fn mu7_invalid_sandbox_configuration_stays_disabled() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("presentation-sandbox.json"), "{bad").unwrap();
    let sandbox = crate::presentation_sandbox::Sandbox::load(root.path());
    assert_eq!(sandbox.capability()["reason"], "invalid_configuration");
    assert!(sandbox.require().is_err());
}

#[test]
fn mu7_generated_outputs_reject_escape_size_type_and_missing_files() {
    use crate::presentation_sandbox::output_file;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("result"), b"1234").unwrap();
    assert_eq!(output_file(root.path(), "result", 4).unwrap(), b"1234");
    for name in ["../result", "/result", "sub/result", "..", "missing"] {
        assert!(output_file(root.path(), name, 4).is_err());
    }
    assert!(output_file(root.path(), "result", 3).is_err());
    std::fs::create_dir(root.path().join("directory")).unwrap();
    assert!(output_file(root.path(), "directory", 4).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.path().join("result"), root.path().join("link")).unwrap();
        assert!(output_file(root.path(), "link", 4).is_err());
    }
    assert!(crate::presentation_sandbox::png(b"not an image", 800, 480).is_err());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "Linux restricted renderer integration with MU7_SANDBOX_ROOT"]
fn mu7_linux_network_author_renders_previews_and_saves_only_owner_version() {
    use runtime::presentation::{PresentationOwner, PresentationRef};
    use runtime::presentation_preview::REQUIRED_PREVIEW_ENVIRONMENTS;
    let root = std::path::PathBuf::from(std::env::var("MU7_SANDBOX_ROOT").unwrap());
    let f = Fixture::configured(Some(&root.join("presentation-sandbox.json")));
    assert_eq!(
        f.ok(&f.a, "GET", "/api/auth/me", json!({}))["capabilities"]["presentation"]["authoring"],
        true
    );
    let w = f.create(&f.a, &f.x, "author");
    let w = f.action(&f.a, &w, "author", "chat/new", json!({}));
    let mut port = f.run(&w, "author");
    let cancel = CancellationToken::default();
    let mut author = |input: Value| {
        port.author_presentation(serde_json::from_value(input).unwrap(), &[], &[], &cancel)
            .unwrap()
    };
    let plot = author(
        json!({"operation":"render_plot","code":"ax.plot([0,1],[0,1])","size":{"width":400,"height":300}}),
    );
    assert_eq!(plot.images.len(), 1);
    let html = format!(
        "<p>Sandbox preview</p><img alt='Plot' style='max-width:100%' src='{}'>",
        plot.body["asset_path"].as_str().unwrap()
    );
    let candidate = author(json!({"operation":"write","title":"Sandbox preview","html":html,"readable_content":"Sandbox preview","asset_refs":[plot.body["asset_ref"]]})).body["candidate_id"].clone();
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let preview =
            author(json!({"operation":"preview","candidate_id":candidate,"viewport":viewport}));
        assert_eq!(preview.body["errors"], json!([]), "{}", preview.body);
        assert!(!preview.images.is_empty());
    }
    let delivered = author(json!({"operation":"deliver","candidate_id":candidate}));
    let reference: PresentationRef =
        serde_json::from_value(delivered.body["reference"].clone()).unwrap();
    let owner = PresentationOwner {
        book_id: f.x.book_id.clone(),
        session_id: w["selected_chat"].as_str().unwrap().into(),
    };
    for (id, expected) in [("A", true), ("B", false)] {
        let handle = f.access.users.lock().unwrap().get(id, "fixture").unwrap();
        let user = handle.lock().unwrap();
        let store = crate::presentation_store::PresentationStore::for_user(&user).unwrap();
        let version = store.read_version(&owner, &reference);
        assert_eq!(version.is_ok(), expected);
        if let Ok(version) = version {
            assert!(version.content.content_files.contains_key(&format!(
                "assets/{}.svg",
                plot.body["asset_ref"].as_str().unwrap()
            )));
        }
    }
}
