use crate::{agent_method_not_allowed, AppState, Reply, Req};
use memory::learning::TutorMutation;
use serde_json::json;

pub(crate) fn route(state: &mut AppState, req: Req) -> Reply {
    let result = (|| {
        let mut learning = state.user.learning_store_for_source(state.workspace.learning_workspace_id.as_deref(), &state.workspace.book.base.book_id)?;
        if req.url == "/tutor/state" && req.method == "GET" {
            return learning.state();
        }
        let request: TutorMutation =
            serde_json::from_str(req.body).map_err(|error| read_tools::ToolError {
                error_code: "TUTOR_ACTION_INVALID".into(),
                category: "validation".into(),
                message: error.to_string(),
            })?;
        if matches!(request.action, memory::learning::TutorAction::SyncWorkspaceGoal { .. }) {
            return Err(read_tools::ToolError { error_code:"TUTOR_ACTION_INVALID".into(), category:"validation".into(), message:"工作区目标由目标栏或明确改向请求修改。".into() });
        }
        if matches!(request.action, memory::learning::TutorAction::SetEnabled { .. }) {
            return Err(read_tools::ToolError { error_code:"TUTOR_ACTION_INVALID".into(), category:"validation".into(), message:"连续教学开关已移除；提问、接续和练习直接使用各自入口。".into() });
        }
        learning.mutate(&request, req.now)
    })();
    match result {
        Ok(state) => Reply {
            status: 200,
            body: serde_json::to_string(&state).unwrap(),
        },
        Err(error) => Reply {
            status: if error.category == "conflict" {
                409
            } else if error.category == "validation" {
                400
            } else {
                503
            },
            body: json!(error).to_string(),
        },
    }
}

pub(crate) fn dispatch(state: &mut AppState, req: Req) -> Reply {
    if req.method == "POST" && matches!(req.url, "/tutor/ask-step" | "/tutor/grade") {
        let result = serde_json::from_str(req.body).map_err(|e| memory::teaching::invalid(e.to_string()))
            .and_then(|input| crate::reading_guidance::GuidanceWork::prepare(&state.private_context(), req.url.trim_start_matches('/'), &input, req.now))
            .and_then(|work| work.execute(state.services.adapter.as_ref()).and_then(|answer| work.finish(&state.private_context(), answer, req.now)));
        return match result { Ok(value) => crate::ok_json(&value), Err(error) => crate::err_reply(&error) };
    }
    if !matches!(req.url, "/tutor/state" | "/tutor/mutate" | "/tutor/readiness") {
        return crate::teaching::route(&state.private_context(), req);
    }
    if req.url == "/tutor/readiness" {
        if req.method != "GET" { return agent_method_not_allowed(); }
        return readiness(state);
    }
    if (req.url == "/tutor/state" && req.method != "GET")
        || (req.url == "/tutor/mutate" && req.method != "POST")
    {
        return agent_method_not_allowed();
    }
    route(state, req)
}

fn readiness(state: &AppState) -> Reply {
    crate::ok_json(&readiness_view(&state.workspace.book, &state.workspace.book_dir))
}

/// Reader-facing admission and optional assets; private build receipts stay internal.
pub fn readiness_view(book: &read_tools::Book, book_dir: &std::path::Path) -> serde_json::Value {
    let ready = tutor_source_readiness(book, book_dir);
    json!({"status":ready["status"],"source_id":ready["source_id"],
        "source_revision":ready["source_revision"],"reason":ready["reason"],
        "limitations":ready["teaching_assets"]["limitations"].as_array().cloned().unwrap_or_default(),
        "teaching_assets":ready["teaching_assets"]})
}

