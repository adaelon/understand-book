//! One turn's temporary Matplotlib result. The author decides whether to attach it to a version.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use read_tools::ToolError;
use runtime::{presentation_author::PlotSize, run_context::CancellationToken};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct PlotAsset {
    pub svg: String,
    pub png_base64: String,
    pub code: String,
    pub data: Value,
    pub width: u32,
    pub height: u32,
    pub font: String,
}

fn failed(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "PRESENTATION_PLOT_FAILED".into(),
        category: "execution".into(),
        message: message.into(),
    }
}

const RUNNER: &str = r#"
import json
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib import font_manager

with open('data.json', encoding='utf-8') as source:
    data = json.load(source)
with open('size.json', encoding='utf-8') as source:
    width, height = json.load(source)

names = {entry.name for entry in font_manager.fontManager.ttflist}
font = next((name for name in ('Microsoft YaHei', 'Noto Sans CJK SC', 'Noto Sans CJK', 'WenQuanYi Micro Hei') if name in names), None)
with open('plot.py', encoding='utf-8') as source:
    code = source.read()
if font is None and any('\u4e00' <= char <= '\u9fff' for char in code + json.dumps(data, ensure_ascii=False)):
    raise RuntimeError('A CJK font is required for Chinese plot labels (Microsoft YaHei or Noto Sans CJK SC)')
font = font or 'DejaVu Sans'
plt.rcParams.update({
    'font.family': font, 'font.size': 12, 'axes.linewidth': 1.0,
    'axes.prop_cycle': plt.cycler(color=['#3c648f', '#c36b43', '#458467', '#8b6aa3', '#b88a34']),
    'figure.facecolor': '#ffffff', 'axes.facecolor': '#ffffff',
    'savefig.facecolor': '#ffffff', 'svg.fonttype': 'none',
})
fig, ax = plt.subplots(figsize=(width / 100, height / 100), dpi=100, layout='constrained')
exec(compile(code, 'plot.py', 'exec'))
fig.savefig('plot.svg', format='svg')
fig.savefig('preview.png', format='png', dpi=100)
with open('font.txt', 'w', encoding='utf-8') as target:
    target.write(font)
plt.close(fig)
"#;

pub(crate) fn render(
    code: String,
    data: Value,
    size: Option<PlotSize>,
    cancellation: &CancellationToken,
) -> Result<PlotAsset, ToolError> {
    cancellation.check()?;
    let size = size.unwrap_or(PlotSize {
        width: 800,
        height: 480,
    });
    if !(320..=1600).contains(&size.width)
        || !(240..=1200).contains(&size.height)
        || code.len() > 32 * 1024
        || serde_json::to_vec(&data)
            .map_err(|e| failed(e.to_string()))?
            .len()
            > 128 * 1024
    {
        return Err(failed(
            "Plot size, code or data exceeds the authoring limit",
        ));
    }
    let dir = tempfile::tempdir().map_err(|e| failed(e.to_string()))?;
    fs::write(dir.path().join("run.py"), RUNNER).map_err(|e| failed(e.to_string()))?;
    fs::write(dir.path().join("plot.py"), &code).map_err(|e| failed(e.to_string()))?;
    fs::write(
        dir.path().join("data.json"),
        serde_json::to_vec(&data).map_err(|e| failed(e.to_string()))?,
    )
    .map_err(|e| failed(e.to_string()))?;
    fs::write(
        dir.path().join("size.json"),
        format!("[{},{}]", size.width, size.height),
    )
    .map_err(|e| failed(e.to_string()))?;
    let stderr =
        fs::File::create(dir.path().join("stderr.txt")).map_err(|e| failed(e.to_string()))?;
    let python = std::env::var("UNDERSTAND_BOOK_PLOT_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
    let mut child = Command::new(&python)
        .arg("-B")
        .arg("run.py")
        .current_dir(dir.path())
        .env("MPLCONFIGDIR", dir.path().join("matplotlib"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| failed(format!("Cannot start plot Python ({python}): {e}")))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if cancellation.is_cancelled() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            cancellation.check()?;
            return Err(failed("Plot exceeded 20 seconds"));
        }
        match child.try_wait().map_err(|e| failed(e.to_string()))? {
            Some(status) => break status,
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    if !status.success() {
        let mut message = String::new();
        fs::File::open(dir.path().join("stderr.txt"))
            .map_err(|e| failed(e.to_string()))?
            .take(4096)
            .read_to_string(&mut message)
            .map_err(|e| failed(e.to_string()))?;
        return Err(failed(format!("Python plot error: {}", message.trim())));
    }
    let svg = String::from_utf8(crate::presentation_sandbox::output_file(dir.path(), "plot.svg", 512 * 1024)?)
        .map_err(|_| failed("Invalid SVG encoding"))?;
    if !svg.contains("<svg") { return Err(failed("Matplotlib did not produce an SVG")); }
    let png = crate::presentation_sandbox::output_file(dir.path(), "preview.png", 1024 * 1024)?;
    crate::presentation_sandbox::png(&png, size.width, size.height)?;
    let png_base64 = STANDARD.encode(png);
    let font = String::from_utf8(crate::presentation_sandbox::output_file(dir.path(), "font.txt", 256)?)
        .map_err(|_| failed("Invalid font metadata"))?;
    Ok(PlotAsset {
        svg,
        png_base64,
        code,
        data,
        width: size.width,
        height: size.height,
        font,
    })
}
