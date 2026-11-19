//! Native board rehearsal using the packaged Reader BoardCanvas renderer.
use memory::boards::BoardDocument;
use read_tools::ToolError;
use runtime::{
    presentation_preview::{PresentationPreviewPort, PreviewReport, PreviewRequest, PreviewViewport},
    run_context::CancellationToken,
};
use std::{path::PathBuf, sync::RwLock};

// A server process serves one installed Reader bundle. Desktop supplies its
// packaged resource directory; network hosting uses UNDERSTAND_BOOK_WEB_DIST.
static WEB_DIST: RwLock<Option<PathBuf>> = RwLock::new(None);

pub(crate) fn set_web_dist(path: PathBuf) {
    *WEB_DIST.write().unwrap_or_else(|e| e.into_inner()) = Some(path);
}

fn unavailable(message: impl Into<String>) -> ToolError {
    ToolError { error_code: "BOARD_PREVIEW_UNAVAILABLE".into(), category: "unavailable".into(), message: message.into() }
}

fn assemble(board: &BoardDocument, candidate_item_ids: &[String]) -> Result<String, ToolError> {
    let root = WEB_DIST.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| {
        std::env::var_os("UNDERSTAND_BOOK_WEB_DIST").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/web/dist"))
    }).join("board-preview");
    let script = std::fs::read_to_string(root.join("renderer.js"))
        .map_err(|e| unavailable(format!("Reader board preview renderer is missing at {}: {e}. Build the Reader web resources before authoring native boards.", root.display())))?;
    let css = std::fs::read_to_string(root.join("renderer.css"))
        .map_err(|e| unavailable(format!("Reader board preview stylesheet is unavailable: {e}")))?;
    let data = serde_json::to_string(board).map_err(|e| unavailable(e.to_string()))?
        .replace('<', "\\u003c");
    let candidate_ids = serde_json::to_string(candidate_item_ids).map_err(|e| unavailable(e.to_string()))?
        .replace('<', "\\u003c");
    Ok(format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>{}</style><body><div id=\"app\"></div><script>window.__boardPreviewDocument={data};window.__boardPreviewCandidateItemIds={candidate_ids};</script><script>{}</script></body></html>",
        css.replace("</style", "<\\/style"), script.replace("</script", "<\\/script")))
}

pub(crate) fn preview(
    candidate_id: &str,
    board: &BoardDocument,
    candidate_item_ids: &[String],
    viewport: Option<PreviewViewport>,
    cancellation: &CancellationToken,
) -> Result<PreviewReport, ToolError> {
    cancellation.check()?;
    let html = assemble(board, candidate_item_ids)?;
    crate::presentation_preview::BrowserPreview::discover().map_err(unavailable)?.preview(
        &PreviewRequest { candidate_id: candidate_id.into(), html, actions: vec![], width: None, viewport, read_selector: None },
        cancellation,
    ).map_err(|e| ToolError { error_code: "BOARD_PREVIEW_FAILED".into(), category: "unavailable".into(), message: format!("{}: {}", e.phase, e.message) })
}
