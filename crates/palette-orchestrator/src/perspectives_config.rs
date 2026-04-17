use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Raw TOML configuration for perspectives.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PerspectivesConfig {
    /// Named base directories for perspective documents.
    #[serde(default)]
    pub perspectives_dirs: HashMap<String, String>,
    /// Perspective definitions (name + paths).
    #[serde(default)]
    pub perspectives: Vec<PerspectiveEntry>,
}

/// A single perspective definition from configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PerspectiveEntry {
    /// Unique identifier for this perspective.
    pub name: String,
    /// Paths in `<dir_name>:<relative_path>` format.
    pub paths: Vec<String>,
}

/// Validated and resolved perspective configuration ready for runtime use.
#[derive(Debug, Clone)]
pub struct ValidatedPerspectives {
    /// Resolved base directories (name -> canonical absolute path).
    pub dirs: HashMap<String, PathBuf>,
    /// Validated perspective definitions.
    pub perspectives: Vec<ValidatedPerspective>,
}

/// A single validated perspective.
#[derive(Debug, Clone)]
pub struct ValidatedPerspective {
    pub name: String,
    pub paths: Vec<PerspectivePath>,
}

/// A parsed `<dir_name>:<relative_path>` entry.
#[derive(Debug, Clone)]
pub struct PerspectivePath {
    pub dir_name: String,
    pub relative_path: String,
}

impl PerspectivePath {
    /// The original `<dir_name>:<relative_path>` string.
    pub fn as_config_str(&self) -> String {
        format!("{}:{}", self.dir_name, self.relative_path)
    }
}

/// Errors from validating perspective configuration.
#[derive(Debug)]
pub enum PerspectivesValidationError {
    /// A perspectives_dirs directory does not exist.
    DirNotFound { name: String, path: String },
    /// Perspective name is not unique.
    DuplicateName { name: String },
    /// Perspective has empty paths.
    EmptyPaths { name: String },
    /// A path entry is empty or has an empty relative component.
    EmptyPathEntry { perspective: String, entry: String },
    /// Duplicate path entry within a perspective.
    DuplicatePathEntry { perspective: String, entry: String },
    /// The dir_name in a path entry does not exist in perspectives_dirs.
    UnknownDir {
        perspective: String,
        dir_name: String,
    },
    /// Resolved path is outside the base directory (traversal attempt).
    PathTraversal {
        perspective: String,
        entry: String,
        reason: String,
    },
}

