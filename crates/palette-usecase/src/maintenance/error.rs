use std::panic::Location;

#[derive(Debug)]
pub enum AdminMaintenanceError {
    DataStore {
        at: &'static Location<'static>,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl std::fmt::Display for AdminMaintenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminMaintenanceError::DataStore { at, source } => {
                write!(
                    f,
                    "maintenance datastore error at {}:{}: {}",
                    at.file(),
                    at.line(),
                    source
                )
            }
        }
    }
}

impl std::error::Error for AdminMaintenanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AdminMaintenanceError::DataStore { source, .. } => Some(source.as_ref()),
        }
    }
}

#[track_caller]
pub(super) fn track_error(
    source: Box<dyn std::error::Error + Send + Sync>,
) -> AdminMaintenanceError {
    AdminMaintenanceError::DataStore {
        at: Location::caller(),
        source,
    }
}
