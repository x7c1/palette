use super::*;

impl Database {
    /// Count the number of work tasks currently in_progress (active members).
    pub fn count_active_members(&self) -> crate::Result<usize> {
        let conn = lock!(self.conn);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE type = 'work' AND status = 'in_progress'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::task::*;

    #[test]
    fn count_active_members() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        create_work(&db, "W-002", None, vec![]);

        assert_eq!(db.count_active_members().unwrap(), 0);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.assign_task(&tid("W-001"), &aid("member-a")).unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);

        db.update_task_status(&tid("W-002"), TaskStatus::Ready)
            .unwrap();
        db.assign_task(&tid("W-002"), &aid("member-b")).unwrap();
        assert_eq!(db.count_active_members().unwrap(), 2);

        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::Done)
            .unwrap();
        assert_eq!(db.count_active_members().unwrap(), 1);
    }
}
