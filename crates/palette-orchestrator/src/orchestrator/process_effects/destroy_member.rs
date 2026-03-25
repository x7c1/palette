use super::Orchestrator;
use palette_domain::worker::WorkerId;

impl Orchestrator {
    pub(super) fn destroy_member(&self, member_id: &WorkerId) {
        let worker = match self.db.remove_worker(member_id) {
            Ok(Some(w)) => w,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(member_id = %member_id, error = %e, "failed to remove member from DB");
                return;
            }
        };
        tracing::info!(member_id = %member_id, "destroying member container");
        if let Err(e) = self.docker.stop_container(&worker.container_id) {
            tracing::warn!(member_id = %member_id, error = %e, "failed to stop member container");
        }
        if let Err(e) = self.docker.remove_container(&worker.container_id) {
            tracing::warn!(member_id = %member_id, error = %e, "failed to remove member container");
        }
    }
}
