use palette_domain::job::CraftStatus;

pub fn craft_status_id(status: CraftStatus) -> i64 {
    match status {
        CraftStatus::Todo => 1,
        CraftStatus::InProgress => 2,
        CraftStatus::InReview => 3,
        CraftStatus::Done => 4,
        CraftStatus::Escalated => 5,
        CraftStatus::Terminated => 24,
    }
}

pub fn craft_status_from_id(id: i64) -> Result<CraftStatus, String> {
    match id {
        1 => Ok(CraftStatus::Todo),
        2 => Ok(CraftStatus::InProgress),
        3 => Ok(CraftStatus::InReview),
        4 => Ok(CraftStatus::Done),
        5 => Ok(CraftStatus::Escalated),
        24 => Ok(CraftStatus::Terminated),
        _ => Err(format!("invalid craft_status id: {id}")),
    }
}
