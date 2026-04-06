use super::TaskNode;
use serde::Deserialize;
use std::path::Path;

/// Root of a Blueprint YAML document.
#[derive(Debug, Deserialize)]
pub struct TaskTreeBlueprint {
    pub task: TaskNode,
}

/// Read and parse a Blueprint YAML file into a TaskTreeBlueprint.
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
    Ok(blueprint)
}

#[derive(Debug)]
pub enum BlueprintReadError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: std::path::PathBuf,
        source: serde_yaml::Error,
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
        }
    }
}

impl std::error::Error for BlueprintReadError {}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn read_blueprint_from_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut f,
            b"task:\n  key: test\n  children:\n    - key: task-a\n      type: craft\n      repository:\n        name: x7c1/palette-demo\n        branch: main\n",
        )
        .unwrap();

        let bp = read_blueprint(f.path()).unwrap();
        assert_eq!(bp.task.key, "test");
        assert_eq!(bp.task.children.len(), 1);
    }
}
