use super::error::track_error;
use super::types::AdminCleanupPlan;
use crate::Interactor;
use crate::maintenance::error::AdminMaintenanceError;
use std::path::Path;

impl Interactor {
    pub fn admin_plan_reset(
        &self,
        data_dir: &Path,
    ) -> Result<AdminCleanupPlan, AdminMaintenanceError> {
        let workflow_ids = self
            .data_store
            .list_workflows(None)
            .map_err(track_error)?
            .into_iter()
            .map(|w| w.id)
            .collect::<Vec<_>>();
        self.gather_cleanup_plan(&workflow_ids, data_dir)
    }
}
