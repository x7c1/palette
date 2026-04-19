use super::error::track_error;
use super::types::AdminCleanupPlan;
use crate::Interactor;
use crate::maintenance::error::AdminMaintenanceError;
use chrono::{Duration, Utc};
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use std::path::Path;

pub struct AdminGcOptions {
    pub workflow_ids: Vec<WorkflowId>,
    pub include_active: bool,
    pub older_than_hours: Option<i64>,
}

impl Interactor {
    pub fn admin_plan_gc(
        &self,
        data_dir: &Path,
        options: &AdminGcOptions,
    ) -> Result<AdminCleanupPlan, AdminMaintenanceError> {
        let workflow_ids = if !options.workflow_ids.is_empty() {
            options.workflow_ids.clone()
        } else {
            let threshold = options
                .older_than_hours
                .map(|h| Utc::now() - Duration::hours(h));
            self.data_store
                .list_workflows(None)
                .map_err(track_error)?
                .into_iter()
                .filter(|wf| {
                    matches!(
                        wf.status,
                        WorkflowStatus::Suspended
                            | WorkflowStatus::Completed
                            | WorkflowStatus::Terminated
                            | WorkflowStatus::Failed
                    ) || (options.include_active
                        && matches!(
                            wf.status,
                            WorkflowStatus::Active | WorkflowStatus::Suspending
                        ))
                })
                .filter(|wf| threshold.is_none_or(|t| wf.started_at <= t))
                .map(|wf| wf.id)
                .collect::<Vec<_>>()
        };
        self.gather_cleanup_plan(&workflow_ids, data_dir)
    }
}
