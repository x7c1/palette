mod error;
pub use error::{Error, Result};

mod record;

mod convert;

use palette_domain::PersistentState;
use std::path::Path;

/// Save state atomically (write to temp file, then rename).
pub fn save(state: &PersistentState, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = convert::to_state_file(state);
    let json = serde_json::to_string_pretty(&file)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Load state from file. Returns None if file doesn't exist.
pub fn load(path: &Path) -> Result<Option<PersistentState>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let file: record::StateFile = serde_json::from_str(&content)?;
    let state = convert::from_state_file(file)?;
    Ok(Some(state))
}
