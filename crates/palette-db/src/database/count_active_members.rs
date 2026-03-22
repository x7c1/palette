use super::*;

impl Database {
    /// Count the number of craft jobs currently in_progress (active members).
    pub fn count_active_members(&self) -> crate::Result<usize> {
        let conn = lock!(self.conn);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE type = 'craft' AND status = 'in_progress'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn count_active_members() {
        let db = test_db();
        create_craft(&db, "C-001", None);
        create_craft(&db, "C-002", None);

        assert_eq!(db.count_active_members().unwrap(), 0);

        db.assign_job(&jid("C-001"), &aid("member-a")).unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);

        db.assign_job(&jid("C-002"), &aid("member-b")).unwrap();
        assert_eq!(db.count_active_members().unwrap(), 2);

        db.update_job_status(&jid("C-001"), JobStatus::Craft(CraftStatus::InReview))
            .unwrap();
        db.update_job_status(&jid("C-001"), JobStatus::Craft(CraftStatus::Done))
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);
    }
}
