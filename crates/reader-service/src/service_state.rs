//! Host capabilities and shared provider/library configuration, without a current reader.
use runtime::ModelAdapter;
use std::path::PathBuf;

pub struct ServiceState {
    pub desktop_host: bool,
    pub reader_only: bool,
    /// None preserves the local host's legacy current-book-derived library root.
    pub library_root: Option<PathBuf>,
    pub adapter: Box<dyn ModelAdapter + Send>,
    /// Exact configuration selected for future local turns; credentials are excluded.
    pub provider_binding: Option<crate::run_input::ProviderBinding>,
    /// Format conversion processes started by this local service.
    pub stage_processes: crate::stage_processes::LocalStageProcesses,
}
