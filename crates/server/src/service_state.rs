//! Host capabilities and shared provider/library configuration, without a current reader.
use runtime::ModelAdapter;
use std::path::PathBuf;

pub struct ServiceState {
    pub desktop_host: bool,
    pub reader_only: bool,
    /// None preserves the local host's legacy current-book-derived library root.
    pub library_root: Option<PathBuf>,
    pub adapter: Box<dyn ModelAdapter + Send>,
}
