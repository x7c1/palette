use crate::models::*;
use crate::schema;
use anyhow::{Context as _, bail};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create db directory: {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_task(&self, req: &CreateTaskRequest) -> anyhow::Result<Task> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let id = req.id.clone().unwrap_or_else(|| {
            let prefix = req
                .task_type
                .as_str()
                .chars()
                .next()
                .unwrap()
                .to_uppercase();
            let suffix = &uuid::Uuid::new_v4().as_simple().to_string()[..8];
            format!("{prefix}-{suffix}")
        });

        let repos_json = req
            .repositories
            .as_ref()
            .map(|r| serde_json::to_string(r).unwrap());

        conn.execute(
            "INSERT INTO tasks (id, type, title, description, assignee, status, priority, repositories, branch, pr_url, created_at, updated_at, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11, NULL)",
            params![
                id,
                req.task_type.as_str(),
                req.title,
                req.description,
                req.assignee,
                TaskStatus::Todo.as_str(),
                req.priority.map(|p| p.as_str()),
                repos_json,
                req.branch,
                now,
                now,
            ],
        )
        .context("failed to insert task")?;

        for dep in &req.depends_on {
            conn.execute(
                "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
                params![id, dep],
            )
            .with_context(|| format!("failed to insert dependency: {id} -> {dep}"))?;
        }

        drop(conn);
        self.get_task(&id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after insert"))
    }

    pub fn get_task(&self, id: &str) -> anyhow::Result<Option<Task>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, assignee, status, priority, repositories, branch, pr_url, created_at, updated_at, notes
             FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| Ok(row_to_task(row)))?;
        match rows.next() {
            Some(Ok(task)) => Ok(Some(task)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn list_tasks(&self, filter: &TaskFilter) -> anyhow::Result<Vec<Task>> {
        let conn = self.conn.lock().unwrap();
        let mut sql = "SELECT id, type, title, description, assignee, status, priority, repositories, branch, pr_url, created_at, updated_at, notes FROM tasks WHERE 1=1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref t) = filter.task_type {
            param_values.push(Box::new(t.as_str().to_string()));
            sql.push_str(&format!(" AND type = ?{}", param_values.len()));
        }
        if let Some(ref s) = filter.status {
            param_values.push(Box::new(s.as_str().to_string()));
            sql.push_str(&format!(" AND status = ?{}", param_values.len()));
        }
        if let Some(ref a) = filter.assignee {
            param_values.push(Box::new(a.clone()));
            sql.push_str(&format!(" AND assignee = ?{}", param_values.len()));
        }
        sql.push_str(" ORDER BY created_at");

        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(row_to_task(row)))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn update_task_status(&self, id: &str, status: TaskStatus) -> anyhow::Result<Task> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id],
        )?;
        if updated == 0 {
            bail!("task not found: {id}");
        }
        drop(conn);
        self.get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after update"))
    }

    pub fn get_dependencies(&self, task_id: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;
        let rows = stmt.query_map(params![task_id], |row| row.get::<_, String>(0))?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    pub fn get_dependents(&self, depends_on_id: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT task_id FROM dependencies WHERE depends_on = ?1")?;
        let rows = stmt.query_map(params![depends_on_id], |row| row.get::<_, String>(0))?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    /// Find review tasks that depend on the given work task.
    pub fn find_reviews_for_work(&self, work_id: &str) -> anyhow::Result<Vec<Task>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.branch, t.pr_url, t.created_at, t.updated_at, t.notes
             FROM tasks t
             JOIN dependencies d ON d.task_id = t.id
             WHERE d.depends_on = ?1 AND t.type = 'review'",
        )?;
        let rows = stmt.query_map(params![work_id], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    /// Find work tasks that a review task depends on.
    pub fn find_works_for_review(&self, review_id: &str) -> anyhow::Result<Vec<Task>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.branch, t.pr_url, t.created_at, t.updated_at, t.notes
             FROM tasks t
             JOIN dependencies d ON d.depends_on = t.id
             WHERE d.task_id = ?1 AND t.type = 'work'",
        )?;
        let rows = stmt.query_map(params![review_id], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn submit_review(
        &self,
        review_task_id: &str,
        req: &SubmitReviewRequest,
    ) -> anyhow::Result<ReviewSubmission> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Determine round number
        let round: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(round), 0) FROM review_submissions WHERE review_task_id = ?1",
                params![review_task_id],
                |row| row.get(0),
            )
            .unwrap_or(0)
            + 1;

        conn.execute(
            "INSERT INTO review_submissions (review_task_id, round, verdict, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                review_task_id,
                round,
                req.verdict.as_str(),
                req.summary,
                now
            ],
        )?;

        let submission_id = conn.last_insert_rowid();

        for comment in &req.comments {
            conn.execute(
                "INSERT INTO review_comments (submission_id, file, line, body)
                 VALUES (?1, ?2, ?3, ?4)",
                params![submission_id, comment.file, comment.line, comment.body],
            )?;
        }

        Ok(ReviewSubmission {
            id: submission_id,
            review_task_id: review_task_id.to_string(),
            round,
            verdict: req.verdict,
            summary: req.summary.clone(),
            created_at: now,
        })
    }

    pub fn get_review_submissions(
        &self,
        review_task_id: &str,
    ) -> anyhow::Result<Vec<ReviewSubmission>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, review_task_id, round, verdict, summary, created_at
             FROM review_submissions WHERE review_task_id = ?1 ORDER BY round",
        )?;
        let rows = stmt.query_map(params![review_task_id], |row| {
            Ok(ReviewSubmission {
                id: row.get(0)?,
                review_task_id: row.get(1)?,
                round: row.get(2)?,
                verdict: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(Verdict::Approved),
                summary: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut submissions = Vec::new();
        for row in rows {
            submissions.push(row?);
        }
        Ok(submissions)
    }

    pub fn get_review_comments(&self, submission_id: i64) -> anyhow::Result<Vec<ReviewComment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, submission_id, file, line, body FROM review_comments WHERE submission_id = ?1",
        )?;
        let rows = stmt.query_map(params![submission_id], |row| {
            Ok(ReviewComment {
                id: row.get(0)?,
                submission_id: row.get(1)?,
                file: row.get(2)?,
                line: row.get(3)?,
                body: row.get(4)?,
            })
        })?;
        let mut comments = Vec::new();
        for row in rows {
            comments.push(row?);
        }
        Ok(comments)
    }
}

