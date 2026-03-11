use super::*;
use crate::models::StoredBlueprint;

impl Database {
    /// Save a blueprint. If a blueprint with the same task_id already exists, it is replaced.
    pub fn save_blueprint(
        &self,
        task_id: &str,
        title: &str,
        yaml: &str,
    ) -> crate::Result<StoredBlueprint> {
        let conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO blueprints (task_id, title, yaml, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, title, yaml, now_str],
        )?;

        Ok(StoredBlueprint {
            task_id: task_id.to_string(),
            title: title.to_string(),
            yaml: yaml.to_string(),
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    #[test]
    fn save_and_retrieve_blueprint() {
        let db = test_db();
        let bp = db
            .save_blueprint(
                "2026/feature-x",
                "Add feature X",
                "task:\n  id: 2026/feature-x",
            )
            .unwrap();
        assert_eq!(bp.task_id, "2026/feature-x");
        assert_eq!(bp.title, "Add feature X");
    }

    #[test]
    fn save_blueprint_replaces_existing() {
        let db = test_db();
        db.save_blueprint("2026/feature-x", "Old title", "old yaml")
            .unwrap();
        let bp = db
            .save_blueprint("2026/feature-x", "New title", "new yaml")
            .unwrap();
        assert_eq!(bp.title, "New title");

        let fetched = db.get_blueprint("2026/feature-x").unwrap().unwrap();
        assert_eq!(fetched.yaml, "new yaml");
    }
}
