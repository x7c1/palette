use super::AgentRole;
use super::AgentStatus;
use super::ContainerId;
use super::TmuxTarget;
use palette_db::AgentId;
use serde::{Deserialize, Serialize};

/// Serde helpers for AgentId (domain type without serde derives).
mod agent_id_serde {
    use palette_db::AgentId;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(id: &AgentId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(id.as_ref())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AgentId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(AgentId::new(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    #[serde(with = "agent_id_serde")]
    pub id: AgentId,
    pub role: AgentRole,
    #[serde(with = "agent_id_serde")]
    pub leader_id: AgentId,
    pub container_id: ContainerId,
    pub tmux_target: TmuxTarget,
    pub status: AgentStatus,
    pub session_id: Option<String>,
}
