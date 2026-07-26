use crate::fugu::FuguRuntime;
use crate::research_jobs::ResearchJobManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct AppState {
    pub(crate) fugu: Arc<FuguRuntime>,
    pub(crate) research_jobs: Arc<ResearchJobManager>,
    pub(crate) workspace_path: PathBuf,
    pub(crate) export_dir: PathBuf,
    pub(crate) save_lock: Arc<Mutex<()>>,
}
