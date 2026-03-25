use palette_domain::agent::AgentRole;

pub fn agent_role_id(role: AgentRole) -> i64 {
    match role {
        AgentRole::Leader => 1,
        AgentRole::ReviewIntegrator => 2,
        AgentRole::Member => 3,
    }
}

pub fn agent_role_from_id(id: i64) -> Result<AgentRole, String> {
    match id {
        1 => Ok(AgentRole::Leader),
        2 => Ok(AgentRole::ReviewIntegrator),
        3 => Ok(AgentRole::Member),
        _ => Err(format!("unknown agent_role id: {id}")),
    }
}
