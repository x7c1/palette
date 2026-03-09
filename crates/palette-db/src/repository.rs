use crate::errors::DbError;
use crate::models::QueuedMessage;
use crate::repository_row;
use crate::schema;
use chrono::{DateTime, Utc};
use palette_domain::*;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

/// Acquire the Mutex lock, converting a poisoned lock into DbError.
macro_rules! lock {
    ($mutex:expr) => {
        $mutex.lock().map_err(|_| DbError::LockPoisoned)?
    };
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DbError::Internal(format!(
                    "failed to create db directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let conn = Connection::open(path).map_err(|e| {
            DbError::Internal(format!("failed to open database {}: {e}", path.display()))
        })?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_task(&self, req: &CreateTaskRequest) -> Result<Task, DbError> {
        let mut conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = req
            .id
            .clone()
            .unwrap_or_else(|| TaskId::generate(req.task_type));

        let repos_json = req
            .repositories
            .as_ref()
            .map(|r| repository_row::repositories_to_json(r));

        // Work tasks start as Draft; review tasks start as Todo
        let initial_status = match req.task_type {
            TaskType::Work => TaskStatus::Draft,
            TaskType::Review => TaskStatus::Todo,
        };

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO tasks (id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, NULL, NULL)",
            params![
                id.as_ref(),
                req.task_type.as_str(),
                req.title,
                req.description,
                req.assignee.as_ref().map(|a| a.as_ref()),
                initial_status.as_str(),
                req.priority.map(|p| p.as_str()),
                repos_json,
                now_str,
                now_str,
            ],
        )?;

        for dep in &req.depends_on {
            tx.execute(
                "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
                params![id.as_ref(), dep.as_ref()],
            )?;
        }

        let task = query_task(&tx, &id)?
            .ok_or_else(|| DbError::Task(TaskError::NotFound { task_id: id }))?;

        tx.commit()?;
        Ok(task)
    }

    pub fn get_task(&self, id: &TaskId) -> Result<Option<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at
             FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_task(row)))?;
        match rows.next() {
            Some(Ok(task)) => Ok(Some(task)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn list_tasks(&self, filter: &TaskFilter) -> Result<Vec<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut sql = "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at FROM tasks WHERE 1=1".to_string();
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
            param_values.push(Box::new(a.as_ref().to_string()));
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

    pub fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, DbError> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id.as_ref()],
        )?;
        if updated == 0 {
            return Err(TaskError::NotFound {
                task_id: id.clone(),
            }
            .into());
        }
        drop(conn);
        self.get_task(id)?.ok_or_else(|| {
            TaskError::NotFound {
                task_id: id.clone(),
            }
            .into()
        })
    }

    pub fn get_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;
        let rows = stmt.query_map(params![task_id.as_ref()], |row| {
            Ok(TaskId::new(row.get::<_, String>(0)?))
        })?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    pub fn get_dependents(&self, depends_on_id: &TaskId) -> Result<Vec<TaskId>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare("SELECT task_id FROM dependencies WHERE depends_on = ?1")?;
        let rows = stmt.query_map(params![depends_on_id.as_ref()], |row| {
            Ok(TaskId::new(row.get::<_, String>(0)?))
        })?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }

    /// Find review tasks that depend on the given work task.
    pub fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             JOIN dependencies d ON d.task_id = t.id
             WHERE d.depends_on = ?1 AND t.type = 'review'",
        )?;
        let rows = stmt.query_map(params![work_id.as_ref()], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    /// Find work tasks that a review task depends on.
    pub fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             JOIN dependencies d ON d.depends_on = t.id
             WHERE d.task_id = ?1 AND t.type = 'work'",
        )?;
        let rows = stmt.query_map(params![review_id.as_ref()], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn submit_review(
        &self,
        review_task_id: &TaskId,
        req: &SubmitReviewRequest,
    ) -> Result<ReviewSubmission, DbError> {
        let mut conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let tx = conn.transaction()?;

        // Determine round number
        let round: i32 = tx
            .query_row(
                "SELECT COALESCE(MAX(round), 0) FROM review_submissions WHERE review_task_id = ?1",
                params![review_task_id.as_ref()],
                |row| row.get(0),
            )
            .unwrap_or(0)
            + 1;

        tx.execute(
            "INSERT INTO review_submissions (review_task_id, round, verdict, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                review_task_id.as_ref(),
                round,
                req.verdict.as_str(),
                req.summary,
                now_str
            ],
        )?;

        let submission_id = tx.last_insert_rowid();

        for comment in &req.comments {
            tx.execute(
                "INSERT INTO review_comments (submission_id, file, line, body)
                 VALUES (?1, ?2, ?3, ?4)",
                params![submission_id, comment.file, comment.line, comment.body],
            )?;
        }

        tx.commit()?;

        Ok(ReviewSubmission {
            id: submission_id,
            review_task_id: review_task_id.clone(),
            round,
            verdict: req.verdict,
            summary: req.summary.clone(),
            created_at: now,
        })
    }

    pub fn get_review_submissions(
        &self,
        review_task_id: &TaskId,
    ) -> Result<Vec<ReviewSubmission>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, review_task_id, round, verdict, summary, created_at
             FROM review_submissions WHERE review_task_id = ?1 ORDER BY round",
        )?;
        let rows = stmt.query_map(params![review_task_id.as_ref()], |row| {
            Ok(ReviewSubmission {
                id: row.get(0)?,
                review_task_id: TaskId::new(row.get::<_, String>(1)?),
                round: row.get(2)?,
                verdict: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(Verdict::Approved),
                summary: row.get(4)?,
                created_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;
        let mut submissions = Vec::new();
        for row in rows {
            submissions.push(row?);
        }
        Ok(submissions)
    }

    /// Assign a task to a member and set status to in_progress.
    pub fn assign_task(&self, task_id: &TaskId, assignee: &AgentId) -> Result<Task, DbError> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE tasks SET status = ?1, assignee = ?2, assigned_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![TaskStatus::InProgress.as_str(), assignee.as_ref(), now, now, task_id.as_ref()],
        )?;
        if updated == 0 {
            return Err(TaskError::NotFound {
                task_id: task_id.clone(),
            }
            .into());
        }
        drop(conn);
        self.get_task(task_id)?.ok_or_else(|| {
            TaskError::NotFound {
                task_id: task_id.clone(),
            }
            .into()
        })
    }

    /// Find work tasks that are ready and have all work dependencies done.
    /// Returns tasks ordered by priority (high > medium > low > null).
    pub fn find_assignable_tasks(&self) -> Result<Vec<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             WHERE t.type = 'work'
             AND t.status = 'ready'
             AND NOT EXISTS (
               SELECT 1 FROM dependencies d
               JOIN tasks dep ON d.depends_on = dep.id
               WHERE d.task_id = t.id
               AND dep.type = 'work'
               AND dep.status != 'done'
             )
             ORDER BY
               CASE t.priority
                 WHEN 'high' THEN 0
                 WHEN 'medium' THEN 1
                 WHEN 'low' THEN 2
                 ELSE 3
               END",
        )?;
        let rows = stmt.query_map([], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    /// Count the number of work tasks currently in_progress (active members).
    pub fn count_active_members(&self) -> Result<usize, DbError> {
        let conn = lock!(self.conn);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE type = 'work' AND status = 'in_progress'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    // --- Message Queue ---

    /// Enqueue a message for a target (member or leader).
    pub fn enqueue_message(
        &self,
        target_id: &AgentId,
        message: &str,
    ) -> Result<QueuedMessage, DbError> {
        let conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        conn.execute(
            "INSERT INTO message_queue (target_id, message, created_at) VALUES (?1, ?2, ?3)",
            params![target_id.as_ref(), message, now_str],
        )?;
        let id = conn.last_insert_rowid();
        Ok(QueuedMessage {
            id,
            target_id: target_id.clone(),
            message: message.to_string(),
            created_at: now,
        })
    }

    /// Dequeue the next message for a target (FIFO). Returns None if empty.
    pub fn dequeue_message(&self, target_id: &AgentId) -> Result<Option<QueuedMessage>, DbError> {
        let conn = lock!(self.conn);
        let msg = conn
            .prepare(
                "SELECT id, target_id, message, created_at FROM message_queue WHERE target_id = ?1 ORDER BY id LIMIT 1",
            )?
            .query_row(params![target_id.as_ref()], |row| {
                Ok(QueuedMessage {
                    id: row.get(0)?,
                    target_id: AgentId::new(row.get::<_, String>(1)?),
                    message: row.get(2)?,
                    created_at: parse_datetime(&row.get::<_, String>(3)?),
                })
            })
            .ok();

        if let Some(ref msg) = msg {
            conn.execute("DELETE FROM message_queue WHERE id = ?1", params![msg.id])?;
        }
        Ok(msg)
    }

    /// Check if a target has pending messages.
    pub fn has_pending_messages(&self, target_id: &AgentId) -> Result<bool, DbError> {
        let conn = lock!(self.conn);
        let exists = conn
            .prepare("SELECT 1 FROM message_queue WHERE target_id = ?1 LIMIT 1")?
            .exists(params![target_id.as_ref()])?;
        Ok(exists)
    }

    pub fn get_review_comments(&self, submission_id: i64) -> Result<Vec<ReviewComment>, DbError> {
        let conn = lock!(self.conn);
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

/// Parse an RFC3339 datetime string from the database.
fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Query a single task by ID from a connection or transaction.
fn query_task(conn: &Connection, id: &TaskId) -> Result<Option<Task>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at
         FROM tasks WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_task(row)))?;
    match rows.next() {
        Some(Ok(task)) => Ok(Some(task)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

fn row_to_task(row: &rusqlite::Row) -> Task {
    let repos_str: Option<String> = row.get(7).unwrap();
    let repositories: Option<Vec<Repository>> =
        repos_str.and_then(|s| repository_row::repositories_from_json(&s));

    Task {
        id: TaskId::new(row.get::<_, String>(0).unwrap()),
        task_type: row.get::<_, String>(1).unwrap().parse().unwrap(),
        title: row.get(2).unwrap(),
        description: row.get(3).unwrap(),
        assignee: row.get::<_, Option<String>>(4).unwrap().map(AgentId::new),
        status: row.get::<_, String>(5).unwrap().parse().unwrap(),
        priority: row
            .get::<_, Option<String>>(6)
            .unwrap()
            .and_then(|s| s.parse().ok()),
        repositories,
        pr_url: row.get(8).unwrap(),
        created_at: parse_datetime(&row.get::<_, String>(9).unwrap()),
        updated_at: parse_datetime(&row.get::<_, String>(10).unwrap()),
        notes: row.get(11).unwrap(),
        assigned_at: row
            .get::<_, Option<String>>(12)
            .unwrap()
            .map(|s| parse_datetime(&s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn tid(s: &str) -> TaskId {
        TaskId::new(s)
    }

    fn aid(s: &str) -> AgentId {
        AgentId::new(s)
    }

    #[test]
    fn create_and_get_task() {
        let db = test_db();
        let task = db
            .create_task(&CreateTaskRequest {
                id: Some(tid("W-001")),
                task_type: TaskType::Work,
                title: "Implement feature".to_string(),
                description: Some("Details".to_string()),
                assignee: Some(aid("member-a")),
                priority: Some(Priority::High),
                repositories: Some(vec![Repository {
                    name: "x7c1/palette".to_string(),
                    branch: Some("feature/test".to_string()),
                }]),
                depends_on: vec![],
            })
            .unwrap();

        assert_eq!(task.id, tid("W-001"));
        assert_eq!(task.task_type, TaskType::Work);
        assert_eq!(task.status, TaskStatus::Draft);
        assert_eq!(task.priority, Some(Priority::High));

        let fetched = db.get_task(&tid("W-001")).unwrap().unwrap();
        assert_eq!(fetched.title, "Implement feature");
    }

    #[test]
    fn create_task_with_dependencies() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work task".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review task".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();

        let deps = db.get_dependencies(&tid("R-001")).unwrap();
        assert_eq!(deps, vec![tid("W-001")]);

        let dependents = db.get_dependents(&tid("W-001")).unwrap();
        assert_eq!(dependents, vec![tid("R-001")]);
    }

    #[test]
    fn list_tasks_with_filter() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work 1".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review 1".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
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
        assert_eq!(works[0].id, tid("W-001"));
    }

    #[test]
    fn update_task_status() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        let updated = db
            .update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
    }

    #[test]
    fn submit_and_get_review() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();

        let sub = db
            .submit_review(
                &tid("R-001"),
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
                &tid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();
        assert_eq!(sub2.round, 2);

        let submissions = db.get_review_submissions(&tid("R-001")).unwrap();
        assert_eq!(submissions.len(), 2);

        let comments = db.get_review_comments(sub.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Fix this");
    }

    #[test]
    fn find_reviews_for_work() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();

        let reviews = db.find_reviews_for_work(&tid("W-001")).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, tid("R-001"));

        let works = db.find_works_for_review(&tid("R-001")).unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].id, tid("W-001"));
    }

    fn create_work(db: &Database, id: &str, priority: Option<Priority>, deps: Vec<TaskId>) {
        db.create_task(&CreateTaskRequest {
            id: Some(tid(id)),
            task_type: TaskType::Work,
            title: format!("Task {id}"),
            description: None,
            assignee: None,
            priority,
            repositories: None,
            depends_on: deps,
        })
        .unwrap();
    }

    #[test]
    fn assign_task_sets_assignee_and_status() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();

        let task = db.assign_task(&tid("W-001"), &aid("member-a")).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.assignee, Some(aid("member-a")));
        assert!(task.assigned_at.is_some());
    }

    #[test]
    fn find_assignable_tasks_no_deps() {
        let db = test_db();
        create_work(&db, "W-001", Some(Priority::High), vec![]);
        create_work(&db, "W-002", Some(Priority::Low), vec![]);

        // Both in draft — not assignable
        assert_eq!(db.find_assignable_tasks().unwrap().len(), 0);

        // Set both to ready
        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-002"), TaskStatus::Ready)
            .unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 2);
        assert_eq!(assignable[0].id, tid("W-001")); // high priority first
        assert_eq!(assignable[1].id, tid("W-002")); // low priority second
    }

    #[test]
    fn find_assignable_tasks_with_deps() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        create_work(&db, "W-002", None, vec![tid("W-001")]);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-002"), TaskStatus::Ready)
            .unwrap();

        // W-002 depends on W-001 which is not done
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("W-001"));

        // Complete W-001
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::Done)
            .unwrap();

        // Now W-002 is assignable
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("W-002"));
    }

    #[test]
    fn find_assignable_tasks_diamond_dag() {
        let db = test_db();
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        create_work(&db, "A", None, vec![]);
        create_work(&db, "B", None, vec![tid("A")]);
        create_work(&db, "C", None, vec![tid("A")]);
        create_work(&db, "D", None, vec![tid("B"), tid("C")]);

        for id in ["A", "B", "C", "D"] {
            db.update_task_status(&tid(id), TaskStatus::Ready).unwrap();
        }

        // Only A is assignable
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("A"));

        // Complete A → B and C become assignable
        db.assign_task(&tid("A"), &aid("m-a")).unwrap();
        db.update_task_status(&tid("A"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("A"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 2);
        let ids: Vec<&str> = assignable.iter().map(|t| t.id.as_ref()).collect();
        assert!(ids.contains(&"B"));
        assert!(ids.contains(&"C"));

        // Complete B, but D still waits for C
        db.assign_task(&tid("B"), &aid("m-b")).unwrap();
        db.update_task_status(&tid("B"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("B"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("C"));

        // Complete C → D becomes assignable
        db.assign_task(&tid("C"), &aid("m-c")).unwrap();
        db.update_task_status(&tid("C"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("C"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("D"));
    }

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

    #[test]
    fn message_queue_enqueue_dequeue() {
        let db = test_db();

        // Empty queue
        assert!(db.dequeue_message(&aid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&aid("member-a")).unwrap());

        // Enqueue
        let msg1 = db.enqueue_message(&aid("member-a"), "hello").unwrap();
        let msg2 = db.enqueue_message(&aid("member-a"), "world").unwrap();
        assert!(msg1.id < msg2.id);

        assert!(db.has_pending_messages(&aid("member-a")).unwrap());
        assert!(!db.has_pending_messages(&aid("member-b")).unwrap());

        // Dequeue in FIFO order
        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "hello");

        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "world");

        // Queue is empty
        assert!(db.dequeue_message(&aid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&aid("member-a")).unwrap());
    }

    #[test]
    fn message_queue_per_target_isolation() {
        let db = test_db();

        db.enqueue_message(&aid("member-a"), "msg-a").unwrap();
        db.enqueue_message(&aid("member-b"), "msg-b").unwrap();

        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-a");

        let dequeued = db.dequeue_message(&aid("member-b")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-b");
    }
}
