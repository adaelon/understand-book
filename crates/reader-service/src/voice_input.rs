//! Short dictation returns editable text. Audio lives only for this request.
use base64::{engine::general_purpose::STANDARD, Engine};
use read_tools::ToolError;
use runtime::run_context::CancellationToken;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, process::Command, sync::Mutex, time::{Duration, Instant}};

const MAX_AUDIO_BYTES: usize = 2 * 1024 * 1024;
pub const REQUEST_BYTES: usize = 3 * 1024 * 1024;
static RECOGNIZER: Mutex<()> = Mutex::new(());

fn error(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError { error_code:code.into(), category:category.into(), message:message.into() }
}
fn unavailable() -> ToolError {
    error("VOICE_NOT_CONFIGURED","unavailable","语音输入暂未启用，请先配置本地语音识别。")
}

struct Config { python: PathBuf, model: PathBuf }
impl Config {
    fn new(python: PathBuf, model: PathBuf) -> Result<Self, ToolError> {
        if (python.is_absolute() && !python.is_file())
            || !["model.bin", "config.json", "tokenizer.json"].iter().all(|file| model.join(file).is_file())
        {
            return Err(unavailable());
        }
        Ok(Self { python, model })
    }

    fn from_env() -> Result<Self, ToolError> {
        let model = std::env::var_os("UNDERSTAND_BOOK_VOICE_MODEL_DIR").map(PathBuf::from).ok_or_else(unavailable)?;
        let python = std::env::var_os("UNDERSTAND_BOOK_VOICE_PYTHON")
            .or_else(|| std::env::var_os("VIDEO_WHISPER_PYTHON")).map(PathBuf::from).unwrap_or_else(|| "python".into());
        Self::new(python, model)
    }

    fn resolve(runtime: Option<&Path>) -> Result<Self, ToolError> {
        Self::from_env().or_else(|error| {
            let runtime = runtime.ok_or(error)?;
            Self::new(runtime.join("python").join("python.exe"), runtime.join("model"))
        })
    }
}

pub fn status() -> Value {
    status_with_runtime(None)
}

pub(crate) fn status_with_runtime(runtime: Option<&Path>) -> Value {
    json!({"available": Config::resolve(runtime).is_ok(), "max_seconds": 90})
}

#[derive(Deserialize)]
struct Input {
    audio: String,
    mime_type: String,
    locale: runtime::response_language::ResponseLanguage,
}

#[derive(Deserialize, Serialize)]
struct Transcript { text: String, duration_ms: u64 }

pub struct Work { config: Config, audio: Vec<u8>, language: &'static str }

fn parse_and_validate(input: &Value) -> Result<(Vec<u8>, &'static str), ToolError> {
    let input: Input = serde_json::from_value(input.clone())
        .map_err(|_| error("VOICE_INVALID","validation","录音内容不完整，请重新录制。"))?;
    if !matches!(input.mime_type.split(';').next(),Some("audio/webm"|"audio/ogg"|"audio/mp4"|"audio/wav"|"audio/x-m4a"))
        || input.audio.len() > MAX_AUDIO_BYTES.div_ceil(3) * 4 {
        return Err(error("VOICE_INVALID","validation","请录制 90 秒以内的语音。"));
    }
    let audio = STANDARD.decode(input.audio).map_err(|_| error("VOICE_INVALID","validation","录音读取失败，请重新录制。"))?;
    if audio.is_empty() || audio.len() > MAX_AUDIO_BYTES {
        return Err(error("VOICE_INVALID","validation","录音为空或过长，请重新录制。"));
    }
    let language = match input.locale {
        runtime::response_language::ResponseLanguage::English => "en",
        runtime::response_language::ResponseLanguage::SimplifiedChinese => "zh",
    };
    Ok((audio, language))
}
impl Work {
    pub fn prepare(input: &Value) -> Result<Self, ToolError> {
        Self::prepare_with_runtime(input, None)
    }

    pub(crate) fn prepare_with_runtime(input: &Value, runtime: Option<&Path>) -> Result<Self, ToolError> {
        let (audio, language) = parse_and_validate(input)?;
        Ok(Self { config: Config::resolve(runtime)?, audio, language })
    }

    pub(crate) fn prepare_with_manager(
        input: &Value,
        manager: &crate::voice_model::VoiceModelManager,
    ) -> Result<Self, ToolError> {
        let (audio, language) = parse_and_validate(input)?;
        let resolved = manager.resolve().ok_or_else(unavailable)?;
        Ok(Self {
            config: Config {
                python: resolved.python,
                model: resolved.model_dir,
            },
            audio,
            language,
        })
    }

    pub fn execute(self, cancel: &CancellationToken) -> Result<Value, ToolError> {
        let _recognizer = RECOGNIZER.try_lock().map_err(|_| error("VOICE_BUSY","rate_limit","正在处理另一段语音，请稍后重试。"))?;
        cancel.check()?;
        let staging = tempfile::tempdir().map_err(|e| error("VOICE_FAILED","unavailable",e.to_string()))?;
        let script = staging.path().join("dictation.py");
        let audio = staging.path().join("recording");
        let output = staging.path().join("result.json");
        let save = || -> std::io::Result<()> {
            fs::write(&audio,&self.audio)?;
            fs::write(&script,include_str!("voice_input.py"))
        };
        save().map_err(|e| error("VOICE_FAILED","unavailable",e.to_string()))?;
        let mut command = Command::new(self.config.python);
        command.arg("-B").arg(&script).arg(&audio).arg(&self.config.model).arg(self.language).arg(&output)
            .env("HF_HUB_OFFLINE","1").env("PYTHONUTF8","1");
        crate::video_audio::execute(command,staging.path(),"Voice recognition",cancel,Instant::now()+Duration::from_secs(120),None)
            .map_err(|e| error("VOICE_FAILED","unavailable",e.message))?;
        let transcript: Transcript = serde_json::from_slice(&fs::read(output)
            .map_err(|e| error("VOICE_FAILED","unavailable",e.to_string()))?)
            .map_err(|_| error("VOICE_FAILED","unavailable","语音转写没有完成，可以重试这段录音。"))?;
        Ok(json!(transcript))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_unsupported_and_oversized_recordings_before_loading_a_model() {
        for (audio,mime) in [("".into(),"audio/webm"),(STANDARD.encode(b"audio"),"video/webm"),
            ("A".repeat(REQUEST_BYTES+1),"audio/webm")] {
            assert_eq!(Work::prepare(&json!({"audio":audio,"mime_type":mime,"locale":"zh-CN"})).err().unwrap().error_code,"VOICE_INVALID");
        }
    }
}
