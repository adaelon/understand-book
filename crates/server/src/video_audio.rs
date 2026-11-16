//! Local Whisper processes a bounded original-audio clip. Publications stay read-only.
use base_schema::PublishedBookRef;
use read_tools::{Book, ToolError};
use runtime::{run_context::CancellationToken, video_audio::{AudioReview, ReviewAudio, ReviewedWord}};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};

fn failed(message: impl Into<String>) -> ToolError {
    ToolError { error_code: "VIDEO_AUDIO_REVIEW_FAILED".into(), category: "unavailable".into(), message: message.into() }
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError { error_code: "VIDEO_AUDIO_REVIEW_FAILED".into(), category: "invalid_input".into(), message: message.into() }
}

pub struct AudioConfig {
    pub python: PathBuf,
    pub model_dir: PathBuf,
}
impl AudioConfig {
    pub fn from_env() -> Result<Self, ToolError> {
        let model_dir = std::env::var_os("VIDEO_WHISPER_MODEL_DIR").map(PathBuf::from)
            .filter(|p| p.is_dir()).ok_or_else(|| failed("Set VIDEO_WHISPER_MODEL_DIR to the installed local small.en model directory"))?;
        Ok(Self { python: std::env::var_os("VIDEO_WHISPER_PYTHON").map(PathBuf::from).unwrap_or_else(|| "python".into()), model_dir })
    }
}

pub(crate) fn review_for_run(scope: &crate::run_scope::RunScope, request: ReviewAudio, cancel: &CancellationToken) -> Result<AudioReview, ToolError> {
    review_for_run_metered(scope,request,cancel,None)
}

pub(crate) fn review_for_run_metered(scope: &crate::run_scope::RunScope, request: ReviewAudio, cancel: &CancellationToken,
    spend: Option<&crate::model_spend_store::ModelSpendStore>) -> Result<AudioReview, ToolError> {
    let declared: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(scope.book_dir.join("publication.json")).map_err(|e| failed(e.to_string()))?
    ).map_err(|e| failed(e.to_string()))?;
    if scope.publication.as_ref().is_some_and(|p| p.reference != declared.reference) { return Err(failed("Run publication mismatch")); }
    review_audio_metered(&scope.reading_book(), &scope.book_dir, &declared.reference, &scope.turn_id, request, cancel, &AudioConfig::from_env()?,spend.map(|spend|(spend,scope)))
}

pub(crate) fn execute(mut command: Command, staging: &Path, phase: &str, cancel: &CancellationToken, deadline: Instant, started: Option<&mut bool>) -> Result<(), ToolError> {
    cancel.check()?;
    let log_path = staging.join(format!("{phase}.log"));
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(fs::File::create(&log_path).map_err(|e| failed(e.to_string()))?));
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| failed(format!("Cannot start {phase}: {e}")))?;
    if let Some(started) = started { *started = true; }
    drop(command);
    loop {
        if cancel.is_cancelled() || Instant::now() >= deadline {
            let _ = child.kill(); let _ = child.wait(); cancel.check()?;
            return Err(failed(format!("{phase} exceeded the 120 second audio-review deadline")));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() { return Ok(()); }
                let log = fs::read_to_string(&log_path).unwrap_or_default();
                return Err(failed(format!("{phase} failed: {}", log.chars().rev().take(1200).collect::<String>().chars().rev().collect::<String>())));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => { let _=child.kill(); let _=child.wait(); return Err(failed(e.to_string())); }
        }
    }
}

pub fn review_audio(book: &Book, directory: &Path, publication: &PublishedBookRef, run_id: &str, request: ReviewAudio,
    cancel: &CancellationToken, config: &AudioConfig) -> Result<AudioReview, ToolError> {
    review_audio_metered(book,directory,publication,run_id,request,cancel,config,None)
}

