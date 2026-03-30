use crate::{AppState, Error};
use palette_domain::ReasonKey;
use palette_domain::job::{CreateJobRequest, JobId, JobType};
use palette_domain::rule::{RuleEffect, TaskEffect};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_usecase::task_store::TaskStore;
use palette_usecase::{RuleEngine, TaskRuleEngine};

/// Resolve which tasks become Ready, activate them, and recurse into composites.
pub(super) fn activate_ready_children(
    state: &AppState,
    task_store: &TaskStore,
    task_engine: &TaskRuleEngine<'_>,
    child_ids: &[TaskId],
) -> crate::Result<Vec<RuleEffect>> {
    let task_effects = task_engine
        .resolve_ready_tasks(child_ids)
        .map_err(Error::internal)?;

    let mut effects = Vec::new();

    for effect in &task_effects {
        let TaskEffect::TaskStatusChanged {
            task_id,
            new_status,
        } = effect
        else {
            continue;
        };

        task_store
            .update_task_status(task_id, *new_status)
            .map_err(Error::internal)?;
        tracing::info!(task_id = %task_id, status = ?new_status, "task status changed");

        if *new_status != TaskStatus::Ready {
            continue;
        }

        let Some(task) = task_store.get_task(task_id).map_err(Error::internal)? else {
            continue;
        };
        let children = task_store
            .get_child_tasks(task_id)
            .map_err(Error::internal)?;

        if let Some(job_type) = task.job_type {
            // Task with job: create job and set composite to InProgress
            if !children.is_empty() {
                task_store
                    .update_task_status(task_id, TaskStatus::InProgress)
                    .map_err(Error::internal)?;
            }
            let job_effects = create_job(state, &task, job_type)?;
            effects.extend(job_effects);

            // Review composites (review-integrate): resolve children immediately.
            // Craft composites: children are activated later on InReview.
            if !children.is_empty() && job_type == JobType::Review {
                let ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
                let child_effects = activate_ready_children(state, task_store, task_engine, &ids)?;
                effects.extend(child_effects);
            }
        } else if !children.is_empty() {
            // Pure composite: spawn supervisor, then InProgress and recurse
            effects.push(RuleEffect::SpawnSupervisor {
                task_id: task_id.clone(),
                role: WorkerRole::Leader,
            });
            task_store
                .update_task_status(task_id, TaskStatus::InProgress)
                .map_err(Error::internal)?;
            let ids: Vec<TaskId> = children.iter().map(|c| c.id.clone()).collect();
            let child_effects = activate_ready_children(state, task_store, task_engine, &ids)?;
            effects.extend(child_effects);
        }
    }

    Ok(effects)
}

/// Create a Job for a task and return AutoAssign effects.
pub(super) fn create_job(
    state: &AppState,
    task: &palette_domain::task::Task,
    job_type: JobType,
) -> crate::Result<Vec<RuleEffect>> {
    let job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest {
            id: Some(JobId::generate(job_type)),
            task_id: task.id.clone(),
            job_type,
            title: palette_domain::job::Title::parse(task.key.to_string())
                .map_err(|e| Error::internal(e.reason_key()))?,
            plan_path: palette_domain::job::PlanPath::parse(
                task.plan_path
                    .clone()
                    .ok_or_else(|| Error::internal(format!("task {} has no plan_path", task.id)))?,
            )
            .map_err(|e| Error::internal(e.reason_key()))?,
            assignee_id: None,
            priority: task.priority,
            repository: task.repository.clone(),
        })
        .map_err(Error::internal)?;

    let effects = RuleEngine::new(state.interactor.data_store.as_ref(), 0)
        .on_job_created(&job.id)
        .map_err(Error::internal)?;

    tracing::info!(
        job_id = %job.id,
        task_id = %task.id,
        job_type = ?job_type,
        "created job for task"
    );

    Ok(effects)
}
