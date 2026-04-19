use std::io::Write;
use std::path::{Path, PathBuf};

/// A test Blueprint directory holding `blueprint.yaml` plus stub files for
/// every `plan_path` the YAML references. The TempDir is kept alive by this
/// wrapper so the directory survives until the test is done with it.
pub struct BlueprintFixture {
    _dir: tempfile::TempDir,
    blueprint_path: PathBuf,
}

impl BlueprintFixture {
    pub fn path(&self) -> &Path {
        &self.blueprint_path
    }
}

pub fn write_blueprint_file(yaml: &str) -> BlueprintFixture {
    let dir = tempfile::tempdir().unwrap();
    let blueprint_path = dir.path().join("blueprint.yaml");
    let mut bp = std::fs::File::create(&blueprint_path).unwrap();
    bp.write_all(yaml.as_bytes()).unwrap();

    // The parser requires a co-located parent plan (README.md) next to every
    // blueprint.yaml.
    std::fs::write(dir.path().join("README.md"), "# test blueprint\n").unwrap();

    // Palette's Blueprint parser verifies every `plan_path` declared in the
    // YAML points to an existing file. Create empty stubs so fixtures can
    // carry plan_path references for readability without each test having to
    // write the files itself.
    for plan_rel in extract_plan_paths(yaml) {
        let target = dir.path().join(&plan_rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        if !target.exists() {
            std::fs::File::create(&target).unwrap();
        }
    }

    BlueprintFixture {
        _dir: dir,
        blueprint_path,
    }
}

fn extract_plan_paths(yaml: &str) -> Vec<String> {
    yaml.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("plan_path:")?;
            let value = rest.trim().trim_matches(|c: char| c == '"' || c == '\'');
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
        .collect()
}
