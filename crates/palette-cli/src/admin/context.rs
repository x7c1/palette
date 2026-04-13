use crate::config::Config;
use crate::interactor_factory::build_interactor;
use palette_usecase::Interactor;
use std::path::Path;
use std::sync::Arc;

pub(super) struct AdminContext {
    pub config: Config,
    pub interactor: Arc<Interactor>,
}

pub(super) fn build_admin_context(
    config_path: &Path,
) -> Result<AdminContext, Box<dyn std::error::Error>> {
    let config = Config::load(config_path)?;
    let perspectives = config.perspectives.validate()?;
    let interactor = build_interactor(&config, &perspectives)?;
    Ok(AdminContext { config, interactor })
}
