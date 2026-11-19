//! Browser commands borrow the same user authority and the explicitly authorized scene.
use crate::*;

pub use reader_service::memory_commands::{reply, read, memory};

pub(crate) fn history_with_library(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace,
    library: &std::sync::Mutex<published_library::PublishedLibrary>, owner: &str) -> Result<Value, ToolError> {
    let Some(selected) = workspace.selected_chat.as_deref() else {
        return read(user, workspace, "chat/history", &Value::Null, "").unwrap();
    };
    let session = user.agent_history.sessions.iter().find(|s|s.id==selected
        && s.matches_scope(workspace.learning_workspace_id.as_deref(), &workspace.book.base.book_id))
        .ok_or_else(authorization::missing)?;
    let mut response = agent_history_response(&user.agent_history, &workspace.book, Some(selected))?;
    for (saved, view) in session.turns.iter().zip(response.current.turns.iter_mut()) {
        if let Some(reference) = &saved.published_book_ref {
            if let Ok(publication) = library.lock().unwrap().load(owner, reference) {
                *view = turn_view(&publication.book, saved);
            }
        }
    }
    Ok(json!(response))
}

