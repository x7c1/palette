mod notification;
pub use notification::handle_notification;

mod session_start;
pub use session_start::handle_session_start;

mod stop;
pub use stop::handle_stop;

#[derive(serde::Deserialize)]
pub(crate) struct HookQuery {
    pub worker_id: Option<String>,
}
