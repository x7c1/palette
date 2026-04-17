use super::TaskNode;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Filename for the parent plan that must accompany every Blueprint.
pub const PARENT_PLAN_FILENAME: &str = "README.md";

/// Filename for the Blueprint YAML.
pub const BLUEPRINT_FILENAME: &str = "blueprint.yaml";

/// Root of a Blueprint YAML document.
#[derive(Debug, Deserialize)]
pub struct TaskTreeBlueprint {
    pub task: TaskNode,
}

/// Read and parse a Blueprint YAML file into a TaskTreeBlueprint.
///
/// Enforces the co-location convention:
/// - The Blueprint's directory must contain a parent plan (`README.md`).
/// - No nested `blueprint.yaml` may exist in any subdirectory.
pub fn read_blueprint(path: &Path) -> Result<TaskTreeBlueprint, BlueprintReadError> {
    let yaml = std::fs::read_to_string(path).map_err(|e| BlueprintReadError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let blueprint: TaskTreeBlueprint =
        serde_yaml::from_str(&yaml).map_err(|e| BlueprintReadError::Parse {
            path: path.to_path_buf(),
            source: e,
        })?;

    let blueprint_dir = path
        .parent()
        .ok_or_else(|| BlueprintReadError::InvalidLocation {
            path: path.to_path_buf(),
        })?;

    let parent_plan = blueprint_dir.join(PARENT_PLAN_FILENAME);
    if !parent_plan.exists() {
        return Err(BlueprintReadError::ParentPlanMissing {
            blueprint_dir: blueprint_dir.to_path_buf(),
            expected: parent_plan,
        });
    }

    if let Some(nested) = find_nested_blueprint(blueprint_dir)? {
        return Err(BlueprintReadError::NestedBlueprint {
            outer: path.to_path_buf(),
            nested,
        });
    }

    Ok(blueprint)
}

fn find_nested_blueprint(dir: &Path) -> Result<Option<PathBuf>, BlueprintReadError> {
    let entries = std::fs::read_dir(dir).map_err(|e| BlueprintReadError::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| BlueprintReadError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join(BLUEPRINT_FILENAME);
            if candidate.exists() {
                return Ok(Some(candidate));
            }
            if let Some(found) = find_nested_blueprint(&path)? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

#[derive(Debug)]
pub enum BlueprintReadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    InvalidLocation {
        path: PathBuf,
    },
    ParentPlanMissing {
        blueprint_dir: PathBuf,
        expected: PathBuf,
    },
    NestedBlueprint {
        outer: PathBuf,
        nested: PathBuf,
    },
}

impl std::fmt::Display for BlueprintReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlueprintReadError::Io { path, source } => {
                write!(f, "failed to read blueprint '{}': {source}", path.display())
            }
            BlueprintReadError::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse blueprint '{}': {source}",
                    path.display()
                )
            }
            BlueprintReadError::InvalidLocation { path } => {
                write!(
                    f,
                    "blueprint path '{}' has no parent directory",
                    path.display()
                )
            }
            BlueprintReadError::ParentPlanMissing {
                blueprint_dir,
                expected,
            } => {
                write!(
                    f,
                    "parent plan missing: expected '{}' alongside blueprint at '{}'",
                    expected.display(),
                    blueprint_dir.display()
                )
            }
            BlueprintReadError::NestedBlueprint { outer, nested } => {
                write!(
                    f,
                    "nested blueprint not allowed: '{}' exists under outer blueprint '{}'",
                    nested.display(),
                    outer.display()
                )
            }
        }
    }
}

impl std::error::Error for BlueprintReadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    const MINIMAL_BLUEPRINT: &str = "task:\n  key: test\n  children:\n    - key: task-a\n      type: craft\n      repository:\n        name: x7c1/palette-demo\n        branch: main\n";

    #[test]
    fn parse_nested_task_tree() {
        let yaml = r#"
task:
  key: feature-x
  children:
    - key: planning
      children:
        - key: api-plan
          type: craft
          plan_path: planning/api-plan
          repository:
            name: x7c1/palette-demo
            branch: main
        - key: api-plan-review
          type: review
          depends_on: [api-plan]

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: execution/api-impl
          repository:
            name: x7c1/palette-demo
            branch: feature/x-api-impl
        - key: api-impl-review
          type: review
          depends_on: [api-impl]
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(blueprint.task.key, "feature-x");
        assert_eq!(blueprint.task.children.len(), 2);

        let planning = &blueprint.task.children[0];
        assert_eq!(planning.children.len(), 2);
        assert!(planning.job_type.is_none());

        let execution = &blueprint.task.children[1];
        assert_eq!(execution.depends_on, vec!["planning"]);
    }

    #[test]
    fn read_blueprint_with_parent_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, MINIMAL_BLUEPRINT);
        write_file(dir.path(), PARENT_PLAN_FILENAME, "# parent\n");

        let bp = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME)).unwrap();
        assert_eq!(bp.task.key, "test");
    }

    #[test]
    fn read_blueprint_rejects_missing_parent_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, MINIMAL_BLUEPRINT);

        let result = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME));
        assert!(matches!(
            result,
            Err(BlueprintReadError::ParentPlanMissing { .. })
        ));
    }

    #[test]
    fn read_blueprint_rejects_nested_blueprint() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, MINIMAL_BLUEPRINT);
        write_file(dir.path(), PARENT_PLAN_FILENAME, "# parent\n");
        // A child directory containing its own blueprint.yaml — should fail
        write_file(
            &dir.path().join("subtask"),
            BLUEPRINT_FILENAME,
            MINIMAL_BLUEPRINT,
        );
        write_file(
            &dir.path().join("subtask"),
            PARENT_PLAN_FILENAME,
            "# child\n",
        );

        let result = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME));
        assert!(matches!(
            result,
            Err(BlueprintReadError::NestedBlueprint { .. })
        ));
    }
}
