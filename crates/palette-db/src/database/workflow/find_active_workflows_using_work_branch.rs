use super::super::{Database, corrupt_parse, lock};
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use rusqlite::params;

impl Database {
    /// Return non-terminal workflows (`Active`, `Suspending`, or `Suspended`)
    /// whose task tree contains a Craft job targeting `(repo_name, work_branch)`.
    ///
    /// The branch identity check uses `json_extract` against the
    /// `jobs.repository` JSON column; only Craft jobs populate that field
    /// (see [`RepositoryRow`]).
    pub fn find_active_workflows_using_work_branch(
        &self,
        repo_name: &str,
        work_branch: &str,
    ) -> crate::Result<Vec<WorkflowId>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT w.id
               FROM workflows w
               JOIN tasks t ON t.workflow_id = w.id
               JOIN jobs j ON j.task_id = t.id
              WHERE j.repository IS NOT NULL
                AND json_extract(j.repository, '$.name') = ?1
                AND json_extract(j.repository, '$.work_branch') = ?2
                AND w.status_id IN (?3, ?4, ?5)",
        )?;
        let rows = stmt.query_map(
            params![
                repo_name,
                work_branch,
                crate::lookup::workflow_status_id(WorkflowStatus::Active),
                crate::lookup::workflow_status_id(WorkflowStatus::Suspending),
                crate::lookup::workflow_status_id(WorkflowStatus::Suspended),
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut out = Vec::new();
        for raw in rows {
            let id_str = raw?;
            let id = WorkflowId::parse(&id_str).map_err(corrupt_parse)?;
            out.push(id);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::test_db;
    use crate::database::CreateTaskRequest;
    use palette_domain::job::{CreateJobRequest, JobDetail, PlanPath, Repository, Title};
    use palette_domain::task::TaskId;
    use palette_domain::workflow::{WorkflowId, WorkflowStatus};

    fn craft(name: &str, work_branch: &str, source_branch: Option<&str>) -> JobDetail {
        JobDetail::Craft {
            repository: Repository::parse(name, work_branch, source_branch.map(String::from))
                .unwrap(),
        }
    }

    fn seed_workflow_with_craft(
        db: &crate::database::Database,
        workflow_id: &str,
        task_id: &str,
        craft_detail: JobDetail,
    ) {
        let wf_id = WorkflowId::parse(workflow_id).unwrap();
        db.create_workflow(&wf_id, "test/blueprint.yaml").unwrap();
        let task = TaskId::parse(task_id).unwrap();
        db.create_task(&CreateTaskRequest {
            id: task.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();
        db.create_job(&CreateJobRequest::new(
            task,
            Title::parse("craft").unwrap(),
            Some(PlanPath::parse("plan/x").unwrap()),
            None,
            None,
            craft_detail,
        ))
        .unwrap();
    }

    #[test]
    fn returns_active_workflow_that_uses_work_branch() {
        let db = test_db();
        seed_workflow_with_craft(
            &db,
            "wf-branch-1",
            "wf-branch-1:task-c",
            craft("x7c1/palette", "feature/foo", None),
        );
        let found = db
            .find_active_workflows_using_work_branch("x7c1/palette", "feature/foo")
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].as_ref(), "wf-branch-1");
    }

    #[test]
    fn ignores_different_repo_or_work_branch() {
        let db = test_db();
        seed_workflow_with_craft(
            &db,
            "wf-branch-2",
            "wf-branch-2:task-c",
            craft("x7c1/palette", "feature/bar", None),
        );
        assert!(
            db.find_active_workflows_using_work_branch("x7c1/palette", "feature/foo")
                .unwrap()
                .is_empty()
        );
        assert!(
            db.find_active_workflows_using_work_branch("x7c1/other", "feature/bar")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn ignores_terminal_workflows() {
        let db = test_db();
        seed_workflow_with_craft(
            &db,
            "wf-branch-3a",
            "wf-branch-3a:task-c",
            craft("x7c1/palette", "feature/foo", None),
        );
        let wf_id = WorkflowId::parse("wf-branch-3a").unwrap();
        db.update_workflow_status(&wf_id, WorkflowStatus::Completed)
            .unwrap();
        assert!(
            db.find_active_workflows_using_work_branch("x7c1/palette", "feature/foo")
                .unwrap()
                .is_empty()
        );

        seed_workflow_with_craft(
            &db,
            "wf-branch-3b",
            "wf-branch-3b:task-c",
            craft("x7c1/palette", "feature/foo", None),
        );
        let wf_id = WorkflowId::parse("wf-branch-3b").unwrap();
        db.mark_workflow_failed(&wf_id, "workflow/workspace_setup_failed")
            .unwrap();
        assert!(
            db.find_active_workflows_using_work_branch("x7c1/palette", "feature/foo")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn includes_suspended_workflows() {
        let db = test_db();
        seed_workflow_with_craft(
            &db,
            "wf-branch-4",
            "wf-branch-4:task-c",
            craft("x7c1/palette", "feature/foo", None),
        );
        let wf_id = WorkflowId::parse("wf-branch-4").unwrap();
        db.update_workflow_status(&wf_id, WorkflowStatus::Suspended)
            .unwrap();
        let found = db
            .find_active_workflows_using_work_branch("x7c1/palette", "feature/foo")
            .unwrap();
        assert_eq!(found.len(), 1);
    }
}