pub(crate) fn review_audio_metered(book: &Book, directory: &Path, publication: &PublishedBookRef, run_id: &str, request: ReviewAudio,
    cancel: &CancellationToken, config: &AudioConfig, metering: Option<(&crate::model_spend_store::ModelSpendStore,&crate::run_scope::RunScope)>) -> Result<AudioReview, ToolError> {
    cancel.check()?;
    let declared: crate::published_library::PublicationManifest = serde_json::from_slice(
        &fs::read(directory.join("publication.json")).map_err(|e| failed(e.to_string()))?
    ).map_err(|e| failed(e.to_string()))?;
    if declared.reference != *publication { return Err(failed("Publication identity mismatch")); }
    let (manifest, map) = book.load_video_source(directory)?;
    manifest.validate_publication(publication).map_err(failed)?;
    manifest.source.validate_location(&request.location).map_err(invalid)?;
    let duration_ms = request.location.end_ms - request.location.start_ms;
    if !(1.0..=30_000.0).contains(&duration_ms) {
        return Err(invalid(format!("location spans {duration_ms} ms; request a 1–30000 ms audio range with 1 <= end_ms - start_ms <= 30000. Split longer audio into shorter reviews.")));
    }
    if request.question.trim().is_empty() {
        return Err(invalid("question is blank; provide the spoken word or phrase you want to check."));
    }
    let question_length = request.question.chars().count();
    if question_length > 2000 {
        return Err(invalid(format!("question has {question_length} characters; shorten it to at most 2000 characters.")));
    }
    let staging = tempfile::tempdir().map_err(|e| failed(e.to_string()))?;
    let audio = staging.path().join("original.wav");
    let mut decode = Command::new(crate::video_media::executable("ffmpeg"));
    decode.args(["-v","error","-ss",&(request.location.start_ms / 1000.0).to_string(),"-i"])
        .arg(directory.join(&manifest.source.imported_path))
        .args(["-t",&(duration_ms / 1000.0).to_string(),"-map","0:a:0","-vn","-ac","1","-ar","16000","-c:a","pcm_s16le","-y"])
        .arg(&audio);
    let deadline = Instant::now() + Duration::from_secs(120);
    execute(decode, staging.path(), "FFmpeg", cancel, deadline,None)?;
    // Embed the existing VI3 adapter so execution does not depend on a repository-relative script.
    let script = staging.path().join("whisper.py");
    fs::write(&script, include_str!("../../../packages/core/src/video-whisper.py")).map_err(|e| failed(e.to_string()))?;
    let input = staging.path().join("input.json");
    let output = staging.path().join("result.json");
    // No question, saved transcript or visible text is supplied as a recognition prompt.
    fs::write(&input, json!({"audio":audio,"model_dir":config.model_dir}).to_string()).map_err(|e| failed(e.to_string()))?;
    let mut recognize = Command::new(&config.python);
    recognize.arg(script).arg(input).arg(&output);
    let usage_id=metering.map(|(spend,scope)|spend.service_admit_run(scope,duration_ms.ceil() as i64,crate::multi_user_host::now())).transpose()?;
    let mut started=false;
    let recognized=(|| {
        execute(recognize, staging.path(), "Whisper", cancel, deadline,Some(&mut started))?;
        let raw: Value=serde_json::from_slice(&fs::read(output).map_err(|e|failed(e.to_string()))?).map_err(|e|failed(e.to_string()))?;
        let actual=raw["info"]["duration"].as_f64().filter(|v|v.is_finite() && *v > 0.0).ok_or_else(||failed("Missing decoded audio duration"))? * 1000.0;
        if (actual-duration_ms).abs()>1.0 { return Err(failed("Decoded audio does not cover the requested range")); }
        Ok((raw,actual))
    })();
    if let (Some((spend,scope)),Some(usage_id))=(metering,usage_id) {
        let quantity=recognized.as_ref().ok().map(|(_,ms)|(*ms).round().min(duration_ms.ceil()) as i64);
        let status=if recognized.is_ok() {"completed"} else if started {"unknown"} else {"not_started"};
        spend.service_receipt(&scope.turn_id,crate::service_usage::ServiceReceipt {usage_id,status:status.into(),
            quantity_ms:if status=="not_started" {Some(0)} else {quantity},
            evidence:json!({"source":"video.review_audio","publication":publication,"start_ms":request.location.start_ms,
                "end_ms":request.location.end_ms,"decoded_duration_ms":recognized.as_ref().ok().map(|(_,ms)|ms)})},crate::multi_user_host::now())?;
    }
    let (raw,_)=recognized?;
    cancel.check()?;
    let mut review = project(publication, request, map.segments, &raw)?;
    review.run_id = run_id.into();
    cancel.check()?;
    Ok(review)
}

