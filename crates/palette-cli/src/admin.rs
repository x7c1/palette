use crate::config::Config;
use chrono::{DateTime, Duration, Utc};
use clap::{Args, Subcommand};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const USER_CONFIG_RELATIVE: &str = ".config/palette/config.toml";

#[derive(Subcommand)]
pub enum AdminCommand {
    /// Remove runtime state for all workflows and clear runtime data directories.
    Reset(ResetArgs),
    /// Garbage-collect stale workflows and their runtime artifacts.
    Gc(GcArgs),
}

#[derive(Args)]
pub struct ResetArgs {
    /// Path to the configuration file (overrides the default user config)
    #[arg(short, long)]
    config: Option<String>,
    /// Preview what would be deleted without deleting anything
    #[arg(long)]
    dry_run: bool,
    /// Confirm destructive operation
    #[arg(long)]
    yes: bool,
}

#[derive(Args)]
pub struct GcArgs {
    /// Path to the configuration file (overrides the default user config)
    #[arg(short, long)]
    config: Option<String>,
    /// Target specific workflow IDs (repeatable)
    #[arg(long = "workflow-id")]
    workflow_ids: Vec<String>,
    /// Include active/suspending workflows in candidates
    #[arg(long)]
    include_active: bool,
    /// Keep only workflows older than this number of hours
    #[arg(long)]
    older_than_hours: Option<i64>,
    /// Preview what would be deleted without deleting anything
    #[arg(long)]
    dry_run: bool,
    /// Confirm destructive operation
    #[arg(long)]
    yes: bool,
}

pub fn run(command: AdminCommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        AdminCommand::Reset(args) => run_reset(args),
        AdminCommand::Gc(args) => run_gc(args),
    }
}