impl std::fmt::Display for PerspectivesValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirNotFound { name, path } => {
                write!(f, "perspectives_dirs[{name}]: directory not found: {path}")
            }
            Self::DuplicateName { name } => {
                write!(f, "perspectives: duplicate name: {name}")
            }
            Self::EmptyPaths { name } => {
                write!(f, "perspectives[{name}]: paths must not be empty")
            }
            Self::EmptyPathEntry { perspective, entry } => {
                write!(
                    f,
                    "perspectives[{perspective}]: empty path entry: {entry:?}"
                )
            }
            Self::DuplicatePathEntry { perspective, entry } => {
                write!(
                    f,
                    "perspectives[{perspective}]: duplicate path entry: {entry}"
                )
            }
            Self::UnknownDir {
                perspective,
                dir_name,
            } => {
                write!(
                    f,
                    "perspectives[{perspective}]: unknown dir name: {dir_name}"
                )
            }
            Self::PathTraversal {
                perspective,
                entry,
                reason,
            } => {
                write!(
                    f,
                    "perspectives[{perspective}]: path traversal in {entry}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for PerspectivesValidationError {}

/// Aggregated error for perspective configuration validation.
#[derive(Debug)]
pub struct PerspectivesConfigError {
    pub errors: Vec<PerspectivesValidationError>,
}

impl std::fmt::Display for PerspectivesConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "perspectives config validation failed:")?;
        for e in &self.errors {
            writeln!(f, "  {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PerspectivesConfigError {}

impl PerspectivesConfig {
    /// Validate and resolve the raw configuration into runtime-ready form.
    pub fn validate(&self) -> Result<ValidatedPerspectives, PerspectivesConfigError> {
        let mut errors = Vec::new();

        // Validate and canonicalize directories
        let mut dirs = HashMap::new();
        for (name, path_str) in &self.perspectives_dirs {
            let path = Path::new(path_str);
            match path.canonicalize() {
                Ok(canonical) if canonical.is_dir() => {
                    dirs.insert(name.clone(), canonical);
                }
                _ => {
                    errors.push(PerspectivesValidationError::DirNotFound {
                        name: name.clone(),
                        path: path_str.clone(),
                    });
                }
            }
        }

        // Validate unique perspective names
        let mut seen_names = HashSet::new();
        for entry in &self.perspectives {
            if !seen_names.insert(&entry.name) {
                errors.push(PerspectivesValidationError::DuplicateName {
                    name: entry.name.clone(),
                });
            }
        }

        // Validate each perspective's paths
        let mut validated_perspectives = Vec::new();
        for entry in &self.perspectives {
            if entry.paths.is_empty() {
                errors.push(PerspectivesValidationError::EmptyPaths {
                    name: entry.name.clone(),
                });
                continue;
            }

            let mut seen_paths = HashSet::new();
            let mut validated_paths = Vec::new();
            for path_entry in &entry.paths {
                // Check for empty entries
                if path_entry.is_empty() {
                    errors.push(PerspectivesValidationError::EmptyPathEntry {
                        perspective: entry.name.clone(),
                        entry: path_entry.clone(),
                    });
                    continue;
                }

                // Parse "dir_name:relative_path"
                let Some((dir_name, relative)) = path_entry.split_once(':') else {
                    errors.push(PerspectivesValidationError::EmptyPathEntry {
                        perspective: entry.name.clone(),
                        entry: path_entry.clone(),
                    });
                    continue;
                };

                if relative.is_empty() {
                    errors.push(PerspectivesValidationError::EmptyPathEntry {
                        perspective: entry.name.clone(),
                        entry: path_entry.clone(),
                    });
                    continue;
                }

                // Check for duplicate paths
                if !seen_paths.insert(path_entry.clone()) {
                    errors.push(PerspectivesValidationError::DuplicatePathEntry {
                        perspective: entry.name.clone(),
                        entry: path_entry.clone(),
                    });
                    continue;
                }

                // Check dir_name exists
                let Some(base_dir) = dirs.get(dir_name) else {
                    errors.push(PerspectivesValidationError::UnknownDir {
                        perspective: entry.name.clone(),
                        dir_name: dir_name.to_string(),
                    });
                    continue;
                };

                // Resolve and check for traversal
                let full_path = base_dir.join(relative);
                match full_path.canonicalize() {
                    Ok(canonical) => {
                        if !canonical.starts_with(base_dir) {
                            errors.push(PerspectivesValidationError::PathTraversal {
                                perspective: entry.name.clone(),
                                entry: path_entry.clone(),
                                reason: "resolved path is outside base directory".to_string(),
                            });
                            continue;
                        }
                    }
                    Err(e) => {
                        errors.push(PerspectivesValidationError::PathTraversal {
                            perspective: entry.name.clone(),
                            entry: path_entry.clone(),
                            reason: e.to_string(),
                        });
                        continue;
                    }
                }

                validated_paths.push(PerspectivePath {
                    dir_name: dir_name.to_string(),
                    relative_path: relative.to_string(),
                });
            }

            validated_perspectives.push(ValidatedPerspective {
                name: entry.name.clone(),
                paths: validated_paths,
            });
        }

        if errors.is_empty() {
            Ok(ValidatedPerspectives {
                dirs,
                perspectives: validated_perspectives,
            })
        } else {
            Err(PerspectivesConfigError { errors })
        }
    }
}

impl ValidatedPerspectives {
    /// Return the set of perspective names for blueprint validation.
    pub fn names(&self) -> HashSet<String> {
        self.perspectives.iter().map(|p| p.name.clone()).collect()
    }

    /// Find a perspective by name.
    pub fn find(&self, name: &str) -> Option<&ValidatedPerspective> {
        self.perspectives.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_dirs() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("docs");
        fs::create_dir_all(base.join("axioms")).unwrap();
        fs::create_dir_all(base.join("principles")).unwrap();
        (tmp, base.to_string_lossy().to_string())
    }

    #[test]
    fn validates_valid_config() {
        let (tmp, base_path) = setup_dirs();
        let _ = tmp; // keep alive
        let config = PerspectivesConfig {
            perspectives_dirs: [("team-docs".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "rust-review".to_string(),
                paths: vec![
                    "team-docs:axioms".to_string(),
                    "team-docs:principles".to_string(),
                ],
            }],
        };
        let validated = config.validate().unwrap();
        assert_eq!(validated.names().len(), 1);
        assert!(validated.names().contains("rust-review"));
    }

    #[test]
    fn rejects_nonexistent_dir() {
        let config = PerspectivesConfig {
            perspectives_dirs: [("team-docs".to_string(), "/nonexistent/path".to_string())].into(),
            perspectives: vec![],
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, PerspectivesValidationError::DirNotFound { .. }))
        );
    }

    #[test]
    fn rejects_duplicate_perspective_names() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![
                PerspectiveEntry {
                    name: "dup".to_string(),
                    paths: vec!["a:axioms".to_string()],
                },
                PerspectiveEntry {
                    name: "dup".to_string(),
                    paths: vec!["a:principles".to_string()],
                },
            ],
        };
        let err = config.validate().unwrap_err();
        assert!(err.errors.iter().any(
            |e| matches!(e, PerspectivesValidationError::DuplicateName { name } if name == "dup")
        ));
    }

    #[test]
    fn rejects_empty_paths() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "empty".to_string(),
                paths: vec![],
            }],
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, PerspectivesValidationError::EmptyPaths { .. }))
        );
    }

    #[test]
    fn rejects_empty_path_entry() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "test".to_string(),
                paths: vec!["".to_string(), "a:".to_string()],
            }],
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.errors
                .iter()
                .all(|e| matches!(e, PerspectivesValidationError::EmptyPathEntry { .. }))
        );
        assert_eq!(err.errors.len(), 2);
    }

    #[test]
    fn rejects_duplicate_path_entries() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "test".to_string(),
                paths: vec!["a:axioms".to_string(), "a:axioms".to_string()],
            }],
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, PerspectivesValidationError::DuplicatePathEntry { .. }))
        );
    }

    #[test]
    fn rejects_unknown_dir_name() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "test".to_string(),
                paths: vec!["unknown:axioms".to_string()],
            }],
        };
        let err = config.validate().unwrap_err();
        assert!(err.errors
            .iter()
            .any(|e| matches!(e, PerspectivesValidationError::UnknownDir { dir_name, .. } if dir_name == "unknown")));
    }

    #[test]
    fn rejects_path_traversal() {
        let (_tmp, base_path) = setup_dirs();
        let config = PerspectivesConfig {
            perspectives_dirs: [("a".to_string(), base_path)].into(),
            perspectives: vec![PerspectiveEntry {
                name: "test".to_string(),
                paths: vec!["a:../../../etc".to_string()],
            }],
        };
        let err = config.validate().unwrap_err();
        assert!(
            err.errors
                .iter()
                .any(|e| matches!(e, PerspectivesValidationError::PathTraversal { .. }))
        );
    }

    #[test]
    fn empty_config_is_valid() {
        let config = PerspectivesConfig::default();
        let validated = config.validate().unwrap();
        assert!(validated.names().is_empty());
    }
}
