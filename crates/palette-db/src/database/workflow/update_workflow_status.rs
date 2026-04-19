use super::super::{Database, lock};
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    /// Update the workflow's status. When transitioning to a terminal state
    /// (`Completed`, `Terminated`, or `Failed`), any active branch claims are
    /// released in the same transaction so the freed `(repo_name,
    /// work_branch)` pairs become available to new workflows.
    pub fn update_workflow_status(
        &self,
        id: &WorkflowId,
        status: WorkflowStatus,
    ) -> crate::Result<()> {
        let mut conn = lock(&self.conn)?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE workflows SET status_id = ?1 WHERE id = ?2",
            params![crate::lookup::workflow_status_id(status), id.as_ref()],
        )?;
        if is_terminal(status) {
            tx.execute(
                "DELETE FROM workflow_branch_claims WHERE workflow_id = ?1",
                params![id.as_ref()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn is_terminal(status: WorkflowStatus) -> bool {
    matches!(
        status,
        WorkflowStatus::Completed | WorkflowStatus::Terminated | WorkflowStatus::Failed
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    #[test]
    fn transition_to_completed_releases_branch_claims() {
        let db = test_db();
        let wf = WorkflowId::parse("wf-update-1").unwrap();
        db.create_workflow_with_branch_claims(
            &wf,
            "blueprint.yaml",
            &[("x7c1/palette".into(), "feature/one".into())],
        )
        .unwrap();

        db.update_workflow_status(&wf, WorkflowStatus::Completed)
            .unwrap();

        // Reusing the branch on a new workflow must succeed.
        let wf_next = WorkflowId::parse("wf-update-1b").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &wf_next,
                "next.yaml",
                &[("x7c1/palette".into(), "feature/one".into())],
            )
            .unwrap();
        assert!(conflicts.is_empty());
    }

    #[test]
    fn transition_to_non_terminal_preserves_claims() {
        let db = test_db();
        let wf = WorkflowId::parse("wf-update-2").unwrap();
        db.create_workflow_with_branch_claims(
            &wf,
            "blueprint.yaml",
            &[("x7c1/palette".into(), "feature/two".into())],
        )
        .unwrap();

        db.update_workflow_status(&wf, WorkflowStatus::Suspending)
            .unwrap();
        db.update_workflow_status(&wf, WorkflowStatus::Suspended)
            .unwrap();

        // Branch stays claimed while the workflow is suspended.
        let wf_next = WorkflowId::parse("wf-update-2b").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &wf_next,
                "next.yaml",
                &[("x7c1/palette".into(), "feature/two".into())],
            )
            .unwrap();
        assert_eq!(conflicts.len(), 1);
    }
}
