use super::super::{Database, lock};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    /// Delete all branch claims held by a workflow. Called when the workflow
    /// transitions to a terminal state (`Completed`, `Failed`, `Terminated`)
    /// so that its `(repo_name, work_branch)` pairs become available to a
    /// subsequent workflow.
    ///
    /// Safe to call on a workflow with no claims (no-op).
    pub fn release_workflow_branch_claims(&self, id: &WorkflowId) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "DELETE FROM workflow_branch_claims WHERE workflow_id = ?1",
            params![id.as_ref()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    #[test]
    fn releasing_non_existent_workflow_is_noop() {
        let db = test_db();
        let wf = WorkflowId::parse("wf-release-missing").unwrap();
        assert!(db.release_workflow_branch_claims(&wf).is_ok());
    }

    #[test]
    fn releases_only_the_target_workflow_claims() {
        let db = test_db();
        let wf_a = WorkflowId::parse("wf-release-1a").unwrap();
        db.create_workflow_with_branch_claims(
            &wf_a,
            "a.yaml",
            &[("x7c1/palette".into(), "feature/a".into())],
        )
        .unwrap();
        let wf_b = WorkflowId::parse("wf-release-1b").unwrap();
        db.create_workflow_with_branch_claims(
            &wf_b,
            "b.yaml",
            &[("x7c1/palette".into(), "feature/b".into())],
        )
        .unwrap();

        db.release_workflow_branch_claims(&wf_a).unwrap();

        // wf_b's claim must still block reuse of feature/b.
        let wf_c = WorkflowId::parse("wf-release-1c").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &wf_c,
                "c.yaml",
                &[("x7c1/palette".into(), "feature/b".into())],
            )
            .unwrap();
        assert_eq!(conflicts.len(), 1);

        // feature/a is free again.
        let wf_d = WorkflowId::parse("wf-release-1d").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &wf_d,
                "d.yaml",
                &[("x7c1/palette".into(), "feature/a".into())],
            )
            .unwrap();
        assert!(conflicts.is_empty());
    }
}
