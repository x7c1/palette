use palette_domain::job::Priority;

pub fn priority_id(priority: Priority) -> i64 {
    match priority {
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::Low => 3,
    }
}

pub fn priority_from_id(id: i64) -> Result<Priority, String> {
    match id {
        1 => Ok(Priority::High),
        2 => Ok(Priority::Medium),
        3 => Ok(Priority::Low),
        _ => Err(format!("invalid priority id: {id}")),
    }
}
