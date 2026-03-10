mod notification;
pub use notification::handle_notification;

mod stop;
pub use stop::handle_stop;

#[derive(serde::Deserialize)]
pub(crate) struct HookQuery {
    pub member_id: Option<String>,
}
