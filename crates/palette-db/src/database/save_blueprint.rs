use super::*;
use palette_domain::blueprint::SaveBlueprintRequest;

impl Database {
    /// Save a blueprint. If a blueprint with the same task_id already exists, it is replaced.
    pub fn save_blueprint(&self, req: &SaveBlueprintRequest) -> crate::Result<()> {
        let conn = lock!(self.conn);
        let created_at = req.created_at.to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO blueprints (task_id, title, yaml, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![req.task_id, req.title, req.yaml, created_at],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use chrono::Utc;
    use palette_domain::blueprint::SaveBlueprintRequest;

    fn save_req(task_id: &str, title: &str, yaml: &str) -> SaveBlueprintRequest {
        SaveBlueprintRequest {
            task_id: task_id.to_string(),
            title: title.to_string(),
            yaml: yaml.to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn save_and_retrieve_blueprint() {
        let db = test_db();
        db.save_blueprint(&save_req(
            "2026/feature-x",
            "Add feature X",
            "task:\n  id: 2026/feature-x",
        ))
        .unwrap();

        let bp = db.get_blueprint("2026/feature-x").unwrap().unwrap();
        assert_eq!(bp.task_id, "2026/feature-x");
        assert_eq!(bp.title, "Add feature X");
    }

    #[test]
    fn save_blueprint_replaces_existing() {
        let db = test_db();
        db.save_blueprint(&save_req("2026/feature-x", "Old title", "old yaml"))
            .unwrap();
        db.save_blueprint(&save_req("2026/feature-x", "New title", "new yaml"))
            .unwrap();

        let fetched = db.get_blueprint("2026/feature-x").unwrap().unwrap();
        assert_eq!(fetched.title, "New title");
        assert_eq!(fetched.yaml, "new yaml");
    }
}
