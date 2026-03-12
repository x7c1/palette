use super::*;
use crate::models::stored_blueprint::StoredBlueprint;
use palette_domain::blueprint::Blueprint;

impl Database {
    /// List all stored blueprints, ordered by creation time (newest first).
    pub fn list_blueprints(&self) -> crate::Result<Vec<Blueprint>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT task_id, title, yaml, created_at FROM blueprints ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredBlueprint {
                task_id: row.get(0)?,
                title: row.get(1)?,
                yaml: row.get(2)?,
                created_at: parse_datetime(&row.get::<_, String>(3)?),
            })
        })?;

        let mut blueprints = Vec::new();
        for row in rows {
            blueprints.push(row?.into());
        }
        Ok(blueprints)
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
    fn list_blueprints_empty() {
        let db = test_db();
        let bps = db.list_blueprints().unwrap();
        assert!(bps.is_empty());
    }

    #[test]
    fn list_blueprints_returns_all() {
        let db = test_db();
        db.save_blueprint(&save_req("task-a", "Task A", "yaml-a"))
            .unwrap();
        db.save_blueprint(&save_req("task-b", "Task B", "yaml-b"))
            .unwrap();

        let bps = db.list_blueprints().unwrap();
        assert_eq!(bps.len(), 2);
    }
}
