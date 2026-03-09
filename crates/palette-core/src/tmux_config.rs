use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TmuxConfig {
    pub session_name: String,
}
