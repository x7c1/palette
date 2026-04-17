use crate::config::Config;
use palette_db::Database;
use palette_docker::CallbackNetworkMode;
use palette_docker::DockerManager;
use palette_domain::terminal::TerminalSessionName;
use palette_fs::FsBlueprintReader;
use palette_orchestrator::github_client::GhCliReviewClient;
use palette_orchestrator::{CallbackNetwork, ValidatedPerspectives};
use palette_tmux::TmuxManager;
use palette_usecase::Interactor;
use std::sync::Arc;

pub(crate) fn build_interactor(
    config: &Config,
    perspectives: &ValidatedPerspectives,
) -> Result<Arc<Interactor>, Box<dyn std::error::Error>> {
    let session_name = TerminalSessionName::new(&config.tmux.session_name);
    let tmux = TmuxManager::new(session_name);

    let db_path = config.db_path();
    let db = Database::open(&db_path)?;
    tracing::info!(db_path = %db_path.display(), "database initialized");

    let callback_network_mode = match config.docker.callback_network {
        CallbackNetwork::Auto => CallbackNetworkMode::Auto,
        CallbackNetwork::Host => CallbackNetworkMode::Host,
        CallbackNetwork::Bridge => CallbackNetworkMode::Bridge,
    };
    let docker = DockerManager::new(
        config.docker.worker_callback_url.clone(),
        callback_network_mode,
    );

    Ok(Arc::new(Interactor {
        container: Box::new(docker),
        terminal: Box::new(tmux),
        data_store: Box::new(db),
        blueprint: Box::new(FsBlueprintReader::new(perspectives.names())),
        github_review_port: GhCliReviewClient::boxed(),
    }))
}
