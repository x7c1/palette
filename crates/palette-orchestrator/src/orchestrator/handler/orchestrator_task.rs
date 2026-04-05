use std::time::Instant;

use palette_domain::job::{Job, JobDetail, JobId, JobStatus, MechanizedStatus};
use palette_domain::server::ServerEvent;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::Orchestrator;

/// Default timeout for orchestrator task commands (5 minutes).
const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

impl Orchestrator {
    /// Execute an orchestrator task's command asynchronously.
    ///
    /// Spawns the command as a child process and monitors it in a background
    /// tokio task. When the command completes (or times out), an
    /// `OrchestratorTaskCompleted` event is sent back to the event loop.
    pub(super) fn execute_orchestrator_task(
        &self,
        job: &Job,
        event_tx: &mpsc::UnboundedSender<ServerEvent>,
    ) {
        let JobDetail::Orchestrator { ref command } = job.detail else {
            tracing::error!(job_id = %job.id, "execute_orchestrator_task called on non-orchestrator job");
            return;
        };
        let Some(command) = command else {
            tracing::error!(job_id = %job.id, "orchestrator task has no command");
            return;
        };

        // Mark job as in-progress
        if let Err(e) = self.interactor.data_store.update_job_status(
            &job.id,
            JobStatus::Orchestrator(MechanizedStatus::InProgress),
        ) {
            tracing::error!(job_id = %job.id, error = %e, "failed to set orchestrator job to in_progress");
            return;
        }

        let job_id = job.id.clone();
        let command = command.clone();
        let tx = event_tx.clone();

        // Determine workspace directory for command execution.
        // The orchestrator task depends on the implementation task, so
        // the parent task's craft job workspace should exist.
        let work_dir = self.resolve_orchestrator_work_dir(job);

        tokio::spawn(async move {
            let start = Instant::now();

            let result = tokio::time::timeout(DEFAULT_TIMEOUT, async {
                let child = match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .current_dir(work_dir.as_deref().unwrap_or("."))
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(child) => child,
                    Err(e) => {
                        return (false, String::new(), format!("spawn error: {e}"), None);
                    }
                };

                match child.wait_with_output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let success = output.status.success();
                        let exit_code = output.status.code();
                        (success, stdout, stderr, exit_code)
                    }
                    Err(e) => (false, String::new(), format!("wait error: {e}"), None),
                }
            })
            .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            let (success, stdout, stderr, exit_code) = match result {
                Ok(r) => r,
                Err(_) => (false, String::new(), "command timed out".to_string(), None),
            };

