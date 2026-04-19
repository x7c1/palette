use super::super::{Database, lock};
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    /// Transition a workflow into [`WorkflowStatus::Failed`] with a reason key.
    ///
    /// Returns `Ok(true)` if the row transitioned, `Ok(false)` if the workflow
    /// was already in a terminal state (`Completed`, `Terminated`, or `Failed`)
    /// and was left untouched.
    pub fn mark_workflow_failed(&self, id: &WorkflowId, reason: &str) -> crate::Result<bool> {
        let conn = lock(&self.conn)?;
        let rows = conn.execute(
            "UPDATE workflows
                SET status_id = ?1, failure_reason = ?2
              WHERE id = ?3
                AND status_id NOT IN (?4, ?5, ?6)",
            params![
                crate::lookup::workflow_status_id(WorkflowStatus::Failed),
                reason,
                id.as_ref(),
                crate::lookup::workflow_status_id(WorkflowStatus::Completed),
                crate::lookup::workflow_status_id(WorkflowStatus::Terminated),
                crate::lookup::workflow_status_id(WorkflowStatus::Failed),
            ],
        )?;
        Ok(rows > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::test_db;
    use palette_domain::workflow::{WorkflowId, WorkflowStatus};

    #[test]
    fn transitions_active_workflow_to_failed() {
        let db = test_db();
        let wf_id = WorkflowId::parse("wf-fail-1").unwrap();
        db.create_workflow(&wf_id, "test/blueprint.yaml").unwrap();

        let transitioned = db
            .mark_workflow_failed(&wf_id, "workflow/workspace_setup_failed")
            .unwrap();
        assert!(transitioned);

        let wf = db.get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(wf.status, WorkflowStatus::Failed);
        assert_eq!(
            wf.failure_reason.as_deref(),
            Some("workflow/workspace_setup_failed")
        );
    }

    #[test]
    fn is_noop_on_completed_workflow() {
        let db = test_db();
        let wf_id = WorkflowId::parse("wf-fail-2").unwrap();
        db.create_workflow(&wf_id, "test/blueprint.yaml").unwrap();
        db.update_workflow_status(&wf_id, WorkflowStatus::Completed)
            .unwrap();

        let transitioned = db
            .mark_workflow_failed(&wf_id, "workflow/anything")
            .unwrap();
        assert!(!transitioned);

        let wf = db.get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(wf.status, WorkflowStatus::Completed);
        assert!(wf.failure_reason.is_none());
    }

    #[test]
    fn is_noop_on_terminated_workflow() {
        let db = test_db();
        let wf_id = WorkflowId::parse("wf-fail-3").unwrap();
        db.create_workflow(&wf_id, "test/blueprint.yaml").unwrap();
        db.update_workflow_status(&wf_id, WorkflowStatus::Terminated)
            .unwrap();

        let transitioned = db
            .mark_workflow_failed(&wf_id, "workflow/anything")
            .unwrap();
        assert!(!transitioned);

        let wf = db.get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(wf.status, WorkflowStatus::Terminated);
        assert!(wf.failure_reason.is_none());
    }

    #[test]
    fn is_noop_on_already_failed_workflow() {
        let db = test_db();
        let wf_id = WorkflowId::parse("wf-fail-4").unwrap();
        db.create_workflow(&wf_id, "test/blueprint.yaml").unwrap();
        db.mark_workflow_failed(&wf_id, "workflow/first").unwrap();

        let transitioned = db.mark_workflow_failed(&wf_id, "workflow/second").unwrap();
        assert!(!transitioned);

        let wf = db.get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(wf.status, WorkflowStatus::Failed);
        assert_eq!(wf.failure_reason.as_deref(), Some("workflow/first"));
    }

    #[test]
    fn filters_failed_workflows_via_list() {
        let db = test_db();
        let active_id = WorkflowId::parse("wf-fail-5a").unwrap();
        let failed_id = WorkflowId::parse("wf-fail-5b").unwrap();
        db.create_workflow(&active_id, "test/blueprint.yaml")
            .unwrap();
        db.create_workflow(&failed_id, "test/blueprint.yaml")
            .unwrap();
        db.mark_workflow_failed(&failed_id, "workflow/workspace_setup_failed")
            .unwrap();

        let failed = db.list_workflows(Some(WorkflowStatus::Failed)).unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, failed_id);
    }
}
