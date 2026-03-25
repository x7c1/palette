use super::Orchestrator;
use std::collections::HashSet;

impl Orchestrator {
    /// Detect and remove orphan containers at startup.
    ///
    /// An orphan container has the `palette.managed=true` label but no
    /// corresponding worker record in the database — typically left over from
    /// a previous crash or forced exit.
    pub fn clean_orphan_containers(&self) {
        let managed = match self.docker.list_managed_containers() {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(error = %e, "failed to list managed containers for orphan cleanup");
                return;
            }
        };
        if managed.is_empty() {
            return;
        }

        let known: HashSet<String> = match self.db.list_all_workers() {
            Ok(workers) => workers
                .into_iter()
                .map(|w| w.container_id.to_string())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "failed to list workers for orphan cleanup");
                return;
            }
        };

        let mut orphan_count = 0;
        for container_id in &managed {
            // Docker ps --format '{{.ID}}' returns short IDs.
            // DB stores the full ID returned by docker create.
            // A container is known if any DB entry starts with the short ID,
            // or the short ID starts with a DB entry (shouldn't happen but be safe).
            let is_known = known.iter().any(|k| k.starts_with(container_id.as_ref()));
            if is_known {
                continue;
            }
            tracing::info!(container_id = %container_id, "removing orphan container");
            if let Err(e) = self.docker.stop_container(container_id) {
                tracing::warn!(container_id = %container_id, error = %e, "failed to stop orphan container");
            }
            if let Err(e) = self.docker.remove_container(container_id) {
                tracing::warn!(container_id = %container_id, error = %e, "failed to remove orphan container");
            }
            orphan_count += 1;
        }

        if orphan_count > 0 {
            tracing::info!(orphan_count, "orphan container cleanup complete");
        }
    }
}
