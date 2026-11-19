//! Cloud registration and account-scoped discovery of preparation media.
pub use reader_service::video_media::*;
use base_schema::{VideoImportStage, VideoSourceRef};
use read_tools::ToolError;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{fs, path::{Path, PathBuf}};

// Offline registration uses the service writer and existing users. The registry contains portable
// snapshots and explicit per-user grants, without manufacturing a published Book.
pub fn register(root: &Path, workspace: &Path, owners: &[String]) -> Result<Value, ToolError> {
    let control = crate::control_store::ControlStore::open(
        crate::control_store::ServiceWriter::acquire(root)?,
    )?;
    for owner in owners {
        control.user_paths(owner)?;
    }
    if owners.is_empty() {
        return Err(invalid("At least one reader is required"));
    }
    let state = read_import(workspace)?;
    if state.stage != VideoImportStage::AwaitingTranscript {
        return Err(invalid("Complete video import before registration"));
    }
    media_path(workspace)?;
    let parent = root.join("library/videos").join(&state.video.media_id);
    fs::create_dir_all(&parent).map_err(|_| invalid("Cannot create video registry"))?;
    let target = parent.join(&state.video.media_revision);
    if target.exists() {
        return Err(invalid(
            "Video already registered; update its grants explicitly",
        ));
    }
    let staging = tempfile::tempdir_in(&parent).map_err(|_| invalid("Cannot stage video"))?;
    fs::create_dir(staging.path().join("media")).map_err(|_| invalid("Cannot stage media"))?;
    for name in [
        "video_import.json",
        "media/original.mp4",
        "media/probe.json",
    ] {
        fs::copy(workspace.join(name), staging.path().join(name))
            .map_err(|_| invalid("Cannot copy imported media"))?;
    }
    fs::write(
        staging.path().join("readers.json"),
        serde_json::to_vec(owners).unwrap(),
    )
    .map_err(|_| invalid("Cannot save grants"))?;
    fs::rename(staging.path(), &target).map_err(|_| invalid("Cannot register media"))?;
    projection(&target)
}
pub fn set_readers(
    root: &Path,
    video: &VideoSourceRef,
    owners: &[String],
) -> Result<(), ToolError> {
    let control = crate::control_store::ControlStore::open(
        crate::control_store::ServiceWriter::acquire(root)?,
    )?;
    if !safe_segment(&video.media_id) || !safe_segment(&video.media_revision) {
        return Err(invalid("Invalid video identity"));
    }
    for owner in owners {
        control.user_paths(owner)?;
    }
    let dir = root
        .join("library/videos")
        .join(&video.media_id)
        .join(&video.media_revision);
    if read_import(&dir)?.video != *video {
        return Err(invalid("Video identity mismatch"));
    }
    fs::write(
        dir.join("readers.json"),
        serde_json::to_vec(owners).unwrap(),
    )
    .map_err(|_| invalid("Cannot save readers"))
}
pub(crate) fn authorized_dir(
    root: &Path,
    owner: &str,
    video: &VideoSourceRef,
) -> Result<PathBuf, ToolError> {
    if !safe_segment(&video.media_id) || !safe_segment(&video.media_revision) {
        return Err(crate::service_error::missing());
    }
    let dir = root
        .join("library/videos")
        .join(&video.media_id)
        .join(&video.media_revision);
    let owners: Vec<String> = fs::read(dir.join("readers.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or_else(crate::service_error::missing)?;
    if !owners.iter().any(|o| o == owner) || read_import(&dir)?.video != *video {
        return Err(crate::service_error::missing());
    }
    Ok(dir)
}
pub(crate) fn catalog(root: &Path, owner: &str) -> Vec<Value> {
    let mut result = vec![];
    if let Ok(ids) = fs::read_dir(root.join("library/videos")) {
        for id in ids.flatten() {
            if let Ok(revisions) = fs::read_dir(id.path()) {
                for revision in revisions.flatten() {
                    let video = VideoSourceRef {
                        media_id: id.file_name().to_string_lossy().into(),
                        media_revision: revision.file_name().to_string_lossy().into(),
                    };
                    if let Ok(dir) = authorized_dir(root, owner, &video) {
                        if let Ok(value) = projection(&dir) {
                            result.push(value);
                        }
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{authorization::Authorization, control_store::{ControlStore, ServiceWriter},
        multi_user_host::{self, Headers, Site}, user_registry::UserRegistry};
    use std::sync::Arc;
    fn fixture(dir: &Path) {
        fs::create_dir_all(dir.join("media")).unwrap();
        // Transport fixture bytes, not a playable video; actual decode is covered by Core and browser tests.
        fs::write(dir.join("media/original.mp4"), b"0123456789").unwrap();
        fs::write(dir.join("media/probe.json"), "{}").unwrap();
        fs::write(dir.join("video_import.json"), json!({"version":"video_import.v1","video":{"media_id":"lecture","media_revision":"1"},"title":"Lecture","original_path":"gone.mp4","stage":"awaiting_transcript","copied_bytes":10,"total_bytes":10,"source":{"video":{"media_id":"lecture","media_revision":"1"},"imported_path":"media/original.mp4","original_path":"gone.mp4","duration_ms":1000,"width":160,"height":90},"failure":null}).to_string()).unwrap();
    }
    #[test]
    fn video_registration_is_portable_and_authorized_before_status_media_head_and_ask() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input");
        fixture(&input);
        let service = root.path().join("service");
        {
            let mut control =
                ControlStore::open(ServiceWriter::acquire(&service).unwrap()).unwrap();
            for owner in ["A", "B"] {
                control
                    .provision_password(owner, "fixture-only-password", true)
                    .unwrap();
            }
        }
        register(&service, &input, &["A".into()]).unwrap();
        assert!(register(&service, &input, &["B".into()]).is_err());
        let reference = VideoSourceRef { media_id: "lecture".into(), media_revision: "1".into() };
        set_readers(&service, &reference, &["B".into()]).unwrap();
        assert!(authorized_dir(&service, "A", &reference).is_err());
        assert!(authorized_dir(&service, "B", &reference).is_ok());
        assert!(set_readers(&service, &reference, &["missing-user".into()]).is_err());
        set_readers(&service, &reference, &["A".into()]).unwrap();
        let moved = root.path().join("moved-service");
        fs::rename(&service, &moved).unwrap();
        let access = Arc::new(Authorization::new(UserRegistry::open(&moved).unwrap()).unwrap());
        let token_a = access
            .auth
            .login("A", "fixture-only-password", None, multi_user_host::now())
            .unwrap();
        let token_b = access
            .auth
            .login("B", "fixture-only-password", None, multi_user_host::now())
            .unwrap();
        let call = |token: Option<&str>, method: &str, path: &str| {
            let mut headers = vec![
                ("Host".into(), "reader.example".into()),
                ("Origin".into(), "https://reader.example".into()),
                ("Range".into(), "bytes=2-5".into()),
            ];
            if let Some(token) = token {
                headers.push(("Cookie".into(), format!("{}={token}", crate::auth::COOKIE)));
                headers.push(("X-CSRF-Token".into(), crate::auth::csrf(token)));
            }
            multi_user_host::dispatch(
                &access,
                &Site::new("https://reader.example").unwrap(),
                method,
                path,
                &Headers(headers),
                "{}",
                multi_user_host::now(),
            )
        };
        for (method, leaf) in [
            ("GET", "media"),
            ("HEAD", "media"),
            ("GET", "status"),
            ("POST", "ask"),
        ] {
            let path = format!("/api/videos/lecture/1/{leaf}");
            assert_eq!(call(None, method, &path).status, 401);
            assert_eq!(call(Some(&token_b), method, &path).status, 404);
            assert_eq!(
                call(Some(&token_a), method, &path).status,
                if leaf == "ask" { 409 } else { 200 }
            );
        }
        let list = call(Some(&token_a), "GET", "/api/videos");
        let value: Value = serde_json::from_slice(&list.body).unwrap();
        assert_eq!(value["videos"].as_array().unwrap().len(), 1);
        assert_eq!(value["videos"][0]["qa_ready"], false);
        assert!(!String::from_utf8(list.body).unwrap().contains("gone.mp4"));
        assert_eq!(
            serde_json::from_slice::<Value>(&call(Some(&token_b), "GET", "/api/videos").body)
                .unwrap()["videos"],
            json!([])
        );
        // Exercise the actual multi-user HTTP responder, including tiny_http's HEAD suppression.
        let host = multi_user_host::start_with_access(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(Site::new("https://reader.example").unwrap()),
            access.clone(),
        )
        .unwrap();
        let url = format!("{}/api/videos/lecture/1/media", host.url);
        let response = ureq::get(&url)
            .set("Host", "reader.example")
            .set("Cookie", &format!("{}={token_a}", crate::auth::COOKIE))
            .set("Range", "bytes=2-5")
            .call()
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.header("Content-Range"), Some("bytes 2-5/10"));
        assert_eq!(response.into_string().unwrap(), "2345");
        let response = ureq::head(&url)
            .set("Host", "reader.example")
            .set("Cookie", &format!("{}={token_a}", crate::auth::COOKIE))
            .call()
            .unwrap();
        assert_eq!(response.header("Content-Length"), Some("10"));
        assert_eq!(response.into_string().unwrap(), "");
        host.shutdown();
        fs::write(moved.join("library/videos/lecture/1/readers.json"), "[]").unwrap();
        assert_eq!(
            call(Some(&token_a), "HEAD", "/api/videos/lecture/1/media").status,
            404
        );
    }
    #[test]
    fn video_publication_requires_complete_dependencies_and_loads_searchable_reader_without_graph() {
        use crate::published_library::PublishedLibrary;
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("workspace"); fs::create_dir_all(input.join("media")).unwrap();
        let f: Value = serde_json::from_str(include_str!("../../base-schema/fixtures/video-contract.json")).unwrap();
        let case = &f["cases"][0];
        let mut manifest = case["manifest"].clone();
        let video = manifest["source"]["video"].clone();
        manifest["coverage"] = json!([{"capability":"transcript","accepted":[{"video":video,"start_ms":0,"end_ms":manifest["source"]["duration_ms"]}],"pending":[],"failed":[]}]);
        fs::write(input.join("media/original.mp4"),b"0123456789").unwrap();
        fs::write(input.join("media/probe.json"),"{}").unwrap();
        fs::write(input.join("video_import.json"),json!({"version":"video_import.v1","video":video,"title":"Lecture","original_path":"gone.mp4","stage":"awaiting_transcript","copied_bytes":10,"total_bytes":10,"source":manifest["source"],"failure":null}).to_string()).unwrap();
        fs::write(input.join("source.txt"),case["source_text"].as_str().unwrap()).unwrap();
        let mut base=case["base"].clone(); base["graph_nodes"]=json!([]);base["graph_edges"]=json!([]);
        // VI1 only needed a mapped body; searchable Books also classify the title as a leaf.
        base["lid_nodes"][0]["children"]=json!(["1.1","1.2"]);
        base["lid_nodes"][1]["lid"]=json!("1.2");base["lid_nodes"][1]["path"]=json!([1,2]);
        base["lid_nodes"].as_array_mut().unwrap().insert(1,json!({"lid":"1.1","path":[1,1],"kind":"paragraph","span":{"start":0,"end":9},"children":[]}));
        let mut map=case["map"].clone();
        for segment in map["segments"].as_array_mut().unwrap() { segment["text_range"]["lid"]=json!("1.2"); }
        fs::write(input.join("base.json"),base.to_string()).unwrap();
        fs::write(input.join("video_source_manifest.json"),manifest.to_string()).unwrap();
        let map_path=input.join(manifest["transcript_map_path"].as_str().unwrap());
        fs::create_dir_all(map_path.parent().unwrap()).unwrap();fs::write(&map_path,map.to_string()).unwrap();
        let raw_path=input.join(case["map"]["raw_result_path"].as_str().unwrap());
        fs::create_dir_all(raw_path.parent().unwrap()).unwrap();fs::write(&raw_path,"{\"fixture\":true}").unwrap();
        let service=root.path().join("service");
        {
            let mut control=ControlStore::open(ServiceWriter::acquire(&service).unwrap()).unwrap();
            control.provision_password("A","fixture-only-password",true).unwrap();
        }
        register(&service,&input,&["A".into()]).unwrap();
        let mut library=PublishedLibrary::new(ControlStore::open(ServiceWriter::acquire(&service).unwrap()).unwrap());
        fs::rename(&raw_path,raw_path.with_extension("saved")).unwrap();
        assert!(library.publish(&input).is_err());
        assert!(!input.join("video_publication.json").exists());
        fs::rename(raw_path.with_extension("saved"),&raw_path).unwrap();
        let mut partial=manifest.clone();partial["coverage"][0]["accepted"][0]["end_ms"]=json!(600000);
        fs::write(input.join("video_source_manifest.json"),partial.to_string()).unwrap();
        assert!(library.publish(&input).is_err());
        fs::write(input.join("video_source_manifest.json"),manifest.to_string()).unwrap();
        let published=library.publish(&input).unwrap();
        assert_eq!(published.build_readiness["status"],"transcript_ready");
        assert_eq!(published.build_readiness["semantic"],"pending");
        assert!(published.files.contains_key("media/original.mp4"));
        let loaded=library.load("A",&published.reference).unwrap();
        assert!(loaded.book.text("1.2",None).unwrap().contains("underflow"));
        assert!(loaded.book.base.graph_nodes.is_empty());
        assert_eq!(projection(&input).unwrap()["english_ready"],true);
        assert_eq!(transcript(&input).unwrap()["publication"],json!(published.reference));
        assert!(library.load("B",&published.reference).is_err());
        library.revoke("A",&published.reference).unwrap();
        assert!(library.load("A",&published.reference).is_err());
        // Actual video attachment changes require a new Book even if text and IDs were left unchanged.
        fs::write(input.join("media/original.mp4"),b"different!").unwrap();
        assert_eq!(library.publish(&input).unwrap_err().error_code,"BOOK_CONTENT_CHANGED");
        drop(loaded);drop(library);
        let moved=root.path().join("moved");fs::rename(&input,&moved).unwrap();
        // Sibling publication remains reachable after the workspace directory is renamed.
        assert_eq!(transcript(&moved).unwrap()["publication"],json!(published.reference));
        let access=Arc::new(Authorization::new(UserRegistry::open(&service).unwrap()).unwrap());
        access.library.lock().unwrap().grant("A",&published.reference).unwrap();
        let token=access.auth.login("A","fixture-only-password",None,multi_user_host::now()).unwrap();
        let host=multi_user_host::start_with_access("127.0.0.1:0".parse().unwrap(),
            Arc::new(Site::new("https://reader.example").unwrap()),access.clone()).unwrap();
        let url=format!("{}{}",host.url,published.reference.url("video/media"));
        let response=ureq::get(&url).set("Host","reader.example").set("Cookie",&format!("{}={token}",crate::auth::COOKIE))
            .set("Range","bytes=2-5").call().unwrap();
        assert_eq!(response.status(),206);assert_eq!(response.into_string().unwrap(),"2345");
        let head=ureq::head(&url).set("Host","reader.example").set("Cookie",&format!("{}={token}",crate::auth::COOKIE)).call().unwrap();
        assert_eq!(head.header("Content-Length"),Some("10"));assert_eq!(head.into_string().unwrap(),"");
        let transcript_url=format!("{}{}",host.url,published.reference.url("video_transcript"));
        let response=ureq::get(&transcript_url).set("Host","reader.example").set("Cookie",&format!("{}={token}",crate::auth::COOKIE)).call().unwrap();
        assert!(response.into_string().unwrap().contains("underflow"));
        let search_url=format!("{}{}?query=underflow&match_mode=exact",host.url,published.reference.url("search_text"));
        let response=ureq::get(&search_url).set("Host","reader.example").set("Cookie",&format!("{}={token}",crate::auth::COOKIE)).call().unwrap();
        let found:Value=serde_json::from_str(&response.into_string().unwrap()).unwrap();
        assert!(found["total_occurrences"].as_u64().unwrap()>0);
        access.library.lock().unwrap().revoke("A",&published.reference).unwrap();
        assert!(matches!(ureq::head(&url).set("Host","reader.example").set("Cookie",&format!("{}={token}",crate::auth::COOKIE)).call(),Err(ureq::Error::Status(404,_))));
        host.shutdown();
    }
}
