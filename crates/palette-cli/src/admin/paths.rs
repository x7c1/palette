use std::path::{Path, PathBuf};

const USER_CONFIG_RELATIVE: &str = ".config/palette/config.toml";

pub(super) fn resolve_config_path(
    config_override: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = config_override {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME").map_err(|e| format!("HOME environment variable: {e}"))?;
    let user_config = PathBuf::from(home).join(USER_CONFIG_RELATIVE);
    if user_config.exists() {
        Ok(user_config)
    } else {
        Err(format!("config not found: {}", user_config.display()).into())
    }
}

pub(super) fn data_dir_from_db_path(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("data"))
}

pub(super) fn remove_paths(paths: &[PathBuf]) -> usize {
    let mut removed = 0;
    for path in paths {
        if !path.exists() {
            continue;
        }
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match result {
            Ok(()) => removed += 1,
            Err(e) => eprintln!("warn: failed to remove {}: {}", path.display(), e),
        }
    }
    removed
}
