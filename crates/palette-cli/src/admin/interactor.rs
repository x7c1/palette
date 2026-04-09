use crate::config::Config;
use palette_db::Database;
use palette_domain::task::TaskTree;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_domain::workflow::WorkflowId;
use palette_usecase::{
    BlueprintReader, ContainerMounts, ContainerRuntime, GitHubReviewPort, Interactor,
    ReadBlueprintError, ReviewEvent, ReviewFileComment, TerminalSession,
};
use std::path::Path;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub(super) struct AdminContext {
    pub config: Config,
    pub interactor: Interactor,
}

pub(super) fn build_admin_context(
    config_path: &Path,
) -> Result<AdminContext, Box<dyn std::error::Error>> {
    let config = Config::load(config_path)?;
    let interactor = build_admin_interactor(&config.db_path)?;
    Ok(AdminContext { config, interactor })
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
