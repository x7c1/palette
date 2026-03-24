use rusqlite::Connection;

pub(crate) const SCHEMA: &str = r#"
-- Lookup tables

CREATE TABLE IF NOT EXISTS job_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS job_statuses (
    id INTEGER PRIMARY KEY,
    job_type_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    FOREIGN KEY (job_type_id) REFERENCES job_types(id),
    UNIQUE (job_type_id, name)
);

CREATE TABLE IF NOT EXISTS task_statuses (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS workflow_statuses (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS verdict_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS priorities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

-- Main tables

CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    blueprint_path TEXT NOT NULL,
    status_id INTEGER NOT NULL,
    worker_counter INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    FOREIGN KEY (status_id) REFERENCES workflow_statuses(id)
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    status_id INTEGER NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflows(id),
    FOREIGN KEY (status_id) REFERENCES task_statuses(id)
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    type_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    plan_path TEXT NOT NULL,
    assignee TEXT,
    status_id INTEGER NOT NULL,
    priority_id INTEGER,
    repository TEXT,
    pr_url TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    notes TEXT,
    assigned_at TEXT,
    FOREIGN KEY (task_id) REFERENCES tasks(id),
    FOREIGN KEY (type_id) REFERENCES job_types(id),
    FOREIGN KEY (status_id) REFERENCES job_statuses(id),
    FOREIGN KEY (priority_id) REFERENCES priorities(id)
);

CREATE TABLE IF NOT EXISTS review_submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_job_id TEXT NOT NULL,
    round INTEGER NOT NULL,
    verdict_id INTEGER NOT NULL,
    summary TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (review_job_id) REFERENCES jobs(id),
    FOREIGN KEY (verdict_id) REFERENCES verdict_types(id)
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

-- Indexes

CREATE INDEX IF NOT EXISTS idx_jobs_type_status ON jobs(type_id, status_id);
CREATE INDEX IF NOT EXISTS idx_review_submissions_job ON review_submissions(review_job_id);
CREATE INDEX IF NOT EXISTS idx_review_comments_submission ON review_comments(submission_id);
CREATE INDEX IF NOT EXISTS idx_message_queue_target ON message_queue(target_id, id);
CREATE INDEX IF NOT EXISTS idx_tasks_workflow ON tasks(workflow_id);
"#;

pub(crate) const SEED: &str = r#"
-- Job types
INSERT OR IGNORE INTO job_types (id, name) VALUES (1, 'craft');
INSERT OR IGNORE INTO job_types (id, name) VALUES (2, 'review');

-- Craft statuses (job_type_id = 1)
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (1, 1, 'todo');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (2, 1, 'in_progress');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (3, 1, 'in_review');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (4, 1, 'done');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (5, 1, 'escalated');

-- Review statuses (job_type_id = 2)
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (6, 2, 'todo');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (7, 2, 'in_progress');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (8, 2, 'changes_requested');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (9, 2, 'done');
INSERT OR IGNORE INTO job_statuses (id, job_type_id, name) VALUES (10, 2, 'escalated');

-- Task statuses
INSERT OR IGNORE INTO task_statuses (id, name) VALUES (1, 'pending');
INSERT OR IGNORE INTO task_statuses (id, name) VALUES (2, 'ready');
INSERT OR IGNORE INTO task_statuses (id, name) VALUES (3, 'in_progress');
INSERT OR IGNORE INTO task_statuses (id, name) VALUES (4, 'suspended');
INSERT OR IGNORE INTO task_statuses (id, name) VALUES (5, 'completed');

-- Workflow statuses
INSERT OR IGNORE INTO workflow_statuses (id, name) VALUES (1, 'active');
INSERT OR IGNORE INTO workflow_statuses (id, name) VALUES (2, 'suspended');
INSERT OR IGNORE INTO workflow_statuses (id, name) VALUES (3, 'completed');

-- Verdict types
INSERT OR IGNORE INTO verdict_types (id, name) VALUES (1, 'approved');
INSERT OR IGNORE INTO verdict_types (id, name) VALUES (2, 'changes_requested');

-- Priorities
INSERT OR IGNORE INTO priorities (id, name) VALUES (1, 'high');
INSERT OR IGNORE INTO priorities (id, name) VALUES (2, 'medium');
INSERT OR IGNORE INTO priorities (id, name) VALUES (3, 'low');
"#;

pub(crate) fn initialize(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(SEED)?;
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

        assert!(tables.contains(&"job_types".to_string()));
        assert!(tables.contains(&"job_statuses".to_string()));
        assert!(tables.contains(&"task_statuses".to_string()));
        assert!(tables.contains(&"workflow_statuses".to_string()));
        assert!(tables.contains(&"verdict_types".to_string()));
        assert!(tables.contains(&"priorities".to_string()));
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
