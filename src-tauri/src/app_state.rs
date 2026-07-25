use crate::fugu::FuguRuntime;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct AppState {
    pub(crate) fugu: Arc<FuguRuntime>,
    pub(crate) workspace_path: PathBuf,
    pub(crate) export_dir: PathBuf,
    pub(crate) save_lock: Mutex<()>,
}
