use rusqlite::Connection;

pub(crate) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('craft', 'review')),
    title TEXT NOT NULL,
    plan_path TEXT NOT NULL,
    description TEXT,
    assignee TEXT,
    status TEXT NOT NULL,
    priority TEXT CHECK(priority IN ('high', 'medium', 'low') OR priority IS NULL),
    repository TEXT,
    pr_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    notes TEXT,
    assigned_at TEXT,
    FOREIGN KEY (task_id) REFERENCES tasks(id)
);

CREATE TABLE IF NOT EXISTS review_submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_job_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    verdict TEXT NOT NULL CHECK(verdict IN ('approved', 'changes_requested')),
    summary TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (review_job_id) REFERENCES jobs(id)
);

CREATE TABLE IF NOT EXISTS review_comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    submission_id INTEGER NOT NULL,
    file TEXT NOT NULL,
    line INTEGER NOT NULL,
    body TEXT NOT NULL,
    FOREIGN KEY (submission_id) REFERENCES review_submissions(id)
);

CREATE TABLE IF NOT EXISTS message_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    target_id TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_type_status ON jobs(type, status);
CREATE INDEX IF NOT EXISTS idx_review_submissions_job ON review_submissions(review_job_id);
CREATE INDEX IF NOT EXISTS idx_review_comments_submission ON review_comments(submission_id);
CREATE INDEX IF NOT EXISTS idx_message_queue_target ON message_queue(target_id, id);

CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    blueprint_path TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'suspended', 'completed')),
    started_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'ready', 'in_progress', 'suspended', 'done')),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

CREATE INDEX IF NOT EXISTS idx_tasks_workflow ON tasks(workflow_id);
"#;

pub(crate) fn initialize(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"jobs".to_string()));
        assert!(tables.contains(&"review_submissions".to_string()));
        assert!(tables.contains(&"review_comments".to_string()));
        assert!(tables.contains(&"message_queue".to_string()));
        assert!(tables.contains(&"workflows".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
    }

    #[test]
    fn schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        initialize(&conn).unwrap();
    }
}
