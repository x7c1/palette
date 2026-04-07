mod notification;
pub use notification::handle_notification;

mod session_start;
pub use session_start::handle_session_start;

mod stop;
pub use stop::handle_stop;

use palette_domain::worker::{WorkerId, WorkerSessionId};
use palette_usecase::DataStore;

#[derive(serde::Deserialize)]
pub(crate) struct HookQuery {
    pub worker_id: Option<String>,
}

/// Save session_id from any Claude Code hook payload.
/// All hook payloads include session_id as a common field.
pub(crate) fn save_session_id(
    data_store: &dyn DataStore,
    worker_id: &WorkerId,
    payload: &serde_json::Value,
) {
    if let Some(session_id) = payload.get("session_id").and_then(|v| v.as_str()) {
        let sid = WorkerSessionId::new(session_id);
        if let Err(e) = data_store.update_worker_session_id(worker_id, &sid) {
            tracing::error!(
                worker_id = %worker_id,
                error = %e,
                "failed to save session_id"
            );
        }
    }
}
