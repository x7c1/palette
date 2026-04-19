mod blueprint_error;
mod to_task_tree;
use to_task_tree::to_task_tree;

mod blueprint_validator;

use crate::blueprint::BlueprintReadError;
use palette_core::{InputError, Location};
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::{BlueprintReader, ReadBlueprintError};
use std::collections::HashSet;
use std::path::Path;

/// Filesystem-backed blueprint reader.
///
/// Reads YAML blueprint files from the local filesystem and validates
/// perspective references against the known server configuration.
pub struct FsBlueprintReader {
    known_perspectives: HashSet<String>,
}

impl FsBlueprintReader {
    pub fn new(known_perspectives: HashSet<String>) -> Self {
        Self { known_perspectives }
    }
}

impl BlueprintReader for FsBlueprintReader {
    fn read_blueprint(
        &self,
        path: &Path,
        workflow_id: &WorkflowId,
    ) -> Result<TaskTree, ReadBlueprintError> {
        let blueprint = crate::read_blueprint(path).map_err(read_err_to_usecase_err)?;
        let tree =
            to_task_tree(&blueprint, workflow_id, &self.known_perspectives).map_err(|errors| {
                ReadBlueprintError::Invalid(
                    errors
                        .iter()
                        .map(|e| InputError {
                            location: Location::Body,
                            hint: e.field_path(),
                            reason: e.reason_key(),
                        })
                        .collect(),
                )
            })?;
        Ok(tree)
    }
}

fn read_err_to_usecase_err(err: BlueprintReadError) -> ReadBlueprintError {
    if err.is_not_found()
        && let BlueprintReadError::Io { path, .. } = &err
    {
        return ReadBlueprintError::NotFound {
            path: path.to_path_buf(),
        };
    }
    match err.reason_key() {
        Some(reason) => ReadBlueprintError::Invalid(vec![InputError {
            location: Location::Body,
            hint: err.field_path(),
            reason: reason.to_string(),
        }]),
        None => ReadBlueprintError::Internal(Box::new(err)),
    }
}
