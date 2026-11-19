use super::*;
use crate::run_scope::RunScope;

/// Save through the same private writer as a personal connection. The Reader supplies the
/// original chat, book, revision and observed offsets from the current ACP run.
pub(crate) fn save(
    state: &mut AppState,
    scope: &RunScope,
    source: (String, Option<memory::TextRange>),
    args: &Value,
    save_question: bool,
    now: &str,
) -> Result<Value> {
    let root = dir(state)?.to_path_buf();
    if closed(state)? {
        return Err(denied("请先离开闭卷现场再保存带原文的记录"));
    }
    let grant = current_grant(state);
    let request = json!({
        "source_ref": PassageRef {
            book_id: scope.book_id().to_owned(),
            source_revision: scope.reading_book().source_fingerprint().into(),
            lid: source.0,
            range: source.1,
        },
        "text":field(args,"text")?,
        "operation_id":format!("acp:{}:{}",scope.turn_id,field(args,"operation_id")?),
        "chat_session_id":scope.chat_session_id,
    });
    let name = if save_question {
        "task_save"
    } else {
        "note_add"
    };
    commands::call(
        state,
        &grant,
        &root,
        name,
        &request,
        false,
        now,
        Some(scope),
    )
}
