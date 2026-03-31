use crate::job::{CraftStatus, JobStatus, ReviewStatus, TransitionError};

/// Validate a craft status transition.
pub fn validate_craft_transition(
    from: CraftStatus,
    to: CraftStatus,
) -> Result<(), TransitionError> {
    let valid = matches!(
        (from, to),
        (CraftStatus::Todo, CraftStatus::InProgress)
            | (CraftStatus::InProgress, CraftStatus::InReview)
            | (CraftStatus::InReview, CraftStatus::Done)
            | (CraftStatus::InReview, CraftStatus::InProgress) // changes_requested
            | (_, CraftStatus::Escalated)
    );

    if !valid {
        return Err(TransitionError::invalid(from, to));
    }

    Ok(())
}

/// Validate a review status transition.
pub fn validate_review_transition(
    from: ReviewStatus,
    to: ReviewStatus,
) -> Result<(), TransitionError> {
    let valid = matches!(
        (from, to),
        (ReviewStatus::Todo, ReviewStatus::InProgress)
            | (ReviewStatus::InProgress, ReviewStatus::Done)
            | (ReviewStatus::InProgress, ReviewStatus::ChangesRequested)
            | (ReviewStatus::ChangesRequested, ReviewStatus::InProgress) // re-review
            | (_, ReviewStatus::Escalated)
    );

    if !valid {
        return Err(TransitionError::invalid(from, to));
    }

    Ok(())
}

/// Validate a job status transition, dispatching by job type.
pub fn validate_transition(from: JobStatus, to: JobStatus) -> Result<(), TransitionError> {
    match (from, to) {
        (JobStatus::Craft(f), JobStatus::Craft(t)) => validate_craft_transition(f, t),
        (JobStatus::Review(f), JobStatus::Review(t)) => validate_review_transition(f, t),
        _ => Err(TransitionError::invalid(from, to)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_craft_transitions() {
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::InProgress).is_ok());
        assert!(validate_craft_transition(CraftStatus::InProgress, CraftStatus::InReview).is_ok());
        assert!(validate_craft_transition(CraftStatus::InReview, CraftStatus::Done).is_ok());
        assert!(validate_craft_transition(CraftStatus::InReview, CraftStatus::InProgress).is_ok());
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::Escalated).is_ok());
    }

    #[test]
    fn invalid_craft_transitions() {
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::Done).is_err());
        assert!(validate_craft_transition(CraftStatus::Todo, CraftStatus::InReview).is_err());
        assert!(validate_craft_transition(CraftStatus::Done, CraftStatus::Todo).is_err());
    }

    #[test]
    fn valid_review_transitions() {
        assert!(validate_review_transition(ReviewStatus::Todo, ReviewStatus::InProgress).is_ok());
        assert!(validate_review_transition(ReviewStatus::InProgress, ReviewStatus::Done).is_ok());
        assert!(
            validate_review_transition(ReviewStatus::InProgress, ReviewStatus::ChangesRequested)
                .is_ok()
        );
        assert!(
            validate_review_transition(ReviewStatus::ChangesRequested, ReviewStatus::InProgress)
                .is_ok()
        );
    }

    #[test]
    fn invalid_review_transitions() {
        assert!(validate_review_transition(ReviewStatus::Todo, ReviewStatus::Done).is_err());
        assert!(validate_review_transition(ReviewStatus::Done, ReviewStatus::Todo).is_err());
        assert!(
            validate_review_transition(ReviewStatus::Todo, ReviewStatus::ChangesRequested).is_err()
        );
    }

    #[test]
    fn cross_type_transition_is_invalid() {
        assert!(
            validate_transition(
                JobStatus::Craft(CraftStatus::InProgress),
                JobStatus::Review(ReviewStatus::InProgress),
            )
            .is_err()
        );
    }
}
