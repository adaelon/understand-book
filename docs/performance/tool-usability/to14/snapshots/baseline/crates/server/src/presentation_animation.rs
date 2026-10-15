//! Trusted local Manim execution; temporary media becomes version-owned only on write.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use read_tools::ToolError;
use runtime::{
    presentation::{AnimationAsset, AnimationCue},
    presentation_author::PlotSize,
    run_context::CancellationToken,
};
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RenderedAnimation {
    pub asset: AnimationAsset,
    pub code: String,
    pub data: Value,
    pub frames: Vec<(f64, String)>,
}
fn failed(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "PRESENTATION_ANIMATION_FAILED".into(),
        category: "execution".into(),
        message: message.into(),
    }
}

const RUNNER: &str = include_str!("presentation_animation_runner.py");

pub(crate) fn render(
    code: String,
    data: Value,
    size: Option<PlotSize>,
    cues: Vec<AnimationCue>,
    cancellation: &CancellationToken,
) -> Result<RenderedAnimation, ToolError> {
    cancellation.check()?;
    let size = size.unwrap_or(PlotSize {
        width: 1280,
        height: 720,
    });
    if !(320..=1600).contains(&size.width)
        || !(240..=1200).contains(&size.height)
        || size.width % 2 != 0
        || size.height % 2 != 0
        || code.len() > 32 * 1024
        || data.to_string().len() > 128 * 1024
        || cues.len() > 16
    {
        return Err(failed("Animation needs even dimensions 320..1600 x 240..1200, code <=32 KiB, data <=128 KiB and <=16 cues"));
    }
    let mut ids = std::collections::HashSet::new();
    if cues.iter().any(|cue| {
        cue.id.trim().is_empty()
            || cue.label.trim().is_empty()
            || !ids.insert(&cue.id)
            || !cue.at_seconds.is_finite()
            || cue.at_seconds < 0.0
    }) {
        return Err(failed(
            "Cues need distinct nonempty ids, labels and finite nonnegative times",
        ));
    }
    let dir = tempfile::tempdir().map_err(|e| failed(e.to_string()))?;
    let write = |name: &str, text: &str| {
        fs::write(dir.path().join(name), text).map_err(|e| failed(e.to_string()))
    };
    write("run.py", RUNNER)?;
    write("scene.py", &code)?;
    write(
        "input.json",
        &json!({"data":data,"width":size.width,"height":size.height,"cues":cues}).to_string(),
    )?;
    let log = fs::File::create(dir.path().join("render.log")).map_err(|e| failed(e.to_string()))?;
    let python = std::env::var("UNDERSTAND_BOOK_ANIMATION_PYTHON").map_err(|_| ToolError {
        error_code:"PRESENTATION_ANIMATION_UNAVAILABLE".into(), category:"unavailable".into(),
        message:"Set UNDERSTAND_BOOK_ANIMATION_PYTHON to a Python environment with Manim 0.21.0/Cairo, PyAV, Pillow and required fonts/TeX; see docs/Manim-部署.md".into(),
    })?;
    let mut command = Command::new(&python);
    command
        .arg("-B")
        .arg("run.py")
        .current_dir(dir.path())
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().map_err(|e| failed(e.to_string()))?,
        ))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| failed(format!("Cannot start animation Python ({python}): {e}")))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let status = loop {
        if cancellation.is_cancelled() || Instant::now() >= deadline {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .creation_flags(0x08000000)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            cancellation.check()?;
            return Err(failed(
                "Animation exceeded 180 seconds; simplify the scene and render again",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failed(error.to_string()));
            }
        }
    };
    if !status.success() {
        let log = fs::read_to_string(dir.path().join("render.log")).unwrap_or_default();
        let tail: String = log
            .chars()
            .rev()
            .take(4000)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        return Err(failed(format!(
            "Manim render/decode failed: {}",
            tail.trim()
        )));
    }
    cancellation.check()?;
    #[derive(serde::Deserialize)]
    struct Frame { file: String, at_seconds: f64 }
    #[derive(serde::Deserialize)]
    struct Metadata { width: u32, height: u32, duration_seconds: f64, fps: f64, frames: Vec<Frame> }
    let metadata: Metadata = serde_json::from_slice(&crate::presentation_sandbox::output_file(dir.path(), "metadata.json", 8192)?)
        .map_err(|_| failed("Invalid animation metadata"))?;
    if metadata.width != size.width || metadata.height != size.height || !metadata.duration_seconds.is_finite()
        || metadata.duration_seconds <= 0.0 || metadata.duration_seconds > 180.0 || metadata.fps != 30.0
        || metadata.frames.is_empty() || metadata.frames.len() > 4
        || cues.iter().any(|cue| cue.at_seconds > metadata.duration_seconds) {
        return Err(failed("Invalid animation dimensions, duration or frames"));
    }
    let video = crate::presentation_sandbox::output_file(dir.path(), "animation.mp4", 8 * 1024 * 1024)?;
    if video.len() < 12 || &video[4..8] != b"ftyp" { return Err(failed("Invalid MP4")); }
    let mut frames = Vec::new();
    for (index, frame) in metadata.frames.iter().enumerate() {
        if frame.file != format!("frame-{index}.png") || !frame.at_seconds.is_finite()
            || frame.at_seconds < 0.0 || frame.at_seconds > metadata.duration_seconds {
            return Err(failed("Invalid animation frame"));
        }
        let png = crate::presentation_sandbox::output_file(dir.path(), &frame.file, 4 * 1024 * 1024)?;
        crate::presentation_sandbox::png(&png, size.width, size.height)?;
        frames.push((frame.at_seconds, STANDARD.encode(png)));
    }
    let asset = AnimationAsset {
        video_base64: STANDARD.encode(video),
        poster_png_base64: frames[0].1.clone(),
        width: metadata.width,
        height: metadata.height,
        duration_seconds: metadata.duration_seconds,
        fps: metadata.fps,
        cues,
    };
    Ok(RenderedAnimation {
        asset,
        code,
        data,
        frames,
    })
}

pub(crate) fn metadata(id: &str, asset: &AnimationAsset) -> Value {
    json!({"asset_ref":id,"asset_path":format!("assets/{id}.mp4"),"poster_path":format!("assets/{id}.png"),
        "code_path":format!("animations/{id}.py"),"data_path":format!("animations/{id}.json"),
        "width":asset.width,"height":asset.height,"duration_seconds":asset.duration_seconds,"fps":asset.fps,"cues":asset.cues})
}

pub(crate) fn validate_assets(
    assets: &std::collections::BTreeMap<String, AnimationAsset>,
) -> Result<(), ToolError> {
    let mut total = 0usize;
    for asset in assets.values() {
        let video = STANDARD
            .decode(&asset.video_base64)
            .map_err(|e| failed(e.to_string()))?;
        let poster = STANDARD
            .decode(&asset.poster_png_base64)
            .map_err(|e| failed(e.to_string()))?;
        if video.len() > 8 * 1024 * 1024 {
            return Err(failed("Animation MP4 exceeds 8 MiB"));
        }
        total += video.len() + poster.len();
    }
    if total > 24 * 1024 * 1024 {
        return Err(failed("Version animation media exceeds 24 MiB"));
    }
    Ok(())
}