pub fn source_readiness(book: &read_tools::Book, book_dir: &std::path::Path) -> Reply {
    let source_id = &book.base.book_id;
    let preparing = |reason: &str, status: &str| Reply { status: 200, body: json!({
        "status": status, "source_id": source_id, "teaching_map_revision": null,
        "limitations": [], "reason": reason,
    }).to_string() };
    let receipt_path = book_dir.join("teaching_readiness.json");
    if !receipt_path.exists() { return preparing("正式教学素材尚未发布", "preparing"); }
    let result = (|| -> Result<serde_json::Value, String> {
        let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(&receipt_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if receipt["source_id"].as_str() != Some(source_id.as_str())
            || receipt["source_revision"].as_str() != Some(book.source_fingerprint()) {
            return Err("来源已更新，教学素材需要重新发布".into());
        }
        if receipt["version"] != "teaching_readiness.v1" || receipt["status"] != "ready"
            || ["source", "structure", "objects", "cognitive_materials"].iter().any(|key| receipt["coverage"][key] != "complete")
            || receipt["coverage"]["source_review"] != "passed" {
            return Err("教学素材覆盖或来源审阅尚未完成".into());
        }
        let revision = receipt["teaching_map_revision"].as_str().filter(|r| !r.is_empty()).ok_or("教学版本缺失")?;
        let expected_path = format!("teaching/versions/{revision}/map.json");
        if revision.contains(['/', '\\']) || receipt["map_path"].as_str() != Some(expected_path.as_str()) { return Err("教学版本引用无效".into()); }
        let map: serde_json::Value = serde_json::from_slice(&std::fs::read(book_dir.join(expected_path)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if map["source_revision"] != receipt["source_revision"] || map["source_id"] != receipt["source_id"] {
            return Err("教学版本与来源不一致".into());
        }
        Ok(json!({ "status": "ready", "source_id": source_id, "teaching_map_revision": revision,
            "limitations": receipt["limitations"], "reason": "教学材料已就绪，可以开始学习" }))
    })();
    match result {
        Ok(view) => Reply { status: 200, body: view.to_string() },
        Err(reason) => preparing(&reason, "stale"),
    }
}

/// Tutor admission used by start and turn preparation; asset completeness is separate.
pub fn tutor_source_readiness(book: &read_tools::Book, book_dir: &std::path::Path) -> serde_json::Value {
    // Admission follows the public artifacts in both workspaces and sealed
    // publications. Build history and saved admission decisions are not prerequisites.
    let mut view = tutor_prerequisites(book, book_dir);
    if let Some(publication) = read_json(&book_dir.join("publication.json")) {
        let saved = &publication["tutor_readiness"];
        if publication["source_fingerprint"] == book.source_fingerprint()
            && saved["source_id"] == book.base.book_id
            && saved["source_revision"] == book.source_fingerprint()
            && saved["teaching_assets"].is_object()
        {
            view["teaching_assets"] = saved["teaching_assets"].clone();
            return view;
        }
    }
    let mut assets: serde_json::Value = serde_json::from_str(&source_readiness(book, book_dir).body).unwrap();
    let pass1 = accepted_stage(book, book_dir, "pass1", None);
    let input = pass1.as_ref().and_then(|receipt| receipt["target"]["input_fingerprint"].as_str());
    for (stage, file) in [("formal_objects", "formal_objects.json"), ("cognitive_materials", "cognitive_materials.json")] {
        let artifact = read_json(&book_dir.join(file));
        assets[stage] = match artifact {
            None => json!({"status":"missing"}),
            Some(value) if value["source_id"] != book.base.book_id || value["source_revision"] != book.source_fingerprint() => json!({"status":"stale"}),
            Some(value) if accepted_stage(book, book_dir, stage, input).is_some() =>
                json!({"status":"ready","path":file,"revision":value["revision"]}),
            Some(_) => json!({"status":"unavailable"}),
        };
    }
    view["teaching_assets"] = assets;
    view
}

/// Only accepted public artifacts enter this projection. Build candidates stay private.
fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn accepted_stage(
    book: &read_tools::Book,
    root: &std::path::Path,
    stage: &str,
    input: Option<&str>,
) -> Option<serde_json::Value> {
    let directory = root.join(".build/automatic-build/v2/close").join(stage);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).ok()?.flatten() {
        if entry.path().is_dir() {
            files.extend(std::fs::read_dir(entry.path()).ok()?.flatten().map(|e| e.path()));
        } else {
            files.push(entry.path());
        }
    }
    files.sort();
    for file in files {
        let Some(close) = read_json(&file) else { continue };
        if close["version"] != "automatic_build_stage_close_result.v2" || close["stage"] != stage
            || close["status"] != "closed" || close["quality"]["gate_status"] != "passed"
            || close["postcondition"]["stage_closed"] != true
            || close["target"]["book_id"] != book.base.book_id
            || close["target"]["profile_id"] != crate::current_content_profile(book)
            || close["target"]["input_fingerprint"].as_str().is_none_or(str::is_empty)
            || input.is_some_and(|i| close["target"]["input_fingerprint"] != i)
            || close["postcondition"]["policy_contracts"].as_array().is_none_or(|contracts|
                contracts.iter().any(|c| c["semantic_contract"]["quality_profile"] != "full"))
        { continue; }
        let Some(transaction) = close["publication"]["transaction_id"].as_str()
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())) else { continue };
        let path = root.join(".build/automatic-build/v2/publication").join(stage).join(transaction).join("receipt.json");
        let Some(receipt) = read_json(&path) else { continue };
        if receipt["version"] != "automatic_build_publication_receipt.v1" || receipt["status"] != "committed"
            || receipt["stage"] != stage || receipt["transaction_id"] != transaction
            || close["publication"]["receipt_digest"] != crate::sha256_hex(receipt.to_string().as_bytes())
        { continue; }
        let Some(artifacts) = receipt["artifacts"].as_array().filter(|a| !a.is_empty()) else { continue };
        // Pass2 legitimately updates base.json after Pass1. Its canonical source receipt
        // anchors Pass1; the current sidecars and structure must match their own receipt.
        let required: &[&str] = match stage {
            "pass1" if crate::current_content_profile(book) == "paper" => &["profile_metadata.json"],
            "pass1" => &["source.txt"],
            "profile_sidecar" => &["discourse_index.json", "formula_semantics.json"],
            "book_structure" => &["book_structure.json"],
            "formal_objects" => &["formal_objects.json"],
            "cognitive_materials" => &["cognitive_materials.json"],
            _ => continue,
        };
        if required.iter().all(|name| artifacts.iter().any(|a| {
            if a["path"] != *name { return false; }
            let Ok(bytes) = std::fs::read(root.join(name)) else { return false };
            a["size_bytes"].as_u64() == Some(bytes.len() as u64)
                && a["sha256"] == crate::sha256_hex(&bytes)
                && (*name != "source.txt" || a["sha256"] == book.source_fingerprint())
        })) {
            return Some(json!({"target":close["target"],"transaction_id":transaction,"artifacts":artifacts}));
        }
    }
    None
}

