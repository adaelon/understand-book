//! Bounded, file-backed reads from the Run's immutable publication.
use base64::Engine;
use base_schema::{FrameLocation, FrameRead, FrameRegion, PublishedBookRef};
use read_tools::{Book, ToolError};
use runtime::{
    run_context::CancellationToken,
    video::{FrameBatch, ReadFrames, VideoImage},
};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "VIDEO_READ_FAILED".into(),
        category: "invalid_input".into(),
        message: message.into(),
    }
}

fn failed(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "VIDEO_READ_FAILED".into(),
        category: "unavailable".into(),
        message: message.into(),
    }
}

pub(crate) fn require_reference(scope: &crate::run_scope::RunScope, reference: Option<&PublishedBookRef>) -> Result<(), ToolError> {
    if let Some(reference) = reference {
        let declared: crate::published_library::PublicationManifest = serde_json::from_slice(
            &fs::read(scope.book_dir.join("publication.json")).map_err(|_| runtime::video::unavailable())?
        ).map_err(|_| runtime::video::unavailable())?;
        if &declared.reference != reference { return Err(runtime::video::unavailable()); }
    }
    Ok(())
}

/// Browser mediaTime is rounded to microseconds. Recover its integer stream PTS before
/// storing FrameLocation, so a rounded-up presentation time does not exclude that frame.
pub(crate) fn presentation_time_base(directory: &Path, source: &base_schema::VideoSourceAsset) -> Result<[u32; 2], ToolError> {
    let mut command = Command::new(crate::video_media::executable("ffprobe"));
    command.args(["-v", "error", "-select_streams", "V:0", "-show_entries", "stream=time_base", "-of", "csv=p=0"])
        .arg(directory.join(&source.imported_path)).stdin(Stdio::null());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().map_err(|e| failed(format!("Cannot read video time base: {e}")))?;
    if !output.status.success() { return Err(failed("Cannot read video time base")); }
    let value = String::from_utf8_lossy(&output.stdout);
    let (n, d) = value.trim().split_once('/').ok_or_else(|| failed("Missing video time base"))?;
    let parse = |v: &str| v.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(|| failed("Invalid video time base"));
    Ok([parse(n)?, parse(d)?])
}

pub(crate) fn source_context(
    scope: &crate::run_scope::RunScope,
) -> Result<Option<serde_json::Value>, ToolError> {
    if !scope.book_dir.join("video_source_manifest.json").is_file() {
        return Ok(None);
    }
    let publication: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(scope.book_dir.join("publication.json")).map_err(|e| failed(e.to_string()))?,
    )
    .map_err(|e| failed(e.to_string()))?;
    if scope
        .publication
        .as_ref()
        .is_some_and(|p| p.reference != publication.reference)
    {
        return Err(failed("Run publication mismatch; reopen the intended publication before retrying."));
    }
    let (manifest, _) = scope.book.load_video_source(&scope.book_dir)?;
    Ok(Some(
        serde_json::json!({"publication":publication.reference,"video":manifest.source.video,"transcript":manifest.material.transcript,"duration_ms":manifest.source.duration_ms,
        "width":manifest.source.width,"height":manifest.source.height,"tools":["video.search","video.read_frames","video.review_audio"],
        "guidance":"Use tool.search to discover video.search for earlier diagrams, visible code or repeated occurrences. Search returns derived candidates and actual index gaps; follow with video.read_frames. Current-frame questions can read directly even without an index. Sampling uses original milliseconds; small code/formulas can be reread as a pixel crop. For uncertain spoken wording discover video.review_audio: short original audio is processed by Whisper; preserve its uncertainties and use its audio source target. Cards and transcript alone do not establish visual observation."}),
    ))
}

pub(crate) fn search_for_run(scope: &crate::run_scope::RunScope, request: read_tools::video_search::VideoSearch) -> Result<serde_json::Value, ToolError> {
    let publication: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(scope.book_dir.join("publication.json")).map_err(|_| runtime::video::unavailable())?
    ).map_err(|e| failed(e.to_string()))?;
    if scope.publication.as_ref().is_some_and(|p| p.reference != publication.reference) { return Err(failed("Run publication mismatch; reopen the intended publication before retrying.")); }
    scope.book.search_video(&scope.book_dir, &publication.reference, request)
}

