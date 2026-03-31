use palette_domain::job::MechanizedStatus;

pub fn mechanized_status_id(status: MechanizedStatus) -> i64 {
    match status {
        MechanizedStatus::Todo => 1,
        MechanizedStatus::InProgress => 2,
        MechanizedStatus::Done => 3,
        MechanizedStatus::Failed => 4,
    }
}

pub fn mechanized_status_from_id(id: i64) -> Result<MechanizedStatus, String> {
    match id {
        1 => Ok(MechanizedStatus::Todo),
        2 => Ok(MechanizedStatus::InProgress),
        3 => Ok(MechanizedStatus::Done),
        4 => Ok(MechanizedStatus::Failed),
        _ => Err(format!("invalid mechanized_status id: {id}")),
    }
}