fn row_to_task(row: &rusqlite::Row) -> Task {
    let repos_str: Option<String> = row.get(7).unwrap();
    let repositories: Option<Vec<String>> = repos_str.and_then(|s| serde_json::from_str(&s).ok());

    Task {
        id: row.get(0).unwrap(),
        task_type: row.get::<_, String>(1).unwrap().parse().unwrap(),
        title: row.get(2).unwrap(),
        description: row.get(3).unwrap(),
        assignee: row.get(4).unwrap(),
        status: row.get::<_, String>(5).unwrap().parse().unwrap(),
        priority: row
            .get::<_, Option<String>>(6)
            .unwrap()
            .and_then(|s| s.parse().ok()),
        repositories,
        branch: row.get(8).unwrap(),
        pr_url: row.get(9).unwrap(),
        created_at: row.get(10).unwrap(),
        updated_at: row.get(11).unwrap(),
        notes: row.get(12).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get_task() {
        let db = test_db();
        let task = db
            .create_task(&CreateTaskRequest {
                id: Some("W-001".to_string()),
                task_type: TaskType::Work,
                title: "Implement feature".to_string(),
                description: Some("Details".to_string()),
                assignee: Some("member-a".to_string()),
                priority: Some(Priority::High),
                repositories: Some(vec!["palette".to_string()]),
                branch: Some("feature/test".to_string()),
                depends_on: vec![],
            })
            .unwrap();

        assert_eq!(task.id, "W-001");
        assert_eq!(task.task_type, TaskType::Work);
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.priority, Some(Priority::High));

        let fetched = db.get_task("W-001").unwrap().unwrap();
        assert_eq!(fetched.title, "Implement feature");
    }

    #[test]
    fn create_task_with_dependencies() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work task".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review task".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec!["W-001".to_string()],
        })
        .unwrap();

        let deps = db.get_dependencies("R-001").unwrap();
        assert_eq!(deps, vec!["W-001"]);

        let dependents = db.get_dependents("W-001").unwrap();
        assert_eq!(dependents, vec!["R-001"]);
    }

    #[test]
    fn list_tasks_with_filter() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work 1".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review 1".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        let all = db
            .list_tasks(&TaskFilter {
                task_type: None,
                status: None,
                assignee: None,
            })
            .unwrap();
        assert_eq!(all.len(), 2);

        let works = db
            .list_tasks(&TaskFilter {
                task_type: Some(TaskType::Work),
                status: None,
                assignee: None,
            })
            .unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].id, "W-001");
    }

    #[test]
    fn update_task_status() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        let updated = db
            .update_task_status("W-001", TaskStatus::InProgress)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
    }

    #[test]
    fn submit_and_get_review() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec!["W-001".to_string()],
        })
        .unwrap();

        let sub = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: Some("Needs fixes".to_string()),
                    comments: vec![ReviewCommentInput {
                        file: "src/main.rs".to_string(),
                        line: 10,
                        body: "Fix this".to_string(),
                    }],
                },
            )
            .unwrap();
        assert_eq!(sub.round, 1);
        assert_eq!(sub.verdict, Verdict::ChangesRequested);

        let sub2 = db
            .submit_review(
                "R-001",
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();
        assert_eq!(sub2.round, 2);

        let submissions = db.get_review_submissions("R-001").unwrap();
        assert_eq!(submissions.len(), 2);

        let comments = db.get_review_comments(sub.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Fix this");
    }

    #[test]
    fn find_reviews_for_work() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            branch: None,
            depends_on: vec!["W-001".to_string()],
        })
        .unwrap();

        let reviews = db.find_reviews_for_work("W-001").unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, "R-001");

        let works = db.find_works_for_review("R-001").unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].id, "W-001");
    }
}
