//! Mutable reading scene. Private history remains owned by UserRuntime.
use crate::{agent_stream::RunStream, AgentHistory};
use read_tools::Book;
use reader::Reader;
use runtime::Message;
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
};

pub struct ReaderWorkspace {
    pub publication: Option<Arc<crate::published_library::PublishedBook>>,
    pub id: String,
    pub generation: u64,
    pub selected_chat: Option<String>,
    pub book_dir: PathBuf,
    pub book: Arc<Book>,
    pub reader: Reader,
    pub messages: Vec<Message>,
    pub session_path: Option<PathBuf>,
    pub workbench_loaded_revision: Option<String>,
    pub active_agent_stream: Option<Weak<RunStream>>,
}

impl ReaderWorkspace {
    pub fn local(
        book_dir: PathBuf,
        book: Arc<Book>,
        reader: Reader,
        messages: Vec<Message>,
        session_path: Option<PathBuf>,
    ) -> Self {
        Self {
            publication: None,
            id: "local".into(),
            generation: 0,
            selected_chat: None,
            book_dir,
            book,
            reader,
            messages,
            session_path,
            workbench_loaded_revision: None,
            active_agent_stream: None,
        }
    }

    pub fn bind_publication(&mut self, publication: Arc<crate::published_library::PublishedBook>) {
        self.invalidate();
        self.reader = Reader::new(&publication.book, reader::DEFAULT_RADIUS);
        self.book = publication.book.clone();
        self.book_dir = publication.directory.clone();
        self.publication = Some(publication);
        self.selected_chat = None;
        self.messages.clear();
        self.workbench_loaded_revision = None;
    }

    pub fn localization_cache_path(&self) -> Option<PathBuf> {
        self.publication.as_ref().map(|p| p.localization_cache.clone())
            .or_else(|| crate::paper_minimap_localization_cache_path(&self.session_path))
    }

    pub fn select_chat(&mut self, id: String, messages: Vec<Message>) {
        if self.selected_chat.as_ref() != Some(&id) {
            self.invalidate();
            self.selected_chat = Some(id);
        }
        self.messages = messages;
    }

    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.active_agent_stream = None;
    }

    /// The legacy field is only a local resume hint, never a live selection.
    pub fn restore_chat(&mut self, history: &mut AgentHistory, now: &str) {
        let index = crate::ensure_active_agent_session(history, &self.book.base.book_id, now);
        let session = &history.sessions[index];
        self.select_chat(session.id.clone(), session.messages.clone());
    }
}
