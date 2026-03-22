use serde::{Deserialize, Serialize};

/// JSON representation of the orchestrator's runtime state (active agents and their containers).
/// Persisted to disk so the server can resume agent management after restart.
#[derive(Debug, Serialize, Deserialize)]
pub struct StateFile {
    pub session_name: String,
    pub supervisors: Vec<AgentRecord>,
    pub members: Vec<AgentRecord>,
    #[serde(default)]
    pub member_counter: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub role: String,
    pub supervisor_id: String,
    pub container_id: String,
    pub terminal_target: String,
    pub status: String,
    pub session_id: Option<String>,
}
