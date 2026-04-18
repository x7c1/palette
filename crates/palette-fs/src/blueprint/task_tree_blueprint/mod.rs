use super::TaskNode;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Filename for the Blueprint YAML.
pub const BLUEPRINT_FILENAME: &str = "blueprint.yaml";

/// Root of a Blueprint YAML document.
#[derive(Debug, Deserialize)]
pub struct TaskTreeBlueprint {
    pub task: TaskNode,
}

/// Read and parse a Blueprint YAML file into a TaskTreeBlueprint.
///
/// Validates the co-location convention:
/// - For every task node that declares a `plan_path`, the referenced file must
///   exist under the Blueprint's directory. This applies uniformly to the root
///   task (whose `plan_path` acts as the workflow-wide plan) and to any child
///   task with its own plan document.
/// - No nested `blueprint.yaml` may exist in any subdirectory.
///
/// A Blueprint that declares no `plan_path` on any task is valid and carries
/// no required plan document — this is the normal shape for purely mechanical
/// workflows such as auto-generated PR reviews.
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

    verify_plan_paths_exist(&blueprint.task, blueprint_dir)?;

    if let Some(nested) = find_nested_blueprint(blueprint_dir)? {
        return Err(BlueprintReadError::NestedBlueprint {
            outer: path.to_path_buf(),
            nested,
        });
    }

    Ok(blueprint)
}

fn verify_plan_paths_exist(
    node: &TaskNode,
    blueprint_dir: &Path,
) -> Result<(), BlueprintReadError> {
    if let Some(ref plan_path) = node.plan_path {
        let resolved = blueprint_dir.join(plan_path);
        if !resolved.exists() {
            return Err(BlueprintReadError::PlanPathMissing {
                task_key: node.key.clone(),
                plan_path: plan_path.clone(),
                expected: resolved,
            });
        }
    }
    for child in &node.children {
        verify_plan_paths_exist(child, blueprint_dir)?;
    }
    Ok(())
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
    PlanPathMissing {
        task_key: String,
        plan_path: String,
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
            BlueprintReadError::PlanPathMissing {
                task_key,
                plan_path,
                expected,
            } => {
                write!(
                    f,
                    "plan_path '{plan_path}' on task '{task_key}' does not exist at '{}'",
                    expected.display()
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

    const MINIMAL_BLUEPRINT_NO_PLAN: &str = "task:\n  key: test\n  children:\n    - key: task-a\n      type: craft\n      repository:\n        name: x7c1/palette-demo\n        branch: main\n";

    const BLUEPRINT_WITH_ROOT_PLAN: &str = "task:\n  key: test\n  plan_path: README.md\n  children:\n    - key: task-a\n      type: craft\n      repository:\n        name: x7c1/palette-demo\n        branch: main\n";

    const BLUEPRINT_WITH_CHILD_PLAN: &str = "task:\n  key: test\n  children:\n    - key: task-a\n      type: craft\n      plan_path: task-a/README.md\n      repository:\n        name: x7c1/palette-demo\n        branch: main\n";

    #[test]
    fn parse_nested_task_tree() {
        let yaml = r#"
task:
  key: feature-x
  plan_path: README.md
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
    fn read_blueprint_without_any_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, MINIMAL_BLUEPRINT_NO_PLAN);

        let bp = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME)).unwrap();
        assert_eq!(bp.task.key, "test");
    }

    #[test]
    fn read_blueprint_with_root_plan_when_present() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, BLUEPRINT_WITH_ROOT_PLAN);
        write_file(dir.path(), "README.md", "# plan\n");

        let bp = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME)).unwrap();
        assert_eq!(bp.task.plan_path.as_deref(), Some("README.md"));
    }

    #[test]
    fn read_blueprint_rejects_missing_root_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, BLUEPRINT_WITH_ROOT_PLAN);

        let result = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME));
        let err = result.err().expect("should fail");
        assert!(
            matches!(err, BlueprintReadError::PlanPathMissing { ref task_key, .. } if task_key == "test"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_blueprint_rejects_missing_child_plan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, BLUEPRINT_WITH_CHILD_PLAN);

        let result = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME));
        let err = result.err().expect("should fail");
        assert!(
            matches!(err, BlueprintReadError::PlanPathMissing { ref task_key, .. } if task_key == "task-a"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_blueprint_rejects_nested_blueprint() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), BLUEPRINT_FILENAME, MINIMAL_BLUEPRINT_NO_PLAN);
        // A child directory containing its own blueprint.yaml — should fail
        write_file(
            &dir.path().join("subtask"),
            BLUEPRINT_FILENAME,
            MINIMAL_BLUEPRINT_NO_PLAN,
        );

        let result = read_blueprint(&dir.path().join(BLUEPRINT_FILENAME));
        assert!(matches!(
            result,
            Err(BlueprintReadError::NestedBlueprint { .. })
        ));
    }
}
