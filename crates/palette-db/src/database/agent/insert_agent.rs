use super::super::Database;
use crate::lookup;
use palette_domain::agent::*;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

/// Request to insert a new agent.
pub struct InsertAgentRequest {
    pub id: AgentId,
    pub workflow_id: WorkflowId,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub supervisor_id: AgentId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub session_id: Option<AgentSessionId>,
    pub task_id: TaskId,
}

impl Database {
    /// Insert a new agent record.
    pub fn insert_agent(&self, req: &InsertAgentRequest) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO agents (id, workflow_id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                req.id.as_ref(),
                req.workflow_id.as_ref(),
                lookup::agent_role_id(req.role),
                lookup::agent_status_id(req.status),
                req.supervisor_id.as_ref(),
                req.container_id.as_ref(),
                req.terminal_target.as_ref(),
                req.session_id.as_ref().map(|s| s.as_ref()),
                req.task_id.as_ref(),
                now,
                now,
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    pub fn insert_test_agent(db: &Database, id: &str, role: AgentRole, workflow_id: &str) {
        let wf_id = WorkflowId::new(workflow_id);
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        db.insert_agent(&InsertAgentRequest {
            id: AgentId::new(id),
            workflow_id: wf_id,
            role,
            status: AgentStatus::Booting,
            supervisor_id: AgentId::new(""),
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            session_id: None,
            task_id: TaskId::new(format!("task-{id}")),
        })
        .unwrap();
    }

    #[test]
    fn insert_and_find_agent() {
        let db = test_db();
        insert_test_agent(&db, "leader-1", AgentRole::Leader, "wf-1");

        let agent = db.find_agent(&AgentId::new("leader-1")).unwrap().unwrap();
        assert_eq!(agent.id, AgentId::new("leader-1"));
        assert_eq!(agent.role, AgentRole::Leader);
        assert_eq!(agent.status, AgentStatus::Booting);
    }
}
