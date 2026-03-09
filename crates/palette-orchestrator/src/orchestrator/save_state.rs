use super::Orchestrator;
use palette_domain::PersistentState;
use std::sync::Arc;

impl Orchestrator {
    pub(super) fn save_state(this: &Arc<Self>, infra: &PersistentState) {
        let path = std::path::PathBuf::from(&this.state_path);
        if let Err(e) = palette_file_state::save(infra, &path) {
            tracing::error!(error = %e, "failed to save state");
        }
    }
}
