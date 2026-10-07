//! Borrowed private authority plus an explicit material binding; no mutable Reader.
use crate::{user_runtime::UserRuntime, AppState};
use read_tools::Book;
use runtime::Message;
use std::path::Path;

pub struct PrivateBookContext<'a> {
    pub user: &'a UserRuntime,
    pub book: &'a Book,
    pub book_dir: &'a Path,
    pub messages: &'a [Message],
    pub selected_chat: Option<&'a str>,
}

impl AppState {
    pub fn private_context(&self) -> PrivateBookContext<'_> {
        PrivateBookContext {
            user: &self.user,
            book: &self.workspace.book,
            book_dir: &self.workspace.book_dir,
            messages: &self.workspace.messages,
            selected_chat: self.workspace.selected_chat.as_deref(),
        }
    }
}
