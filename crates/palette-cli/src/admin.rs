use crate::config::Config;
use clap::{Args, Subcommand};
use palette_db::Database;
use palette_domain::task::TaskTree;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_domain::workflow::WorkflowId;
use palette_usecase::{
    AdminCleanupPlan, AdminDeletedCounts, AdminGcOptions, AdminMaintenanceError, BlueprintReader,
    ContainerMounts, ContainerRuntime, GitHubReviewPort, Interactor, ReadBlueprintError,
    ReviewEvent, ReviewFileComment, TerminalSession,
};
use std::path::{Path, PathBuf};

const USER_CONFIG_RELATIVE: &str = ".config/palette/config.toml";
type BoxErr = Box<dyn std::error::Error + Send + Sync>;

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
    let interactor = build_admin_interactor(&config.db_path)?;

    let plan = interactor
        .admin_plan_reset(&data_dir)
        .map_err(to_box_error)?;
    print_plan("reset", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = interactor
        .admin_execute_cleanup(&plan.workflow_ids)
        .map_err(to_box_error)?;
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
    let interactor = build_admin_interactor(&config.db_path)?;

    let workflow_ids = args
        .workflow_ids
        .iter()
        .map(|id| {
            WorkflowId::parse(id.clone())
                .map_err(|e| format!("invalid workflow-id '{id}': {e:?}").into())
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let options = AdminGcOptions {
        workflow_ids,
        include_active: args.include_active,
        older_than_hours: args.older_than_hours,
    };
    let plan = interactor
        .admin_plan_gc(&data_dir, &options)
        .map_err(to_box_error)?;
    if plan.workflow_ids.is_empty() {
        println!("gc: no matching workflows");
        return Ok(());
    }
    print_plan("gc", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = interactor
        .admin_execute_cleanup(&plan.workflow_ids)
        .map_err(to_box_error)?;
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

fn build_admin_interactor(db_path: &str) -> Result<Interactor, Box<dyn std::error::Error>> {
    let db = Database::open(Path::new(db_path)).map_err(|e| {
        format!(
            "failed to open database '{}': {}. stop `palette start` and retry",
            db_path, e
        )
    })?;

    Ok(Interactor {
        container: Box::new(NoopContainer),
        terminal: Box::new(NoopTerminal),
        data_store: Box::new(db),
        blueprint: Box::new(NoopBlueprint),
        github_review_port: Box::new(NoopGitHubReview),
    })
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

fn print_plan(mode: &str, plan: &AdminCleanupPlan) {
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

fn print_deleted(deleted: &AdminDeletedCounts, removed_files: usize) {
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

fn to_box_error(e: AdminMaintenanceError) -> Box<dyn std::error::Error> {
    Box::new(e)
}

fn unsupported<T>(name: &str) -> Result<T, BoxErr> {
    Err(std::io::Error::other(format!("{name} is not available in admin mode")).into())
}

struct NoopContainer;
impl ContainerRuntime for NoopContainer {
    fn create_container(
        &self,
        _name: &str,
        _image: &str,
        _role: WorkerRole,
        _session_name: &str,
        _mounts: ContainerMounts,
    ) -> Result<ContainerId, BoxErr> {
        unsupported("create_container")
    }
    fn start_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("start_container")
    }
    fn stop_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("stop_container")
    }
    fn remove_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("remove_container")
    }
    fn is_container_running(&self, _container_id: &str) -> bool {
        false
    }
    fn is_claude_running(&self, _container_id: &ContainerId) -> bool {
        false
    }
    fn list_managed_containers(&self) -> Result<Vec<ContainerId>, BoxErr> {
        Ok(vec![])
    }
    fn write_settings(
        &self,
        _container_id: &ContainerId,
        _template_path: &Path,
        _worker_id: &str,
    ) -> Result<(), BoxErr> {
        unsupported("write_settings")
    }
    fn copy_file_to_container(
        &self,
        _container_id: &ContainerId,
        _local_path: &Path,
        _container_path: &str,
    ) -> Result<(), BoxErr> {
        unsupported("copy_file_to_container")
    }
    fn copy_dir_to_container(
        &self,
        _container_id: &ContainerId,
        _local_dir: &Path,
        _container_path: &str,
    ) -> Result<(), BoxErr> {
        unsupported("copy_dir_to_container")
    }
    fn read_container_file(
        &self,
        _container_id: &ContainerId,
        _path: &str,
        _tail_lines: usize,
    ) -> Result<String, BoxErr> {
        unsupported("read_container_file")
    }
    fn claude_exec_command(
        &self,
        _container_id: &ContainerId,
        _prompt_file: &str,
        _role: WorkerRole,
        _workdir: Option<&str>,
    ) -> String {
        String::new()
    }
    fn claude_resume_command(
        &self,
        _container_id: &ContainerId,
        _session_id: &WorkerSessionId,
        _role: WorkerRole,
        _workdir: Option<&str>,
    ) -> String {
        String::new()
    }
}

struct NoopTerminal;
impl TerminalSession for NoopTerminal {
    fn create_target(&self, _name: &str) -> Result<TerminalTarget, BoxErr> {
        unsupported("create_target")
    }
    fn create_pane(&self, _base_target: &TerminalTarget) -> Result<TerminalTarget, BoxErr> {
        unsupported("create_pane")
    }
    fn send_keys(&self, _target: &TerminalTarget, _text: &str) -> Result<(), BoxErr> {
        unsupported("send_keys")
    }
    fn send_keys_no_enter(&self, _target: &TerminalTarget, _text: &str) -> Result<(), BoxErr> {
        unsupported("send_keys_no_enter")
    }
    fn capture_pane(&self, _target: &TerminalTarget) -> Result<String, BoxErr> {
        unsupported("capture_pane")
    }
    fn kill_session(&self, _name: &TerminalSessionName) -> Result<(), BoxErr> {
        unsupported("kill_session")
    }
}

struct NoopBlueprint;
impl BlueprintReader for NoopBlueprint {
    fn read_blueprint(
        &self,
        _path: &Path,
        _workflow_id: &WorkflowId,
    ) -> Result<TaskTree, ReadBlueprintError> {
        Err(ReadBlueprintError::Read(
            std::io::Error::other("read_blueprint is not available in admin mode").into(),
        ))
    }
}

struct NoopGitHubReview;
impl GitHubReviewPort for NoopGitHubReview {
    fn post_review(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
        _comments: &[ReviewFileComment],
        _event: ReviewEvent,
    ) -> Result<(), BoxErr> {
        unsupported("post_review")
    }

    fn get_diff_files(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<Vec<String>, BoxErr> {
        unsupported("get_diff_files")
    }
}