fn tutor_prerequisites(book: &read_tools::Book, root: &std::path::Path) -> serde_json::Value {
    let unavailable = |stage: &str, reason: &str| json!({"status":"preparing","source_id":book.base.book_id,
        "source_revision":book.source_fingerprint(),"blocked_stage":stage,"reason":reason});
    if root.join(crate::music_media::MANIFEST).exists() {
        return match crate::music_media::load(root).and_then(|music| {
            crate::music_media::validate_files(root, &music)?;
            Ok(music)
        }) {
            Ok(music) if music.book_id == book.base.book_id && !music.events.is_empty() && !music.structure.is_empty() =>
                json!({"status":"ready","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
                    "reason":"音乐谱面、事件与整体结构已就绪，可以开始伴听",
                    "required_stages":{"music_prebuild":{"status":"done"}},"coverage":music.coverage}),
            Ok(music) if music.book_id == book.base.book_id && music.events.is_empty() && !music.structure.is_empty() =>
                unavailable("music_prebuild", "本章可听原声、看总谱、按段落跳转，尚无精选导听片段"),
            _ => unavailable("music_prebuild", "音乐资料的谱面、事件或整体结构尚未准备完成"),
        };
    }
    if root.join("video_source_manifest.json").exists() {
        let Ok((video, _)) = book.load_video_source(root) else {
            return unavailable("video_transcript", "视频的完整字幕、时间定位或原片尚未准备完成");
        };
        if !video.coverage.iter().any(|coverage| coverage.capability == base_schema::VideoCapability::Discourse
            && coverage.gaps(&video.source).is_ok_and(|gaps| gaps.is_empty()))
            || !book.has_video_discourse()
            || !video_structure_available(root, &video)
        {
            return unavailable("video_structure", "视频的全片原讲解结构尚未准备完成");
        }
        return json!({"status":"ready","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
            "reason":"视频字幕、原片与全片原讲解结构已就绪，可以开始连续学习",
            "required_stages":{"video_transcript":{"status":"done"},"video_structure":{"status":"done"}},
            "coverage":video.coverage});
    }
    // Book is already loaded by the Reader; Pass1's runtime product is base.json.
    if !root.join("source.txt").is_file()
        || crate::read_book_id_from_base(&root.join("base.json")).as_deref() != Some(book.base.book_id.as_str()) {
        return unavailable("pass1", "当前来源的原文或 Pass1 基座缺失或无法读取");
    }
    if read_json(&root.join("publication.json"))
        .is_some_and(|p| p["source_fingerprint"] != book.source_fingerprint())
        || read_json(&root.join("source_manifest.json")).is_some_and(|m|
            m["book_id"] != book.base.book_id
                || (!crate::published_library::canonical_source_v1(&m)
                    && m["canonical_source"]["sha256"] != book.source_fingerprint())) {
        return unavailable("hybrid_foundation", "当前来源与已加载的书籍版本不一致");
    }
    let valid_discourse = read_json(&root.join("discourse_index.json"))
        .and_then(|v| serde_json::from_value::<read_tools::TechnicalLearningDiscourseIndex>(v).ok())
        .is_some();
    if !valid_discourse {
        return unavailable("profile_sidecar", "当前来源的 discourse 缺失或无法读取");
    }
    let valid_structure = read_json(&root.join("book_structure.json"))
        .and_then(|v| serde_json::from_value::<read_tools::BookStructureSidecar>(v).ok())
        .is_some_and(|s| s.header.book_id == book.base.book_id && s.header.profile_id == crate::current_content_profile(book));
    if !valid_structure {
        return unavailable("book_structure", "BookStructure 缺失、无法读取或属于其他来源");
    }
    json!({"status":"ready","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
        "reason":"Pass1、discourse 与 BookStructure 已就绪，可以使用 Tutor",
        "required_stages":{"pass1":{"status":"done"},"profile_sidecar":{"status":"done"},
            "book_structure":{"status":"done"}}})
}

/// Native video structure replaces the book's discourse/BookStructure sidecars.
/// Publication admission and Book::load validate its evidence; reads check that
/// the accepted contributions and their dependencies are still available.
fn video_structure_available(root: &std::path::Path, video: &base_schema::VideoSourceManifest) -> bool {
    let Some(catalog) = read_json(&root.join("video_enrichments.json")) else { return false };
    if catalog["version"] != "video_enrichments.v1" || catalog["material"] != json!(video.material) {
        return false;
    }
    let Some(batches) = catalog["batches"].as_array() else { return false };
    let mut found = false;
    for batch in batches {
        let Some(files) = batch["files"].as_array() else { return false };
        if !files.iter().any(|file| file.as_str().is_some_and(|file| file.ends_with("/discourse.json"))) {
            continue;
        }
        if !files.iter().all(|file| file.as_str().is_some_and(|file| root.join(file).is_file())) {
            return false;
        }
        found = true;
    }
    found
}