pub(crate) fn read_for_run(
    scope: &crate::run_scope::RunScope,
    request: ReadFrames,
    cancel: &CancellationToken,
) -> Result<FrameBatch, ToolError> {
    let publication: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(scope.book_dir.join("publication.json"))
            .map_err(|_| runtime::video::unavailable())?,
    )
    .map_err(|_| runtime::video::unavailable())?;
    if scope
        .publication
        .as_ref()
        .is_some_and(|p| p.reference != publication.reference)
    {
        return Err(failed("Run publication mismatch; reopen the intended publication before retrying."));
    }
    let cache = scope
        .publication
        .as_ref()
        .map(|p| p.localization_cache.parent().unwrap().join("video-frames"))
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("understand-book-video-frames")
                .join(&publication.reference.publication_id)
        });
    read_frames(
        &scope.book,
        &scope.book_dir,
        &publication.reference,
        &scope.turn_id,
        &cache,
        request,
        cancel,
    )
}

pub fn read_frames(
    book: &Book,
    directory: &Path,
    publication: &PublishedBookRef,
    run_id: &str,
    cache: &Path,
    request: ReadFrames,
    cancel: &CancellationToken,
) -> Result<FrameBatch, ToolError> {
    cancel.check()?;
    // The exact publication file, not just matching book_id, binds the source.
    let declared: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(directory.join("publication.json")).map_err(|e| failed(e.to_string()))?,
    )
    .map_err(|e| failed(e.to_string()))?;
    if declared.reference != *publication {
        return Err(failed("Publication identity mismatch; reopen the intended publication before retrying."));
    }
    let (manifest, map) = book.load_video_source(directory)?;
    manifest
        .validate_publication(publication)
        .map_err(failed)?;
    let source = &manifest.source;
    source
        .validate_location(&request.location)
        .map_err(invalid)?;
    request.validate_sampling()?;
    let rect = if let Some(region) = request.region {
        source
            .validate_region(&FrameRegion {
                frame: FrameLocation {
                    video: source.video.clone(),
                    actual_pts_ms: request.sampling[0],
                    width: source.width,
                    height: source.height,
                },
                rect_normalized: region,
            })
            .map_err(invalid)?;
        let [x, y, w, h] = region;
        let left = (x * source.width as f64).floor() as u32;
        let top = (y * source.height as f64).floor() as u32;
        let right = ((x + w) * source.width as f64)
            .ceil()
            .min(source.width as f64) as u32;
        let bottom = ((y + h) * source.height as f64)
            .ceil()
            .min(source.height as f64) as u32;
        Some([left, top, right - left, bottom - top])
    } else {
        None
    };
    fs::create_dir_all(cache).map_err(|e| failed(format!("Cannot create video frame cache: {e}")))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut images = Vec::new();
    let mut bytes = 0;
    for requested_ms in &request.sampling {
        cancel.check()?;
        // Numeric parameters plus immutable publication identity replace repeated decoding.
        let key = format!("{}-{:?}", requested_ms.to_bits(), rect);
        let target = cache.join(key);
        if !target.join("frame.json").is_file() || !target.join("frame.png").is_file() {
            let staging = tempfile::tempdir_in(cache).map_err(|e| failed(e.to_string()))?;
            let log = fs::File::create(staging.path().join("decode.log"))
                .map_err(|e| failed(e.to_string()))?;
            let filter = match rect {
                Some([x, y, w, h]) => {
                    format!("showinfo=checksum=0,format=rgb24,crop={w}:{h}:{x}:{y}:exact=1")
                }
                None => "showinfo=checksum=0".into(),
            };
            let mut command =
                Command::new(crate::video_media::executable("ffmpeg"));
            command
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "info",
                    "-copyts",
                    "-ss",
                    &(*requested_ms / 1000.0).to_string(),
                    "-i",
                ])
                .arg(directory.join(&source.imported_path))
                .args([
                    "-map",
                    "0:V:0",
                    "-vf",
                    &filter,
                    "-frames:v",
                    "1",
                    "-fps_mode",
                    "passthrough",
                    "-y",
                ])
                .arg(staging.path().join("frame.png"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::from(log));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let mut child = command
                .spawn()
                .map_err(|e| failed(format!("Cannot start FFmpeg: {e}; restore the configured or bundled FFmpeg executable before retrying.")))?;
            // Command retains its configured stderr handle on Windows until dropped.
            drop(command);
            let status = loop {
                if cancel.is_cancelled() || Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    cancel.check()?;
                    return Err(failed(
                        "Video read exceeded 60 seconds; request fewer frames",
                    ));
                }
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(failed(e.to_string()));
                    }
                }
            };
            let log = fs::read_to_string(staging.path().join("decode.log"))
                .map_err(|e| failed(e.to_string()))?;
            if !status.success() {
                return Err(failed(format!(
                    "Video decode failed: {}",
                    log.chars()
                        .rev()
                        .take(1200)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                )));
            }
            let frame = decode_location(&log, &source.video)?;
            source.validate_frame(&frame).map_err(failed)?;
            fs::write(
                staging.path().join("frame.json"),
                serde_json::to_vec(&frame).unwrap(),
            )
            .map_err(|e| failed(e.to_string()))?;
            match fs::rename(staging.path(), &target) {
                Ok(()) => {}
                Err(_)
                    if target.join("frame.json").is_file()
                        && target.join("frame.png").is_file() => {}
                Err(e) => return Err(failed(e.to_string())),
            }
        }
        let frame: FrameLocation = serde_json::from_slice(
            &fs::read(target.join("frame.json")).map_err(|e| failed(e.to_string()))?,
        )
        .map_err(|e| failed(e.to_string()))?;
        source.validate_frame(&frame).map_err(failed)?;
        if frame.actual_pts_ms < request.location.start_ms
            || frame.actual_pts_ms >= request.location.end_ms
        {
            return Err(invalid(format!("Decoded frame actual_pts_ms={} for requested_ms={} lies outside [start_ms={}, end_ms={}) ms. Decoding can advance to the next available frame; widen end_ms past that frame without exceeding duration_ms={}, or sample an earlier time.", frame.actual_pts_ms, requested_ms, request.location.start_ms, request.location.end_ms, source.duration_ms)));
        }
        bytes += fs::metadata(target.join("frame.png"))
            .map_err(|e| failed(e.to_string()))?
            .len();
        if bytes > 16 * 1024 * 1024 {
            return Err(invalid(
                "PNG input unavailable or batch exceeds 16 MiB; use fewer frames or a crop",
            ));
        }
        let png = fs::read(target.join("frame.png")).map_err(|e| failed(e.to_string()))?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(failed("Decoded PNG unavailable"));
        }
        let region = rect.map(|[x, y, w, h]| FrameRegion {
            frame: frame.clone(),
            rect_normalized: [
                x as f64 / frame.width as f64,
                y as f64 / frame.height as f64,
                w as f64 / frame.width as f64,
                h as f64 / frame.height as f64,
            ],
        });
        let (image_width, image_height) = rect
            .map(|[_, _, w, h]| (w, h))
            .unwrap_or((frame.width, frame.height));
        if images
            .last()
            .is_some_and(|i: &VideoImage| i.read.frame.actual_pts_ms >= frame.actual_pts_ms)
        {
            continue;
        }
        images.push(VideoImage {
            read: FrameRead {
                requested_ms: *requested_ms,
                frame,
            },
            region,
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            image_width,
            image_height,
        });
    }
    let transcript = map
        .segments
        .into_iter()
        .filter(|s| {
            s.location.start_ms < request.location.end_ms
                && s.location.end_ms > request.location.start_ms
        })
        .collect();
    cancel.check()?;
    Ok(FrameBatch {
        run_id: run_id.into(),
        publication: publication.clone(),
        location: request.location,
        images,
        transcript,
    })
}

