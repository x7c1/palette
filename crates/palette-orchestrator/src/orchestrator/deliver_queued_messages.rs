use super::Orchestrator;
use palette_domain::worker::{WorkerId, WorkerStatus};

impl Orchestrator {
    /// Delivers queued messages to idle targets.
    pub(super) fn deliver_queued_messages(&self, target_id: &WorkerId) -> crate::Result<bool> {
        let worker = self.interactor.data_store.find_worker(target_id)?;

        let terminal_target = match worker {
            Some(ref m) if m.status == WorkerStatus::Idle => m.terminal_target.clone(),
            _ => return Ok(false),
        };

        if let Some(msg) = self.interactor.data_store.dequeue_message(target_id)? {
            self.interactor.terminal.send_keys(&terminal_target, &msg)?;
            // Update status to Working
            self.interactor
                .data_store
                .update_worker_status(target_id, WorkerStatus::Working)?;
            tracing::info!(target_id = %target_id, "delivered queued message");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
