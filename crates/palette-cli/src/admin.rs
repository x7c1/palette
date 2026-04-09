use crate::config::Config;
use chrono::{Duration, Utc};
use clap::{Args, Subcommand};
use palette_db::Database;
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use std::collections::BTreeSet;
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
    workflow_ids: Vec<WorkflowId>,
    task_ids: Vec<TaskId>,
    job_ids: Vec<String>,
    worker_ids: Vec<WorkerId>,
    file_paths: Vec<PathBuf>,
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
    let db = open_db_for_admin(&config.db_path)?;
    let workflow_ids = db
        .list_workflows(None)?
        .into_iter()
        .map(|w| w.id)
        .collect::<Vec<_>>();
    let plan = gather_cleanup_plan(&db, &workflow_ids, &data_dir)?;
    print_plan("reset", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = execute_cleanup(&db, &plan.workflow_ids)?;
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
    let db = open_db_for_admin(&config.db_path)?;

    let selected = select_gc_workflows(
        &db,
        &args.workflow_ids,
        args.include_active,
        args.older_than_hours,
    )?;
    if selected.is_empty() {
        println!("gc: no matching workflows");
        return Ok(());
    }

    let plan = gather_cleanup_plan(&db, &selected, &data_dir)?;
    print_plan("gc", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = execute_cleanup(&db, &plan.workflow_ids)?;
    let removed_files = remove_paths(&plan.file_paths);
    print_deleted(&deleted, removed_files);
    Ok(())
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

fn open_db_for_admin(db_path: &str) -> Result<Database, Box<dyn std::error::Error>> {
    Database::open(Path::new(db_path)).map_err(|e| {
        format!(
            "failed to open database '{}': {}. stop `palette start` and retry",
            db_path, e
        )
        .into()
    })
}

fn select_gc_workflows(
    db: &Database,
    explicit_ids: &[String],
    include_active: bool,
    older_than_hours: Option<i64>,
) -> Result<Vec<WorkflowId>, Box<dyn std::error::Error>> {
    if !explicit_ids.is_empty() {
        return explicit_ids
            .iter()
            .map(|id| {
                WorkflowId::parse(id.clone())
                    .map_err(|e| format!("invalid workflow-id '{id}': {e:?}").into())
            })
            .collect();
    }

    let threshold = older_than_hours.map(|h| Utc::now() - Duration::hours(h));
    let mut selected = Vec::new();
    for wf in db.list_workflows(None)? {
        let eligible_status = matches!(
            wf.status,
            WorkflowStatus::Suspended | WorkflowStatus::Completed
        ) || (include_active
            && matches!(
                wf.status,
                WorkflowStatus::Active | WorkflowStatus::Suspending
            ));
        if !eligible_status {
            continue;
        }
        if let Some(t) = threshold
            && wf.started_at > t
        {
            continue;
        }
        selected.push(wf.id);
    }
    Ok(selected)
}

fn gather_cleanup_plan(
    db: &Database,
    workflow_ids: &[WorkflowId],
    data_dir: &Path,
) -> Result<CleanupPlan, Box<dyn std::error::Error>> {
    let all_workers = db.list_all_workers()?;
    let all_workflows = db.list_workflows(None)?;

    let mut task_ids = Vec::new();
    let mut job_ids = Vec::new();
    let mut worker_ids = Vec::new();
    let mut file_paths = BTreeSet::new();

    for workflow_id in workflow_ids {
        let tasks = db.get_task_statuses(workflow_id)?;
        let mut workflow_job_ids = Vec::new();
        for task_id in tasks.keys() {
            task_ids.push(task_id.clone());
            if let Some(job) = db.get_job_by_task_id(task_id)? {
                let jid = job.id.to_string();
                workflow_job_ids.push(jid.clone());
                job_ids.push(jid);
            }
        }

        let mut workflow_worker_ids = Vec::new();
        for worker in all_workers.iter().filter(|w| w.workflow_id == *workflow_id) {
            worker_ids.push(worker.id.clone());
            workflow_worker_ids.push(worker.id.clone());
        }

        file_paths.insert(data_dir.join("artifacts").join(workflow_id.as_ref()));
        for job_id in &workflow_job_ids {
            file_paths.insert(data_dir.join("workspace").join(job_id));
        }
        for worker_id in &workflow_worker_ids {
            file_paths.insert(data_dir.join("transcripts").join(worker_id.as_ref()));
        }
        if let Some(wf) = all_workflows.iter().find(|w| w.id == *workflow_id) {
            file_paths.insert(resolve_path_like(&wf.blueprint_path));
        }
    }

    Ok(CleanupPlan {
        workflow_ids: workflow_ids.to_vec(),
        task_ids,
        job_ids,
        worker_ids,
        file_paths: file_paths.into_iter().collect(),
    })
}

fn execute_cleanup(
    db: &Database,
    workflow_ids: &[WorkflowId],
) -> Result<DeletedCounts, Box<dyn std::error::Error>> {
    let mut deleted = DeletedCounts::default();
    for workflow_id in workflow_ids {
        let task_ids = db
            .get_task_statuses(workflow_id)?
            .into_keys()
            .collect::<Vec<_>>();
        let worker_ids = db
            .list_all_workers()?
            .into_iter()
            .filter(|w| w.workflow_id == *workflow_id)
            .map(|w| w.id)
            .collect::<Vec<_>>();

        deleted.message_queue += db.delete_messages_by_targets(&worker_ids)?;
        let (deleted_comments, deleted_submissions) =
            db.delete_review_data_by_workflow(workflow_id)?;
        deleted.review_comments += deleted_comments;
        deleted.review_submissions += deleted_submissions;

        for task_id in &task_ids {
            if db.get_job_by_task_id(task_id)?.is_some() {
                deleted.jobs += 1;
            }
            db.delete_jobs_by_task_id(task_id)?;
        }

        for worker_id in &worker_ids {
            if db.remove_worker(worker_id)?.is_some() {
                deleted.workers += 1;
            }
        }

        for task_id in &task_ids {
            db.delete_task(task_id)?;
            deleted.tasks += 1;
        }

        deleted.workflows += db.delete_workflow(workflow_id)?;
    }
    Ok(deleted)
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
    println!("  tasks: {}", plan.task_ids.len());
    println!("  jobs: {}", plan.job_ids.len());
    println!("  workers: {}", plan.worker_ids.len());
    println!("  filesystem targets: {}", plan.file_paths.len());
    for id in &plan.workflow_ids {
        println!("    - {}", id);
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
