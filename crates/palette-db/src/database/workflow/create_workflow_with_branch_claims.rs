use super::super::{Database, lock};
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

/// Repo+work-branch pair that another active workflow already owns.
///
/// Returned by [`Database::create_workflow_with_branch_claims`] when at least
/// one incoming claim collides with an existing active claim; the caller
/// translates this into a 400 response with reason `workflow/work_branch_in_use`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkBranchConflict {
    pub repo_name: String,
    pub work_branch: String,
}

impl Database {
    /// Insert a new `workflows` row together with its `(repo_name, work_branch)`
    /// claims as a single atomic transaction.
    ///
    /// Returns the list of conflicting claims when the insert would violate the
    /// UNIQUE constraint on active claims; on conflict the workflow row is not
    /// created. An empty Vec signals the workflow was created successfully.
    pub fn create_workflow_with_branch_claims(
        &self,
        id: &WorkflowId,
        blueprint_path: &str,
        claims: &[(String, String)],
    ) -> crate::Result<Vec<WorkBranchConflict>> {
        let mut conn = lock(&self.conn)?;
        let tx = conn.transaction()?;

        // Pre-check conflicts so we can return all of them in one response
        // rather than failing on the first UNIQUE violation. The UNIQUE
        // constraint remains as a safety net for any future concurrent path.
        let mut conflicts = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT 1 FROM workflow_branch_claims
                  WHERE repo_name = ?1 AND work_branch = ?2",
            )?;
            for (repo_name, work_branch) in claims {
                let exists = stmt
                    .query_row(params![repo_name, work_branch], |_| Ok(()))
                    .is_ok();
                if exists {
                    conflicts.push(WorkBranchConflict {
                        repo_name: repo_name.clone(),
                        work_branch: work_branch.clone(),
                    });
                }
            }
        }
        if !conflicts.is_empty() {
            // Rolling back an empty transaction is a no-op but keeps intent
            // explicit.
            tx.rollback()?;
            return Ok(conflicts);
        }

        let now = chrono::Utc::now();
        tx.execute(
            "INSERT INTO workflows (id, blueprint_path, status_id, started_at)
                  VALUES (?1, ?2, ?3, ?4)",
            params![
                id.as_ref(),
                blueprint_path,
                crate::lookup::workflow_status_id(WorkflowStatus::Active),
                now.to_rfc3339(),
            ],
        )?;

        for (repo_name, work_branch) in claims {
            tx.execute(
                "INSERT INTO workflow_branch_claims (workflow_id, repo_name, work_branch)
                      VALUES (?1, ?2, ?3)",
                params![id.as_ref(), repo_name, work_branch],
            )?;
        }

        tx.commit()?;
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    fn claim(repo: &str, branch: &str) -> (String, String) {
        (repo.to_string(), branch.to_string())
    }

    #[test]
    fn inserts_workflow_and_claims_atomically() {
        let db = test_db();
        let wf = WorkflowId::parse("wf-claim-1").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &wf,
                "blueprint.yaml",
                &[claim("x7c1/palette", "feature/a")],
            )
            .unwrap();
        assert!(conflicts.is_empty());

        let stored = db.get_workflow(&wf).unwrap();
        assert!(stored.is_some(), "workflow row should exist");
    }

    #[test]
    fn returns_conflict_for_existing_active_claim() {
        let db = test_db();
        let first = WorkflowId::parse("wf-claim-2a").unwrap();
        db.create_workflow_with_branch_claims(
            &first,
            "first.yaml",
            &[claim("x7c1/palette", "feature/shared")],
        )
        .unwrap();

        let second = WorkflowId::parse("wf-claim-2b").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &second,
                "second.yaml",
                &[claim("x7c1/palette", "feature/shared")],
            )
            .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].repo_name, "x7c1/palette");
        assert_eq!(conflicts[0].work_branch, "feature/shared");

        // The second workflow row must not have been created.
        let stored = db.get_workflow(&second).unwrap();
        assert!(
            stored.is_none(),
            "conflicting workflow must not be inserted"
        );
    }

    #[test]
    fn reports_multiple_conflicts_at_once() {
        let db = test_db();
        let existing_a = WorkflowId::parse("wf-claim-3a").unwrap();
        db.create_workflow_with_branch_claims(
            &existing_a,
            "a.yaml",
            &[claim("x7c1/palette", "feature/a")],
        )
        .unwrap();
        let existing_b = WorkflowId::parse("wf-claim-3b").unwrap();
        db.create_workflow_with_branch_claims(
            &existing_b,
            "b.yaml",
            &[claim("x7c1/palette", "feature/b")],
        )
        .unwrap();

        let incoming = WorkflowId::parse("wf-claim-3c").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &incoming,
                "c.yaml",
                &[
                    claim("x7c1/palette", "feature/a"),
                    claim("x7c1/palette", "feature/b"),
                    claim("x7c1/palette", "feature/c"),
                ],
            )
            .unwrap();
        assert_eq!(conflicts.len(), 2);
        assert!(
            conflicts
                .iter()
                .any(|c| c.work_branch == "feature/a" && c.repo_name == "x7c1/palette")
        );
        assert!(
            conflicts
                .iter()
                .any(|c| c.work_branch == "feature/b" && c.repo_name == "x7c1/palette")
        );
    }

    #[test]
    fn released_claims_allow_reuse() {
        let db = test_db();
        let first = WorkflowId::parse("wf-claim-4a").unwrap();
        db.create_workflow_with_branch_claims(
            &first,
            "first.yaml",
            &[claim("x7c1/palette", "feature/reuse")],
        )
        .unwrap();
        db.release_workflow_branch_claims(&first).unwrap();

        let second = WorkflowId::parse("wf-claim-4b").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(
                &second,
                "second.yaml",
                &[claim("x7c1/palette", "feature/reuse")],
            )
            .unwrap();
        assert!(
            conflicts.is_empty(),
            "reusing a released branch must succeed"
        );
    }

    #[test]
    fn empty_claim_list_still_creates_workflow() {
        let db = test_db();
        let wf = WorkflowId::parse("wf-claim-5").unwrap();
        let conflicts = db
            .create_workflow_with_branch_claims(&wf, "blueprint.yaml", &[])
            .unwrap();
        assert!(conflicts.is_empty());
        assert!(db.get_workflow(&wf).unwrap().is_some());
    }
}
