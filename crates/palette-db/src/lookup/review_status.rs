use palette_domain::job::ReviewStatus;

pub fn review_status_id(status: ReviewStatus) -> i64 {
    match status {
        ReviewStatus::Todo => 6,
        ReviewStatus::InProgress => 7,
        ReviewStatus::ChangesRequested => 8,
        ReviewStatus::Done => 9,
        ReviewStatus::Escalated => 10,
    }
}

pub fn review_status_from_id(id: i64) -> Result<ReviewStatus, String> {
    match id {
        6 => Ok(ReviewStatus::Todo),
        7 => Ok(ReviewStatus::InProgress),
        8 => Ok(ReviewStatus::ChangesRequested),
        9 => Ok(ReviewStatus::Done),
        10 => Ok(ReviewStatus::Escalated),
        _ => Err(format!("invalid review_status id: {id}")),
    }
}
