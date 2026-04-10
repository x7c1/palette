use super::noop::blueprint::NoopBlueprint;
use super::noop::container::NoopContainer;
use super::noop::github_review::NoopGitHubReview;
use super::noop::terminal::NoopTerminal;
use crate::config::Config;
use palette_db::Database;
use palette_usecase::Interactor;
use std::path::Path;

pub(super) struct AdminContext {
    pub config: Config,
    pub interactor: Interactor,
}

pub(super) fn build_admin_context(
    config_path: &Path,
) -> Result<AdminContext, Box<dyn std::error::Error>> {
    let config = Config::load(config_path)?;
    let interactor = build_admin_interactor(&config.db_path)?;
    Ok(AdminContext { config, interactor })
}

fn build_admin_interactor(db_path: &str) -> Result<Interactor, Box<dyn std::error::Error>> {
    let db = Database::open(Path::new(db_path)).map_err(|e| {
        format!(
            "failed to open database '{}': {}. stop `palette start` and retry",
            db_path, e
        )
    })?;

    Ok(Interactor {
        container: Box::new(NoopContainer),
        terminal: Box::new(NoopTerminal),
        data_store: Box::new(db),
        blueprint: Box::new(NoopBlueprint),
        github_review_port: Box::new(NoopGitHubReview),
    })
}