fn decode_location(
    log: &str,
    video: &base_schema::VideoSourceRef,
) -> Result<FrameLocation, ToolError> {
    let time_base = log
        .lines()
        .find_map(|l| {
            l.split_once("config in time_base: ")
                .map(|(_, v)| v.split(',').next().unwrap().trim())
        })
        .ok_or_else(|| failed("Missing decoder time base"))?;
    let (n, d) = time_base
        .split_once('/')
        .ok_or_else(|| failed("Invalid decoder time base"))?;
    let number = |s: &str| {
        s.parse::<f64>()
            .map_err(|_| failed("Invalid decoder number"))
    };
    let line = log
        .lines()
        .find(|l| {
            l.contains(" n:")
                && l.split_once(" n:")
                    .is_some_and(|(_, v)| v.trim_start().starts_with("0 "))
        })
        .ok_or_else(|| failed("No decoded frame; no available frame was returned at the requested time. Try an earlier sample within the video."))?;
    let field = |name: &str| {
        line.split_once(name)
            .and_then(|(_, v)| v.split_whitespace().next())
            .ok_or_else(|| failed("Missing decoded frame metadata"))
    };
    let pts = number(field(" pts:")?)?;
    let (w, h) = field(" s:")?
        .split_once('x')
        .ok_or_else(|| failed("Invalid decoded frame size"))?;
    Ok(FrameLocation {
        video: video.clone(),
        actual_pts_ms: pts * number(n)? / number(d)? * 1000.0,
        width: w.parse().map_err(|_| failed("Invalid decoded frame width"))?,
        height: h.parse().map_err(|_| failed("Invalid decoded frame height"))?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn fixture(dir: &Path) -> (Book, PublishedBookRef, ReadFrames) {
        fixture_with_duration(dir, 2)
    }

    pub(crate) fn fixture_with_duration(dir: &Path, seconds: u32) -> (Book, PublishedBookRef, ReadFrames) {
        fs::create_dir_all(dir.join("media")).unwrap();
        let status = Command::new(std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".into()))
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc2=size=160x90:rate=10:duration={seconds}"),
                "-c:v",
                "libx264",
                "-y",
            ])
            .arg(dir.join("media/original.mp4"))
            .status()
            .unwrap();
        assert!(status.success());
        let video = json!({"media_id":"test-video","media_revision":"1"});
        let transcript = json!({"transcript_id":"t","transcript_revision":"1","video":video});
        let duration_ms = seconds * 1000;
        let location = json!({"video":video,"start_ms":0,"end_ms":duration_ms});
        let reference = PublishedBookRef {
            book_id: "test-book".into(),
            publication_id: "test-pub".into(),
        };
        let base = json!({"book_id":reference.book_id,"lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":8},"children":[]}],"graph_nodes":[],"graph_edges":[]});
        let book = Book::new(serde_json::from_value(base).unwrap(), "A frame.");
        fs::write(dir.join("raw.json"), "{}").unwrap();
        fs::write(dir.join("video_source_manifest.json"),json!({"version":"video_source_manifest.v1","material":{"book_id":reference.book_id,"video":video,"transcript":transcript},"source":{"video":video,"imported_path":"media/original.mp4","original_path":"fixture.mp4","duration_ms":duration_ms,"width":160,"height":90},"canonical_text_path":"source.txt","transcript_map_path":"transcript_map.json","coverage":[{"capability":"transcript","accepted":[location],"pending":[],"failed":[]}],"previous_book_id":null}).to_string()).unwrap();
        fs::write(dir.join("transcript_map.json"),json!({"transcript":transcript,"raw_result_path":"raw.json","machine_transcription":{"engine":"fixture","model":"fixture","language":"en"},"segments":[{"segment_id":"s","text":"A frame.","location":location,"text_range":{"lid":"1","range":{"start":0,"end":8}},"words":[]}]}).to_string()).unwrap();
        fs::write(dir.join("publication.json"),json!({"version":"publication.v1","reference":reference,"source_fingerprint":"fixture-existing-contract","files":{},"capabilities":{},"artifact_versions":{},"teaching_readiness":{},"build_readiness":{},"resident_bytes":0}).to_string()).unwrap();
        (
            book,
            reference,
            serde_json::from_value(json!({"location":location,"sampling":[123,1023]})).unwrap(),
        )
    }

    fn pixels(image: &VideoImage) -> (u32, u32, Vec<u8>) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&image.png_base64)
            .unwrap();
        let mut reader = png::Decoder::new(bytes.as_slice()).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(info.buffer_size());
        assert_eq!(info.color_type, png::ColorType::Rgb);
        (info.width, info.height, pixels)
    }

    #[test]
    fn video_search_run_binding_and_candidate_to_real_frame() {
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let (book, reference, request) = fixture(dir.path());
        let reader = reader::Reader::new(&book, 1);
        let input = runtime::run_context::ReaderInputSnapshot::capture(&book, &reader, "find code");
        let book = Arc::new(book);
        let declared: crate::published_library::PublicationManifest = serde_json::from_slice(&fs::read(dir.path().join("publication.json")).unwrap()).unwrap();
        let publication = crate::published_library::PublishedBook { reference:reference.clone(), book:book.clone(), manifest:declared,
            directory:dir.path().into(),localization_cache:dir.path().join("cache/localization.json") };
        let mut scope = crate::run_scope::RunScope { board_context: None, music_question: None, learning_workspace_id: None, goal: None, members: None, publication:Some(Arc::new(publication)), user_id:"u".into(),workspace_id:"w".into(),
            workspace_generation:1,chat_session_id:"c".into(),turn_id:"run".into(),confirmation_id:None,book,book_dir:dir.path().into(),
            provider_binding:runtime::ModelRuntimeProfile::fallback("test",runtime::ProviderToolProtocol::Native),reader_input:input,
            question_quote:None,presentation_follow_up:None };
        let search = || serde_json::from_value(json!({"query":"visiblecode"})).unwrap();
        assert_eq!(search_for_run(&scope,search()).unwrap()["coverage"]["accepted"],json!([]));
        let original = read_for_run(&scope,request.clone(),&CancellationToken::default()).unwrap();
        let (manifest, _) = scope.book.load_video_source(dir.path()).unwrap();
        let dep="video-enrichments/v1/frame.png";
        fs::create_dir_all(dir.path().join("video-enrichments/v1")).unwrap();
        fs::write(dir.path().join(dep),base64::engine::general_purpose::STANDARD.decode(&original.images[0].png_base64).unwrap()).unwrap();
        let card="video-enrichments/v1/visual_units.json";
        fs::write(dir.path().join(card),json!({"version":"video_visual_units.v1","analysis_id":"v1","created_at":"2026-10-10T00:00:00Z","material":manifest.material,
            "units":[{"vu_id":"v1/1","analysis_id":"v1","location":request.location,"representative_frames":[{"frame":original.images[0].read.frame,"path":dep}],
            "screen_text":"visiblecode","description":"test image","change":"initial","transcript_segment_ids":["s"]}]}).to_string()).unwrap();
        fs::write(dir.path().join("video_enrichments.json"),json!({"version":"video_enrichments.v1","material":manifest.material,"batches":[{"files":[card,dep]}]}).to_string()).unwrap();
        let result=search_for_run(&scope,search()).unwrap();
        assert_eq!(result["publication"],json!(reference)); assert_eq!(result["total_matches"],1);
        let hint=serde_json::from_value(result["candidates"][0]["read_hint"].clone()).unwrap();
        let reread=read_for_run(&scope,hint,&CancellationToken::default()).unwrap();
        assert_eq!(pixels(&reread.images[0]),pixels(&original.images[0]));
        assert_eq!(reread.transcript[0].text,"A frame.");
        assert!(source_context(&scope).unwrap().unwrap()["tools"].as_array().unwrap().contains(&json!("video.search")));
        Arc::get_mut(scope.publication.as_mut().unwrap()).unwrap().reference.publication_id="wrong".into();
        assert!(search_for_run(&scope,search()).is_err());
    }

    #[test]
    fn video_original_edge_crop_survives_tool_json_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, _) = fixture(root.path());
        // The submitted Reader crop reaches the right edge exactly. Parse through
        // the same string-to-typed path used by Resident tool dispatch.
        let request: ReadFrames = serde_json::from_str(r#"{"location":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":1900,"end_ms":2000},"sampling":[1900],"region":[0.08645833333333333,0.013888888888888888,0.9135416666666667,0.9398148148148148]}"#).unwrap();
        assert_eq!(request.region.unwrap()[2],0.9135416666666667);
        let batch = read_frames(&book,root.path(),&publication,"original-edge-crop",&root.path().join("cache"),request,&CancellationToken::default()).unwrap();
        assert_eq!(batch.images[0].read.frame.actual_pts_ms,1900.0);
        let (width,height,_) = pixels(&batch.images[0]);
        assert_eq!((width,height),(147,85));
    }

    #[test]
    fn video_frozen_frame_hint_decodes_with_and_without_crop_at_video_end() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, _) = fixture(root.path());
        let (manifest, _) = book.load_video_source(root.path()).unwrap();
        let frame = FrameLocation { video:manifest.source.video.clone(), actual_pts_ms:1900.0, width:160, height:90 };
        for crop in [false,true] {
            let question = base_schema::VideoQuestionContext { draft_id:"frozen".into(),publication:publication.clone(),material:manifest.material.clone(),frame:frame.clone(),
                region:crop.then(|| FrameRegion { frame:frame.clone(),rect_normalized:[0.25,0.0,0.5,0.5] }),range:None,image:None };
            let hint = runtime::video::frozen_frame_read_hint(&question,manifest.source.duration_ms);
            assert_eq!(hint.location.end_ms,2000.0);
            let batch = read_frames(&book,root.path(),&publication,"hint",&root.path().join("cache"),hint,&CancellationToken::default()).unwrap();
            assert_eq!(batch.images.len(),1);
            assert_eq!(batch.images[0].read.frame,frame);
            let (width,height,_) = pixels(&batch.images[0]);
            assert_eq!((width,height),if crop { (80,45) } else { (160,90) });
            assert_eq!(batch.transcript.len(),1);
        }
    }

    #[test]
    fn video_tool_end_exclusive_error_and_corrected_real_read() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, mut request) = fixture(root.path());
        let cache = root.path().join("boundary-cache");
        let cancel = CancellationToken::default();
        let read = |r| read_frames(&book, root.path(), &publication, "boundary", &cache, r, &cancel);
        request.location.start_ms = 200.0;
        request.location.end_ms = 1000.0;
        request.sampling = vec![200.0, 1000.0];
        let error = read(request.clone()).unwrap_err();
        assert_eq!(error.error_code, "VIDEO_READ_FAILED");
        assert_eq!(error.category, "invalid_input");
        for detail in ["sampling[1]", "1000", "end_ms", "exclusive"] {
            assert!(error.message.contains(detail), "{detail}: {}", error.message);
        }
        assert!(!cache.exists(), "invalid samples must fail before decoding");
        request.sampling = vec![200.0, 900.0];
        let batch = read(request).unwrap();
        assert_eq!(batch.images.iter().map(|i| i.read.frame.actual_pts_ms).collect::<Vec<_>>(), vec![200.0, 900.0]);
        assert_eq!(pixels(&batch.images[0]).0, 160);
        assert_eq!(batch.transcript[0].text, "A frame.");
    }

    #[test]
    fn video_tool_frame_diagnostics_identify_the_invalid_field() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, request) = fixture(root.path());
        let cancel = CancellationToken::default();
        let read = |r| read_frames(&book, root.path(), &publication, "diagnostics", &root.path().join("cache"), r, &cancel);
        for (sampling, detail) in [
            (vec![], "sampling"), (vec![0.0; 9], "9"),
            (vec![200.0, 200.0], "sampling[1]"), (vec![300.0, 200.0], "sampling[1]"),
            (vec![-1.0], "sampling[0]"), (vec![f64::NAN], "sampling[0]"),
        ] {
            let mut bad = request.clone(); bad.sampling = sampling;
            let error = read(bad).unwrap_err();
            assert_eq!(error.category, "invalid_input");
            assert!(error.message.contains(detail), "{}", error.message);
        }
        let mut bad = request.clone(); bad.location.end_ms = 2001.0;
        assert!(read(bad).unwrap_err().message.contains("duration_ms=2000"));
        for region in [[0.9, 0.0, 0.2, 1.0], [0.0, 0.0, 0.0, 1.0]] {
            let mut bad = request.clone(); bad.region = Some(region);
            let message = read(bad).unwrap_err().message;
            assert!(message.contains("width") && message.contains("x + width"), "{message}");
        }
        fs::write(root.path().join("media/original.mp4"), "not a video").unwrap();
        let error = read(request).unwrap_err();
        assert_eq!(error.category, "unavailable");
        assert!(error.message.contains("decode"), "{}", error.message);
    }

    #[test]
    fn video_tool_advertised_frame_examples_decode() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, _) = fixture(root.path());
        let spec = runtime::video::spec();
        let examples = spec.parameters["examples"].as_array().expect("frame examples");
        assert!(examples.len() >= 2);
        for example in examples {
            let request = serde_json::from_value(example.clone()).unwrap();
            let batch = read_frames(&book, root.path(), &publication, "example", &root.path().join("cache"), request, &CancellationToken::default()).unwrap();
            assert!(!batch.images.is_empty());
            assert!(!batch.transcript.is_empty());
        }
        for example in runtime::video::search_spec().parameters["examples"].as_array().unwrap() {
            let result = book.search_video(root.path(), &publication, serde_json::from_value(example.clone()).unwrap()).unwrap();
            assert_eq!(result["evidence_status"], "candidate_only");
        }
    }

    #[test]
    fn video_tool_frame_maximum_range_and_original_incident() {
        let root = tempfile::tempdir().unwrap();
        let (book, publication, mut request) = fixture_with_duration(root.path(), 61);
        let read = |r| read_frames(&book, root.path(), &publication, "long", &root.path().join("cache"), r, &CancellationToken::default());
        let error = read(request.clone()).unwrap_err();
        assert!(error.message.contains("61000") && error.message.contains("60000"), "{}", error.message);
        request.location.end_ms = 60000.0;
        assert_eq!(read(request.clone()).unwrap().images.len(), 2);
        for (start, end, sampling) in [
            (10000.0, 20000.0, vec![10000.0,12000.0,14000.0,15433.0,17000.0,18500.0,20000.0]),
            (13000.0, 18000.0, vec![13000.0,14200.0,15433.0,16500.0,18000.0]),
        ] {
            request.location.start_ms = start; request.location.end_ms = end; request.sampling = sampling;
            let error = read(request.clone()).unwrap_err();
            assert!(error.message.contains("exclusive"), "{}", error.message);
            request.sampling.pop();
            let result = read(request.clone()).unwrap();
            assert_eq!(result.images.len(), request.sampling.len());
        }
    }

    #[test]
    fn video_frames_decode_pts_crop_pixels_cache_and_failure_boundaries() {
        let root = tempfile::tempdir().unwrap();
        let (book, reference, request) = fixture(root.path());
        let cache = root.path().join("cache");
        let cancel = CancellationToken::default();
        let read = |r: ReadFrames| {
            read_frames(&book, root.path(), &reference, "run-1", &cache, r, &cancel)
        };
        let full = read(request.clone()).unwrap();
        assert_eq!(full.images.len(), 2);
        assert_eq!(full.images[0].read.requested_ms, 123.0);
        assert_eq!(full.images[0].read.frame.actual_pts_ms, 200.0);
        assert_eq!(full.images[1].read.frame.actual_pts_ms, 1100.0);
        assert_eq!(full.transcript[0].text, "A frame.");
        assert!(!full.projection().to_string().contains("png_base64"));
        assert_eq!(full.projection()["continuous_coverage"], false);
        let mut crop_request = request.clone();
        crop_request.sampling = vec![123.0];
        crop_request.region = Some([0.125, 0.2, 0.5, 0.4]);
        let crop = read(crop_request.clone()).unwrap();
        assert_eq!(crop.images[0].read.frame, full.images[0].read.frame);
        let (w, _, original) = pixels(&full.images[0]);
        let (cw, ch, actual) = pixels(&crop.images[0]);
        assert_eq!((cw, ch), (80, 37)); // ceil((0.2 + 0.4) * 90) includes the final pixel.
        let expected: Vec<_> = (18..18 + ch as usize)
            .flat_map(|y| {
                original[(y * w as usize + 20) * 3..(y * w as usize + 20 + cw as usize) * 3]
                    .iter()
                    .copied()
            })
            .collect();
        assert_eq!(actual, expected, "crop changed source pixels");
        assert_eq!(
            read(crop_request).unwrap().images[0].png_base64,
            crop.images[0].png_base64
        );
        let mut invalid = request.clone();
        invalid.sampling = vec![1023.0, 123.0];
        assert!(read(invalid).is_err());
        let mut invalid = request.clone();
        invalid.sampling = vec![123.0; 9];
        assert!(read(invalid).is_err());
        let mut invalid = request.clone();
        invalid.location.end_ms = 150.0;
        invalid.sampling = vec![123.0];
        assert!(read(invalid).is_err());
        let mut invalid = request.clone();
        invalid.region = Some([0.9, 0.0, 0.2, 0.2]);
        assert!(read(invalid).is_err());
        let mut other = reference.clone();
        other.publication_id = "other".into();
        assert!(read_frames(
            &book,
            root.path(),
            &other,
            "run-1",
            &cache,
            request.clone(),
            &cancel
        )
        .is_err());
        let stopped = CancellationToken::default();
        stopped.cancel();
        assert_eq!(
            read_frames(
                &book,
                root.path(),
                &reference,
                "run-1",
                &cache,
                request.clone(),
                &stopped
            )
            .unwrap_err()
            .error_code,
            "AGENT_RUN_CANCELLED"
        );
        fs::write(root.path().join("media/original.mp4"), "not a video").unwrap();
        assert!(read_frames(
            &book,
            root.path(),
            &reference,
            "run-1",
            &root.path().join("failed-cache"),
            request,
            &cancel
        )
        .is_err());
    }
}
