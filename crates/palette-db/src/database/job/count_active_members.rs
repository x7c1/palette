use super::super::*;

impl Database {
    /// Count the number of jobs currently in_progress (active members).
    /// Includes both craft and review jobs since both consume a container.
    pub fn count_active_members(&self) -> crate::Result<usize> {
        let conn = lock(&self.conn)?;
        // Craft InProgress = 2, Review InProgress = 7
        let craft_ip = crate::lookup::craft_status_id(palette_domain::job::CraftStatus::InProgress);
        let review_ip =
            crate::lookup::review_status_id(palette_domain::job::ReviewStatus::InProgress);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE status_id IN (?1, ?2)",
            params![craft_ip, review_ip],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn counts_craft_and_review_members() {
        let db = test_db();
        setup_worker(&db, "member-a");
        setup_worker(&db, "member-b");
        create_craft(&db, "C-001", None);
        create_review(&db, "R-001");

        assert_eq!(db.count_active_members().unwrap(), 0);

        db.assign_job(&jid("C-001"), &wid("member-a"), JobType::Craft)
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);

        db.assign_job(&jid("R-001"), &wid("member-b"), JobType::Review)
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 2);

        db.update_job_status(&jid("C-001"), JobStatus::Craft(CraftStatus::InReview))
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);

        db.update_job_status(&jid("R-001"), JobStatus::Review(ReviewStatus::Done))
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 0);
    }
}
