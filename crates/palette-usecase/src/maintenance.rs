use crate::Interactor;
use chrono::{Duration, Utc};
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use palette_domain::workflow::{WorkflowId, WorkflowStatus};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct AdminDeletedCounts {
    pub workflows: usize,
    pub tasks: usize,
    pub jobs: usize,
    pub workers: usize,
    pub review_submissions: usize,
    pub review_comments: usize,
    pub message_queue: usize,
}

pub struct AdminCleanupPlan {
    pub workflow_ids: Vec<WorkflowId>,
    pub task_ids: Vec<TaskId>,
    pub job_ids: Vec<String>,
    pub worker_ids: Vec<WorkerId>,
    pub file_paths: Vec<PathBuf>,
}

pub struct AdminGcOptions {
    pub workflow_ids: Vec<WorkflowId>,
    pub include_active: bool,
    pub older_than_hours: Option<i64>,
}

impl Interactor {
    pub fn admin_plan_reset(
        &self,
        data_dir: &Path,
    ) -> Result<AdminCleanupPlan, Box<dyn std::error::Error + Send + Sync>> {
        let workflow_ids = self
            .data_store
            .list_workflows(None)?
            .into_iter()
            .map(|w| w.id)
            .collect::<Vec<_>>();
        self.gather_cleanup_plan(&workflow_ids, data_dir)
    }

    pub fn admin_plan_gc(
        &self,
        data_dir: &Path,
        options: &AdminGcOptions,
    ) -> Result<AdminCleanupPlan, Box<dyn std::error::Error + Send + Sync>> {
        let workflow_ids = if !options.workflow_ids.is_empty() {
            options.workflow_ids.clone()
        } else {
            let threshold = options
                .older_than_hours
                .map(|h| Utc::now() - Duration::hours(h));
            self.data_store
                .list_workflows(None)?
                .into_iter()
                .filter(|wf| {
                    matches!(
                        wf.status,
                        WorkflowStatus::Suspended | WorkflowStatus::Completed
                    ) || (options.include_active
                        && matches!(
                            wf.status,
                            WorkflowStatus::Active | WorkflowStatus::Suspending
                        ))
                })
                .filter(|wf| threshold.is_none_or(|t| wf.started_at <= t))
                .map(|wf| wf.id)
                .collect::<Vec<_>>()
        };
        self.gather_cleanup_plan(&workflow_ids, data_dir)
    }

    pub fn admin_execute_cleanup(
        &self,
        workflow_ids: &[WorkflowId],
    ) -> Result<AdminDeletedCounts, Box<dyn std::error::Error + Send + Sync>> {
        let mut deleted = AdminDeletedCounts::default();

        for workflow_id in workflow_ids {
            let task_ids = self
                .data_store
                .get_task_statuses(workflow_id)?
                .into_keys()
                .collect::<Vec<_>>();
            let worker_ids = self
                .data_store
                .list_all_workers()?
                .into_iter()
                .filter(|w| w.workflow_id == *workflow_id)
                .map(|w| w.id)
                .collect::<Vec<_>>();

            deleted.message_queue += self.data_store.delete_messages_by_targets(&worker_ids)?;
            let (deleted_comments, deleted_submissions) = self
                .data_store
                .delete_review_data_by_workflow(workflow_id)?;
            deleted.review_comments += deleted_comments;
            deleted.review_submissions += deleted_submissions;

            for task_id in &task_ids {
                if self.data_store.get_job_by_task_id(task_id)?.is_some() {
                    deleted.jobs += 1;
                }
                self.data_store.delete_jobs_by_task_id(task_id)?;
            }

            for worker_id in &worker_ids {
                if self.data_store.remove_worker(worker_id)?.is_some() {
                    deleted.workers += 1;
                }
            }

            for task_id in &task_ids {
                self.data_store.delete_task(task_id)?;
                deleted.tasks += 1;
            }

            deleted.workflows += self.data_store.delete_workflow(workflow_id)?;
        }

        Ok(deleted)
    }

    fn gather_cleanup_plan(
        &self,
        workflow_ids: &[WorkflowId],
        data_dir: &Path,
    ) -> Result<AdminCleanupPlan, Box<dyn std::error::Error + Send + Sync>> {
        let all_workers = self.data_store.list_all_workers()?;
        let all_workflows = self.data_store.list_workflows(None)?;

        let mut task_ids = Vec::new();
        let mut job_ids = Vec::new();
        let mut worker_ids = Vec::new();
        let mut file_paths = BTreeSet::new();

        for workflow_id in workflow_ids {
            let tasks = self.data_store.get_task_statuses(workflow_id)?;
            let mut workflow_job_ids = Vec::new();
            for task_id in tasks.keys() {
                task_ids.push(task_id.clone());
                if let Some(job) = self.data_store.get_job_by_task_id(task_id)? {
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

        Ok(AdminCleanupPlan {
            workflow_ids: workflow_ids.to_vec(),
            task_ids,
            job_ids,
            worker_ids,
            file_paths: file_paths.into_iter().collect(),
        })
    }
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
