use super::super::{Database, lock};
use super::row::{COLUMNS, row_to_worker_state};
use crate::lookup;
use palette_domain::worker::*;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    /// List all supervisors for a workflow.
    pub fn list_supervisors(&self, workflow_id: &WorkflowId) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let role_leader = lookup::worker_role_id(WorkerRole::Leader);
        let role_ri = lookup::worker_role_id(WorkerRole::ReviewIntegrator);
        let sql =
            format!("SELECT {COLUMNS} FROM workers WHERE workflow_id = ?1 AND role_id IN (?2, ?3)");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![workflow_id.as_ref(), role_leader, role_ri],
            row_to_worker_state,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all members for a workflow.
    pub fn list_members(&self, workflow_id: &WorkflowId) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let role_member = lookup::worker_role_id(WorkerRole::Member);
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE workflow_id = ?1 AND role_id = ?2");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![workflow_id.as_ref(), role_member],
            row_to_worker_state,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all workers (all workflows).
    pub fn list_all_workers(&self) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let sql = format!("SELECT {COLUMNS} FROM workers");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_worker_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all workers in Booting status (all workflows).
    pub fn list_booting_workers(&self) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let booting_id = lookup::worker_status_id(WorkerStatus::Booting);
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE status_id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![booting_id], row_to_worker_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all workers that are idle or waiting for permission (all workflows).
    pub fn list_idle_or_waiting_workers(&self) -> crate::Result<Vec<WorkerState>> {
        let conn = lock(&self.conn)?;
        let idle_id = lookup::worker_status_id(WorkerStatus::Idle);
        let waiting_id = lookup::worker_status_id(WorkerStatus::WaitingPermission);
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE status_id IN (?1, ?2)");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![idle_id, waiting_id], row_to_worker_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert_worker::tests::insert_test_worker;
    use crate::database::test_helpers::test_db;
    use palette_domain::worker::*;

    #[test]
    fn list_booting_workers() {
        let db = test_db();
        insert_test_worker(&db, "leader-1", WorkerRole::Leader, "wf-1");
        insert_test_worker(&db, "member-1", WorkerRole::Member, "wf-1");
        db.update_worker_status(&WorkerId::new("member-1"), WorkerStatus::Idle)
            .unwrap();

        let booting = db.list_booting_workers().unwrap();
        assert_eq!(booting.len(), 1);
        assert_eq!(booting[0].id, WorkerId::new("leader-1"));
    }
}
