use super::super::*;

impl Database {
    /// Count the number of active workers (all records in the workers table).
    ///
    /// Every worker that has a row in the table is considered active,
    /// regardless of its status, because the row is removed on destroy.
    pub fn count_active_workers(&self) -> crate::Result<usize> {
        let conn = lock(&self.conn)?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM workers", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::*;
    use crate::InsertWorkerRequest;
    use palette_domain::task::TaskId;
    use palette_domain::terminal::TerminalTarget;
    use palette_domain::worker::*;
    use palette_domain::workflow::WorkflowId;

    fn insert_worker(
        db: &super::super::super::Database,
        id: &str,
        role: WorkerRole,
        status: WorkerStatus,
    ) {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        db.insert_worker(&InsertWorkerRequest {
            id: WorkerId::new(id),
            workflow_id: wf_id,
            role,
            status,
            supervisor_id: None,
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            session_id: None,
            task_id: TaskId::parse(format!("wf-test:{id}")).unwrap(),
        })
        .unwrap();
    }

    #[test]
    fn returns_zero_when_no_workers() {
        let db = test_db();
        assert_eq!(db.count_active_workers().unwrap(), 0);
    }

    #[test]
    fn counts_all_worker_statuses() {
        let db = test_db();

        insert_worker(&db, "w-booting", WorkerRole::Member, WorkerStatus::Booting);
        assert_eq!(db.count_active_workers().unwrap(), 1);

        insert_worker(&db, "w-working", WorkerRole::Member, WorkerStatus::Working);
        assert_eq!(db.count_active_workers().unwrap(), 2);

        insert_worker(&db, "w-idle", WorkerRole::Member, WorkerStatus::Idle);
        assert_eq!(db.count_active_workers().unwrap(), 3);

        insert_worker(
            &db,
            "w-waiting",
            WorkerRole::Member,
            WorkerStatus::WaitingPermission,
        );
        assert_eq!(db.count_active_workers().unwrap(), 4);

        insert_worker(&db, "w-crashed", WorkerRole::Member, WorkerStatus::Crashed);
        assert_eq!(db.count_active_workers().unwrap(), 5);
    }

    #[test]
    fn counts_both_members_and_supervisors() {
        let db = test_db();

        insert_worker(&db, "member-1", WorkerRole::Member, WorkerStatus::Working);
        insert_worker(&db, "leader-1", WorkerRole::Leader, WorkerStatus::Working);
        insert_worker(
            &db,
            "review-integrator-1",
            WorkerRole::ReviewIntegrator,
            WorkerStatus::Working,
        );
        assert_eq!(db.count_active_workers().unwrap(), 3);
    }

    #[test]
    fn removed_worker_not_counted() {
        let db = test_db();

        insert_worker(&db, "w-1", WorkerRole::Member, WorkerStatus::Working);
        assert_eq!(db.count_active_workers().unwrap(), 1);

        db.remove_worker(&WorkerId::new("w-1")).unwrap();
        assert_eq!(db.count_active_workers().unwrap(), 0);
    }
}