fn project(publication: &PublishedBookRef, request: ReviewAudio, transcript: Vec<base_schema::TranscriptSegment>, raw: &Value) -> Result<AudioReview, ToolError> {
    let duration = raw["info"]["duration"].as_f64().filter(|d| d.is_finite() && *d > 0.0).ok_or_else(|| failed("Missing decoded audio duration"))? * 1000.0;
    if (duration - (request.location.end_ms - request.location.start_ms)).abs() > 1.0 { return Err(failed("Decoded audio does not cover the requested range")); }
    let segments = raw["segments"].as_array().ok_or_else(|| failed("Missing recognition segments"))?;
    let mut words = Vec::new();
    let mut texts = Vec::new();
    let mut uncertainties = vec!["机器音频复核候选；词时间为识别估计，概率不等于正确性，未消除的听辨疑点仍须回听。".into()];
    for segment in segments {
        texts.push(segment["text"].as_str().ok_or_else(|| failed("Missing recognized text"))?.trim());
        for word in segment["words"].as_array().ok_or_else(|| failed("Missing word estimates"))? {
            let number = |name: &str| word[name].as_f64().filter(|n| n.is_finite()).ok_or_else(|| failed("Invalid word estimate"));
            let start = number("start")? * 1000.0; let end = number("end")? * 1000.0; let probability = number("probability")?;
            if start < 0.0 || end < start || end > duration + 1.0 || !(0.0..=1.0).contains(&probability) { return Err(failed("Word estimate outside processed audio")); }
            let text = word["word"].as_str().ok_or_else(|| failed("Missing word"))?.to_string();
            if probability < 0.5 { uncertainties.push(format!("低把握词 {}（{:.0}–{:.0} ms），请回听核对。", text.trim(), request.location.start_ms + start, request.location.start_ms + end)); }
            words.push(ReviewedWord { text, start_ms: (request.location.start_ms + start).min(request.location.end_ms), end_ms: (request.location.start_ms + end).min(request.location.end_ms), probability });
        }
    }
    let text = texts.join(" ");
    if text.is_empty() { uncertainties.push("本次未识别出措辞；不能据此断言整段静音。".into()); }
    Ok(AudioReview { run_id: String::new(), review_result_ref: uuid::Uuid::now_v7().to_string(), publication: publication.clone(),
        transcript: transcript.into_iter().filter(|s| s.location.start_ms < request.location.end_ms && request.location.start_ms < s.location.end_ms).collect(),
        location: request.location, question: request.question, reviewed_text: text, reviewed_words: words, uncertainties,
        provider: json!({"engine":raw["engine"],"engine_version":raw["engine_version"],"model":raw["model"],"language":raw["language"],
            "load_ms":raw["load_ms"],"inference_ms":raw["inference_ms"],"audio_duration_ms":duration,"audio_input":"original_pcm_s16le_16000_mono","recognition_prompt":null}) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ReviewAudio {
        serde_json::from_value(json!({"location":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":578000,"end_ms":586000},"question":"underflow or overflow?"})).unwrap()
    }
    #[test]
    fn video_audio_original_offsets_uncertainties_and_empty_recognition() {
        let publication=PublishedBookRef {book_id:"b".into(),publication_id:"p".into()};
        let raw=json!({"info":{"duration":8},"segments":[{"text":" underflow overflow", "words":[
            {"word":" underflow","start":1,"end":1.5,"probability":0.95},
            {"word":" overflow","start":2,"end":2,"probability":0.3}]}]});
        let review=project(&publication,request(),vec![],&raw).unwrap();
        assert_eq!(review.reviewed_words[0].start_ms,579000.0);
        assert_eq!(review.reviewed_words[1].end_ms,580000.0);
        assert_eq!(review.uncertainties.len(),2);
        assert!(review.uncertainties[1].contains("overflow"));
        let empty=project(&publication,request(),vec![],&json!({"info":{"duration":8},"segments":[]})).unwrap();
        assert!(empty.uncertainties.iter().any(|s|s.contains("静音")));
        let mut bad=raw.clone();bad["segments"][0]["words"][0]["end"]=json!(9);
        assert!(project(&publication,request(),vec![],&bad).is_err());
        bad=raw.clone();bad["info"]["duration"]=json!(7);
        assert!(project(&publication,request(),vec![],&bad).is_err());
    }
    #[test]
    fn video_audio_missing_audio_bounds_publication_and_cancel() {
        let dir=tempfile::tempdir().unwrap();
        let (book,publication,frames)=crate::video_frames::tests::fixture(dir.path());
        let config=AudioConfig {python:"unavailable-python-for-test".into(),model_dir:dir.path().into()};
        let request=ReviewAudio {location:frames.location,question:"what was said?".into()};
        let read=|r,p,c|review_audio(&book,dir.path(),p,"run",r,c,&config);
        let cancel=CancellationToken::default();
        let error=read(request.clone(),&publication,&cancel).unwrap_err();
        assert!(error.message.contains("FFmpeg failed"),"{}",error.message);
        let mut invalid=request.clone();invalid.question=" ".into();
        assert!(read(invalid,&publication,&cancel).unwrap_err().message.contains("question"));
        let mut invalid=request.clone();invalid.location.end_ms=31000.0;
        assert!(read(invalid,&publication,&cancel).is_err());
        let wrong=PublishedBookRef {publication_id:"other".into(),..publication.clone()};
        assert!(read(request.clone(),&wrong,&cancel).unwrap_err().message.contains("identity"));
        cancel.cancel();
        assert_eq!(read(request,&publication,&cancel).unwrap_err().error_code,"AGENT_RUN_CANCELLED");
    }
    #[test]
    fn video_tool_audio_input_errors_are_repairable() {
        let dir = tempfile::tempdir().unwrap();
        let (book, publication, frames) = crate::video_frames::tests::fixture(dir.path());
        let config = AudioConfig { python: "unavailable-python-for-test".into(), model_dir: dir.path().into() };
        let request = ReviewAudio { location: frames.location, question: "what was said?".into() };
        let read = |r| review_audio(&book, dir.path(), &publication, "run", r, &CancellationToken::default(), &config);
        let mut bad = request.clone(); bad.location.end_ms = 0.5;
        let error = read(bad).unwrap_err();
        assert_eq!(error.category, "invalid_input");
        assert!(error.message.contains("0.5") && error.message.contains("1–30000"), "{}", error.message);
        for question in [" ".to_string(), "x".repeat(2001)] {
            let mut bad = request.clone(); bad.question = question;
            let error = read(bad).unwrap_err();
            assert_eq!(error.category, "invalid_input");
            assert!(error.message.contains("question") && !error.message.contains("audio range"), "{}", error.message);
        }
        let mut bad = request.clone(); bad.location.end_ms = 2001.0;
        let error = read(bad).unwrap_err();
        assert_eq!(error.category, "invalid_input");
        assert!(error.message.contains("duration_ms=2000"), "{}", error.message);
        let error = read(request).unwrap_err();
        assert_eq!(error.category, "unavailable");
        assert!(error.message.contains("FFmpeg failed"), "{}", error.message);
    }

    #[test]
    fn video_tool_audio_duration_boundaries_and_advertised_example() {
        let dir = tempfile::tempdir().unwrap();
        let (book, publication, frames) = crate::video_frames::tests::fixture_with_duration(dir.path(), 31);
        let config = AudioConfig { python: "unavailable-python-for-test".into(), model_dir: dir.path().into() };
        let request = ReviewAudio { location: frames.location, question: "x".repeat(2000) };
        let read = |r| review_audio(&book, dir.path(), &publication, "boundaries", r, &CancellationToken::default(), &config);
        let error = read(request.clone()).unwrap_err();
        assert_eq!(error.category, "invalid_input");
        assert!(error.message.contains("31000") && error.message.contains("30000"));
        // This fixture deliberately has no audio: admissible inputs reach the decoder.
        for end in [1.0, 30000.0] {
            let mut valid = request.clone(); valid.location.end_ms = end;
            let error = read(valid).unwrap_err();
            assert_eq!(error.category, "unavailable");
            assert!(error.message.contains("FFmpeg failed"), "{}", error.message);
        }
        for example in runtime::video_audio::spec().parameters["examples"].as_array().unwrap() {
            let error = read(serde_json::from_value(example.clone()).unwrap()).unwrap_err();
            assert!(error.message.contains("FFmpeg failed"), "{}", error.message);
        }
    }

    #[test]
    fn video_audio_process_timeout_and_cancellation_reap_child() {
        let staging=tempfile::tempdir().unwrap();
        let python=std::env::var("VIDEO_WHISPER_PYTHON").unwrap_or_else(|_| "python".into());
        let command=|| {let mut c=Command::new(&python);c.args(["-c","import time; time.sleep(20)"]);c};
        let cancel=CancellationToken::default();
        assert!(execute(command(),staging.path(),"timeout",&cancel,Instant::now(),None).unwrap_err().message.contains("deadline"));
        let token=cancel.clone();
        let worker=std::thread::spawn(move|| {std::thread::sleep(Duration::from_millis(100));token.cancel();});
        assert_eq!(execute(command(),staging.path(),"cancel",&cancel,Instant::now()+Duration::from_secs(10),None).unwrap_err().error_code,"AGENT_RUN_CANCELLED");
        worker.join().unwrap();
    }
}
