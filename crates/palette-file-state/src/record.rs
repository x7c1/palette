use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StateFile {
    pub session_name: String,
    pub leaders: Vec<AgentRecord>,
    pub members: Vec<AgentRecord>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub role: String,
    pub leader_id: String,
    pub container_id: String,
    pub tmux_target: String,
    pub status: String,
    pub session_id: Option<String>,
}