fn run_reset(args: ResetArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.dry_run && !args.yes {
        return Err("refusing destructive operation: pass --yes (or use --dry-run)".into());
    }

    let config_path = resolve_config_path(args.config.as_deref())?;
    let config = Config::load(&config_path)?;
    let data_dir = data_dir_from_db_path(&config.db_path);
    let mut conn = open_db_for_admin(&config.db_path)?;

    let workflow_ids = list_workflow_ids(&conn)?;
    let plan = gather_cleanup_plan(&conn, &workflow_ids, &data_dir)?;
    print_plan("reset", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let tx = conn.transaction()?;
    let mut deleted = DeletedCounts::default();
    deleted.message_queue += tx.execute("DELETE FROM message_queue", [])?;
    deleted.review_comments += tx.execute("DELETE FROM review_comments", [])?;
    deleted.review_submissions += tx.execute("DELETE FROM review_submissions", [])?;
    deleted.jobs += tx.execute("DELETE FROM jobs", [])?;
    deleted.workers += tx.execute("DELETE FROM workers", [])?;
    deleted.tasks += tx.execute("DELETE FROM tasks", [])?;
    deleted.workflows += tx.execute("DELETE FROM workflows", [])?;
    tx.commit()?;

    let removed_files = remove_paths(&plan.file_paths);
    print_deleted(&deleted, removed_files);
    Ok(())
}

fn run_gc(args: GcArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.dry_run && !args.yes {
        return Err("refusing destructive operation: pass --yes (or use --dry-run)".into());
    }

    let config_path = resolve_config_path(args.config.as_deref())?;
    let config = Config::load(&config_path)?;
    let data_dir = data_dir_from_db_path(&config.db_path);
    let mut conn = open_db_for_admin(&config.db_path)?;

    let selected = select_gc_workflows(
        &conn,
        &args.workflow_ids,
        args.include_active,
        args.older_than_hours,
    )?;
    if selected.is_empty() {
        println!("gc: no matching workflows");
        return Ok(());
    }

    let workflow_ids: Vec<String> = selected.into_iter().map(|w| w.id).collect();
    let plan = gather_cleanup_plan(&conn, &workflow_ids, &data_dir)?;
    print_plan("gc", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let tx = conn.transaction()?;
    let mut deleted = DeletedCounts::default();
    for workflow_id in &workflow_ids {
        deleted.message_queue += tx.execute(
            "DELETE FROM message_queue
             WHERE target_id IN (SELECT id FROM workers WHERE workflow_id = ?1)",
            [workflow_id],
        )?;
        deleted.review_comments += tx.execute(
            "DELETE FROM review_comments
             WHERE submission_id IN (
               SELECT rs.id
               FROM review_submissions rs
               JOIN jobs j ON j.id = rs.review_job_id
               JOIN tasks t ON t.id = j.task_id
               WHERE t.workflow_id = ?1
             )",
            [workflow_id],
        )?;
        deleted.review_submissions += tx.execute(
            "DELETE FROM review_submissions
             WHERE review_job_id IN (
               SELECT j.id
               FROM jobs j
               JOIN tasks t ON t.id = j.task_id
               WHERE t.workflow_id = ?1
             )",
            [workflow_id],
        )?;
        deleted.jobs += tx.execute(
            "DELETE FROM jobs
             WHERE task_id IN (SELECT id FROM tasks WHERE workflow_id = ?1)",
            [workflow_id],
        )?;
        deleted.workers +=
            tx.execute("DELETE FROM workers WHERE workflow_id = ?1", [workflow_id])?;
        deleted.tasks += tx.execute("DELETE FROM tasks WHERE workflow_id = ?1", [workflow_id])?;
        deleted.workflows += tx.execute("DELETE FROM workflows WHERE id = ?1", [workflow_id])?;
    }
    tx.commit()?;

    let removed_files = remove_paths(&plan.file_paths);
    print_deleted(&deleted, removed_files);
    Ok(())
}

#[derive(Debug)]
struct WorkflowEntry {
    id: String,
    status: String,
    started_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct DeletedCounts {
    workflows: usize,
    tasks: usize,
    jobs: usize,
    workers: usize,
    review_submissions: usize,
    review_comments: usize,
    message_queue: usize,
}

struct CleanupPlan {
    workflow_ids: Vec<String>,
    job_ids: Vec<String>,
    worker_ids: Vec<String>,
    file_paths: Vec<PathBuf>,
}

fn open_db_for_admin(db_path: &str) -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path).map_err(|e| {
        format!(
            "failed to open db '{}': {e}. stop palette process and retry",
            db_path
        )
    })?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

fn resolve_config_path(
    config_override: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = config_override {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME").map_err(|e| format!("HOME environment variable: {e}"))?;
    let user_config = PathBuf::from(home).join(USER_CONFIG_RELATIVE);
    if user_config.exists() {
        Ok(user_config)
    } else {
        Err(format!("config not found: {}", user_config.display()).into())
    }
}

fn data_dir_from_db_path(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("data"))
}

fn list_workflow_ids(conn: &Connection) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT id FROM workflows ORDER BY started_at DESC")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn list_workflows(conn: &Connection) -> Result<Vec<WorkflowEntry>, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare(
        "SELECT w.id, ws.name, w.started_at
         FROM workflows w
         JOIN workflow_statuses ws ON ws.id = w.status_id
         ORDER BY w.started_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let started: String = row.get(2)?;
        Ok(WorkflowEntry {
            id: row.get(0)?,
            status: row.get(1)?,
            started_at: DateTime::parse_from_rfc3339(&started)
                .ok()
                .map(|dt| dt.with_timezone(&Utc)),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn select_gc_workflows(
    conn: &Connection,
    explicit_ids: &[String],
    include_active: bool,
    older_than_hours: Option<i64>,
) -> Result<Vec<WorkflowEntry>, Box<dyn std::error::Error>> {
    let workflows = list_workflows(conn)?;
    let threshold = older_than_hours.map(|h| Utc::now() - Duration::hours(h));

    let mut selected = Vec::new();
    for wf in workflows {
        if !explicit_ids.is_empty() && !explicit_ids.iter().any(|id| id == &wf.id) {
            continue;
        }
        if explicit_ids.is_empty()
            && !matches!(wf.status.as_str(), "suspended" | "completed")
            && !(include_active && matches!(wf.status.as_str(), "active" | "suspending"))
        {
            continue;
        }
        if let Some(t) = threshold
            && let Some(started) = wf.started_at
            && started > t
        {
            continue;
        }
        selected.push(wf);
    }
    Ok(selected)
}

fn gather_cleanup_plan(
    conn: &Connection,
    workflow_ids: &[String],
    data_dir: &Path,
) -> Result<CleanupPlan, Box<dyn std::error::Error>> {
    let mut job_ids = Vec::new();
    let mut worker_ids = Vec::new();
    let mut blueprint_paths = Vec::new();

    for workflow_id in workflow_ids {
        {
            let mut stmt = conn.prepare(
                "SELECT j.id
                 FROM jobs j
                 JOIN tasks t ON t.id = j.task_id
                 WHERE t.workflow_id = ?1",
            )?;
            let rows = stmt.query_map([workflow_id], |row| row.get::<_, String>(0))?;
            for row in rows {
                job_ids.push(row?);
            }
        }
        {
            let mut stmt = conn.prepare("SELECT id FROM workers WHERE workflow_id = ?1")?;
            let rows = stmt.query_map([workflow_id], |row| row.get::<_, String>(0))?;
            for row in rows {
                worker_ids.push(row?);
            }
        }
        {
            let mut stmt = conn.prepare("SELECT blueprint_path FROM workflows WHERE id = ?1")?;
            let rows = stmt.query_map([workflow_id], |row| row.get::<_, String>(0))?;
            for row in rows {
                blueprint_paths.push(row?);
            }
        }
    }

    let mut file_paths = Vec::new();
    for workflow_id in workflow_ids {
        file_paths.push(data_dir.join("artifacts").join(workflow_id));
    }
    for job_id in &job_ids {
        file_paths.push(data_dir.join("workspace").join(job_id));
    }
    for worker_id in &worker_ids {
        file_paths.push(data_dir.join("transcripts").join(worker_id));
    }
    for blueprint in blueprint_paths {
        file_paths.push(resolve_path_like(&blueprint));
    }

    Ok(CleanupPlan {
        workflow_ids: workflow_ids.to_vec(),
        job_ids,
        worker_ids,
        file_paths,
    })
}

fn resolve_path_like(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

fn remove_paths(paths: &[PathBuf]) -> usize {
    let mut removed = 0;
    for path in paths {
        if !path.exists() {
            continue;
        }
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match result {
            Ok(()) => removed += 1,
            Err(e) => eprintln!("warn: failed to remove {}: {}", path.display(), e),
        }
    }
    removed
}

fn print_plan(mode: &str, plan: &CleanupPlan) {
    println!("admin {} plan:", mode);
    println!("  workflows: {}", plan.workflow_ids.len());
    println!("  tasks/jobs: {} jobs", plan.job_ids.len());
    println!("  workers: {}", plan.worker_ids.len());
    println!("  filesystem targets: {}", plan.file_paths.len());
    if !plan.workflow_ids.is_empty() {
        for id in &plan.workflow_ids {
            println!("    - {}", id);
        }
    }
}

fn print_deleted(deleted: &DeletedCounts, removed_files: usize) {
    println!("deleted:");
    println!("  workflows: {}", deleted.workflows);
    println!("  tasks: {}", deleted.tasks);
    println!("  jobs: {}", deleted.jobs);
    println!("  workers: {}", deleted.workers);
    println!("  review_submissions: {}", deleted.review_submissions);
    println!("  review_comments: {}", deleted.review_comments);
    println!("  message_queue: {}", deleted.message_queue);
    println!("  filesystem entries removed: {}", removed_files);
}
