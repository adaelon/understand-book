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
    let output = command.output().map_err(|e| invalid(format!("Cannot read video time base: {e}")))?;
    if !output.status.success() { return Err(invalid("Cannot read video time base")); }
    let value = String::from_utf8_lossy(&output.stdout);
    let (n, d) = value.trim().split_once('/').ok_or_else(|| invalid("Missing video time base"))?;
    let parse = |v: &str| v.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(|| invalid("Invalid video time base"));
    Ok([parse(n)?, parse(d)?])
}

pub(crate) fn source_context(
    scope: &crate::run_scope::RunScope,
) -> Result<Option<serde_json::Value>, ToolError> {
    if !scope.book_dir.join("video_source_manifest.json").is_file() {
        return Ok(None);
    }
    let publication: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(scope.book_dir.join("publication.json")).map_err(|e| invalid(e.to_string()))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    if scope
        .publication
        .as_ref()
        .is_some_and(|p| p.reference != publication.reference)
    {
        return Err(invalid("Run publication mismatch"));
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
    ).map_err(|e| invalid(e.to_string()))?;
    if scope.publication.as_ref().is_some_and(|p| p.reference != publication.reference) { return Err(invalid("Run publication mismatch")); }
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
        return Err(invalid("Run publication mismatch"));
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
        &fs::read(directory.join("publication.json")).map_err(|e| invalid(e.to_string()))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    if declared.reference != *publication {
        return Err(invalid("Publication identity mismatch"));
    }
    let (manifest, map) = book.load_video_source(directory)?;
    manifest
        .validate_publication(publication)
        .map_err(invalid)?;
    let source = &manifest.source;
    source
        .validate_location(&request.location)
        .map_err(invalid)?;
    if request.location.end_ms - request.location.start_ms > 60_000.0
        || request.sampling.is_empty()
        || request.sampling.len() > 8
        || request.sampling.windows(2).any(|p| p[0] >= p[1])
        || request.sampling.iter().any(|t| {
            !t.is_finite() || *t < request.location.start_ms || *t >= request.location.end_ms
        })
    {
        return Err(invalid(
            "Use 1–8 ascending sample times inside a range of at most 60000 ms",
        ));
    }
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
    fs::create_dir_all(cache).map_err(|e| invalid(e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut images = Vec::new();
    let mut bytes = 0;
    for requested_ms in &request.sampling {
        cancel.check()?;
        // Numeric parameters plus immutable publication identity replace repeated decoding.
        let key = format!("{}-{:?}", requested_ms.to_bits(), rect);
        let target = cache.join(key);
        if !target.join("frame.json").is_file() || !target.join("frame.png").is_file() {
            let staging = tempfile::tempdir_in(cache).map_err(|e| invalid(e.to_string()))?;
            let log = fs::File::create(staging.path().join("decode.log"))
                .map_err(|e| invalid(e.to_string()))?;
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
                .map_err(|e| invalid(format!("Cannot start FFmpeg: {e}")))?;
            // Command retains its configured stderr handle on Windows until dropped.
            drop(command);
            let status = loop {
                if cancel.is_cancelled() || Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    cancel.check()?;
                    return Err(invalid(
                        "Video read exceeded 60 seconds; request fewer frames",
                    ));
                }
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(invalid(e.to_string()));
                    }
                }
            };
            let log = fs::read_to_string(staging.path().join("decode.log"))
                .map_err(|e| invalid(e.to_string()))?;
            if !status.success() {
                return Err(invalid(format!(
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
            source.validate_frame(&frame).map_err(invalid)?;
            fs::write(
                staging.path().join("frame.json"),
                serde_json::to_vec(&frame).unwrap(),
            )
            .map_err(|e| invalid(e.to_string()))?;
            match fs::rename(staging.path(), &target) {
                Ok(()) => {}
                Err(_)
                    if target.join("frame.json").is_file()
                        && target.join("frame.png").is_file() => {}
                Err(e) => return Err(invalid(e.to_string())),
            }
        }
        let frame: FrameLocation = serde_json::from_slice(
            &fs::read(target.join("frame.json")).map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
        source.validate_frame(&frame).map_err(invalid)?;
        if frame.actual_pts_ms < request.location.start_ms
            || frame.actual_pts_ms >= request.location.end_ms
        {
            return Err(invalid(
                "Decoded frame lies outside the requested range; widen the range",
            ));
        }
        bytes += fs::metadata(target.join("frame.png"))
            .map_err(|e| invalid(e.to_string()))?
            .len();
        if bytes > 16 * 1024 * 1024 {
            return Err(invalid(
                "PNG input unavailable or batch exceeds 16 MiB; use fewer frames or a crop",
            ));
        }
        let png = fs::read(target.join("frame.png")).map_err(|e| invalid(e.to_string()))?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(invalid("Decoded PNG unavailable"));
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
        .ok_or_else(|| invalid("Missing decoder time base"))?;
    let (n, d) = time_base
        .split_once('/')
        .ok_or_else(|| invalid("Invalid time base"))?;
    let number = |s: &str| {
        s.parse::<f64>()
            .map_err(|_| invalid("Invalid decoder number"))
    };
    let line = log
        .lines()
        .find(|l| {
            l.contains(" n:")
                && l.split_once(" n:")
                    .is_some_and(|(_, v)| v.trim_start().starts_with("0 "))
        })
        .ok_or_else(|| invalid("No decoded frame"))?;
    let field = |name: &str| {
        line.split_once(name)
            .and_then(|(_, v)| v.split_whitespace().next())
            .ok_or_else(|| invalid("Missing frame metadata"))
    };
    let pts = number(field(" pts:")?)?;
    let (w, h) = field(" s:")?
        .split_once('x')
        .ok_or_else(|| invalid("Invalid frame size"))?;
    Ok(FrameLocation {
        video: video.clone(),
        actual_pts_ms: pts * number(n)? / number(d)? * 1000.0,
        width: w.parse().map_err(|_| invalid("Invalid width"))?,
        height: h.parse().map_err(|_| invalid("Invalid height"))?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn fixture(dir: &Path) -> (Book, PublishedBookRef, ReadFrames) {
        fs::create_dir_all(dir.join("media")).unwrap();
        let status = Command::new(std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".into()))
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=160x90:rate=10:duration=2",
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
        let location = json!({"video":video,"start_ms":0,"end_ms":2000});
        let reference = PublishedBookRef {
            book_id: "test-book".into(),
            publication_id: "test-pub".into(),
        };
        let base = json!({"book_id":reference.book_id,"lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":8},"children":[]}],"graph_nodes":[],"graph_edges":[]});
        let book = Book::new(serde_json::from_value(base).unwrap(), "A frame.");
        fs::write(dir.join("raw.json"), "{}").unwrap();
        fs::write(dir.join("video_source_manifest.json"),json!({"version":"video_source_manifest.v1","material":{"book_id":reference.book_id,"video":video,"transcript":transcript},"source":{"video":video,"imported_path":"media/original.mp4","original_path":"fixture.mp4","duration_ms":2000,"width":160,"height":90},"canonical_text_path":"source.txt","transcript_map_path":"transcript_map.json","coverage":[{"capability":"transcript","accepted":[location],"pending":[],"failed":[]}],"previous_book_id":null}).to_string()).unwrap();
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
        let mut scope = crate::run_scope::RunScope { publication:Some(Arc::new(publication)), user_id:"u".into(),workspace_id:"w".into(),
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