            let _ = tx.send(ServerEvent::OrchestratorTaskCompleted {
                job_id,
                success,
                stdout,
                stderr,
                exit_code,
                duration_ms,
            });
        });
    }

    /// Handle the completion of an orchestrator task command.
    pub(super) async fn handle_orchestrator_task_completed(
        self: &Arc<Self>,
        job_id: &JobId,
        success: bool,
        stdout: &str,
        stderr: &str,
        exit_code: Option<i32>,
        duration_ms: u64,
    ) {
        let job = match self.interactor.data_store.get_job(job_id) {
            Ok(Some(j)) => j,
            Ok(None) => {
                tracing::error!(job_id = %job_id, "orchestrator task completed but job not found");
                return;
            }
            Err(e) => {
                tracing::error!(job_id = %job_id, error = %e, "failed to get job after orchestrator task completed");
                return;
            }
        };

        // Save check-result.json
        self.save_check_result(&job, success, stdout, stderr, exit_code, duration_ms);

        if success {
            tracing::info!(job_id = %job_id, duration_ms, "orchestrator task succeeded");
            if let Err(e) = self
                .interactor
                .data_store
                .update_job_status(job_id, JobStatus::Orchestrator(MechanizedStatus::Done))
            {
                tracing::error!(job_id = %job_id, error = %e, "failed to mark orchestrator job as done");
                return;
            }
            // Complete the job and cascade through task tree
            match self.try_complete_task_by_job(job_id) {
                Ok(result) => self.dispatch_pending_actions(result),
                Err(e) => {
                    tracing::error!(error = %e, job_id = %job_id, "failed to complete orchestrator job")
                }
            }
        } else {
            tracing::warn!(
                job_id = %job_id,
                exit_code = ?exit_code,
                duration_ms,
                "orchestrator task failed"
            );
            if let Err(e) = self
                .interactor
                .data_store
                .update_job_status(job_id, JobStatus::Orchestrator(MechanizedStatus::Failed))
            {
                tracing::error!(job_id = %job_id, error = %e, "failed to mark orchestrator job as failed");
                return;
            }
            // Revert the dependent implementation task.
            // command is always Some here because execute_orchestrator_task
            // validates it before spawning. Log and bail if somehow absent.
            let Some(command) = job.detail.command() else {
                tracing::error!(job_id = %job_id, "orchestrator job has no command during revert");
                return;
            };
            self.revert_implementation_task(&job, command, stderr);
        }
    }

    /// Revert the implementation task that the orchestrator task depends on.
    /// Sends the failure log as feedback to the crafter.
    ///
    /// `command` is the orchestrator command that failed (already validated
    /// as `Some` by `execute_orchestrator_task`).
    fn revert_implementation_task(&self, orchestrator_job: &Job, command: &str, stderr: &str) {
        let task_id = &orchestrator_job.task_id;

        // Find sibling craft task (implementation task)
        let task_state = match self.interactor.data_store.get_task_state(task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let Some(task) = task_store.get_task(task_id) else {
            return;
        };
        let Some(ref parent_id) = task.parent_id else {
            return;
        };

        // Find the implementation (craft) task among siblings
        let siblings = task_store.get_child_tasks(parent_id);
        for sibling in &siblings {
            if !matches!(sibling.job_detail, Some(JobDetail::Craft { .. })) {
                continue;
            }
            let craft_job = match self.interactor.data_store.get_job_by_task_id(&sibling.id) {
                Ok(Some(j)) => j,
                Ok(None) => continue,
                Err(e) => {
                    tracing::error!(task_id = %sibling.id, error = %e, "failed to get craft job for revert");
                    continue;
                }
            };

            // Revert craft job to InProgress (same as ChangesRequested)
            let reverted_status =
                palette_domain::job::CraftTransition::RequestChanges.to_job_status();
            if let Err(e) = self
                .interactor
                .data_store
                .update_job_status(&craft_job.id, reverted_status)
            {
                tracing::error!(
                    craft_job_id = %craft_job.id,
                    error = %e,
                    "failed to revert implementation task"
                );
                return;
            }

            // Enqueue failure feedback to the crafter
            if let Some(ref assignee) = craft_job.assignee_id {
                let msg = format!(
                    "## Automated Check Failed\n\nCommand: {}\nExit code: {}\n\n```\n{}\n```\n\nPlease fix the issues and try again.",
                    command,
                    craft_job.id,
                    stderr.chars().take(4000).collect::<String>(),
                );
                let _ = self.interactor.data_store.enqueue_message(assignee, &msg);

                // Reactivate the crafter
                match self.reactivate_member(&craft_job.id, assignee) {
                    Ok(actions) => {
                        for id in actions.deliver_to {
                            let _ = self.event_tx.send(
                                palette_domain::server::ServerEvent::DeliverMessages {
                                    target_id: id,
                                },
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to reactivate crafter after check failure");
                    }
                }
            }

            tracing::info!(
                orchestrator_job_id = %orchestrator_job.id,
                craft_job_id = %craft_job.id,
                "reverted implementation task due to check failure"
            );
            return;
        }
    }

    /// Save the check result to data/artifacts/{workflow_id}/{craft_job_id}/check-result.json.
    fn save_check_result(
        &self,
        job: &Job,
        success: bool,
        stdout: &str,
        stderr: &str,
        exit_code: Option<i32>,
        duration_ms: u64,
    ) {
        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };
        let Some(task) = task_store.get_task(&job.task_id) else {
            return;
        };
        let Some(ref parent_id) = task.parent_id else {
            return;
        };

        // Find sibling craft job to get the correct artifacts path
        let siblings = task_store.get_child_tasks(parent_id);
        let craft_sibling = siblings
            .iter()
            .find(|s| matches!(s.job_detail, Some(JobDetail::Craft { .. })));
        let Some(craft_sibling) = craft_sibling else {
            return;
        };
        let craft_job_id = match self
            .interactor
            .data_store
            .get_job_by_task_id(&craft_sibling.id)
        {
            Ok(Some(j)) => j.id,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(task_id = %craft_sibling.id, error = %e, "failed to get craft job for check result");
                return;
            }
        };

        let artifacts_path = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), craft_job_id.as_ref());
        if let Err(e) = std::fs::create_dir_all(&artifacts_path) {
            tracing::warn!(error = %e, "failed to create artifacts directory");
            return;
        }

        let command = job.detail.command();

        let result = serde_json::json!({
            "status": if success { "success" } else { "failed" },
            "command": command,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "duration_ms": duration_ms,
        });

        let path = artifacts_path.join("check-result.json");
        if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&result).unwrap()) {
            tracing::warn!(error = %e, path = %path.display(), "failed to write check-result.json");
        }
    }

    /// Resolve the working directory for an orchestrator task command.
    /// Looks for the sibling craft task's workspace.
    fn resolve_orchestrator_work_dir(&self, job: &Job) -> Option<String> {
        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            Ok(None) => return None,
            Err(e) => {
                tracing::error!(task_id = %job.task_id, error = %e, "failed to get task state for work dir");
                return None;
            }
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "failed to create task store for work dir");
                return None;
            }
        };
        let task = task_store.get_task(&job.task_id)?;
        let parent_id = task.parent_id.as_ref()?;

        let siblings = task_store.get_child_tasks(parent_id);
        let craft_sibling = siblings
            .iter()
            .find(|s| matches!(s.job_detail, Some(JobDetail::Craft { .. })))?;
        let craft_job = match self
            .interactor
            .data_store
            .get_job_by_task_id(&craft_sibling.id)
        {
            Ok(Some(j)) => j,
            Ok(None) => return None,
            Err(e) => {
                tracing::error!(task_id = %craft_sibling.id, error = %e, "failed to get craft job for work dir");
                return None;
            }
        };

        let ws_path = self.workspace_manager.workspace_path(craft_job.id.as_ref());
        if ws_path.exists() {
            Some(ws_path.to_string_lossy().to_string())
        } else {
            None
        }
    }
}
