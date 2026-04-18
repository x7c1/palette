use std::fmt;
use std::path::{Component, Path};

const MAX_LEN: usize = 1024;

/// Relative path to a plan document, resolved against the workflow's Blueprint
/// directory.
///
/// Schemes (e.g. `plans://`, `repo://`), absolute paths, and parent-directory
/// traversal (`..`) are all rejected at parse time so plan resolution cannot
/// escape the Blueprint directory.
#[derive(Debug, Clone)]
pub struct PlanPath(String);

impl PlanPath {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidPlanPath> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidPlanPath::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(InvalidPlanPath::TooLong { len: s.len() });
        }
        if has_scheme(&s) {
            return Err(InvalidPlanPath::HasScheme);
        }
        let path = Path::new(&s);
        if path.is_absolute() {
            return Err(InvalidPlanPath::Absolute);
        }
        for component in path.components() {
            match component {
                Component::ParentDir => return Err(InvalidPlanPath::ParentTraversal),
                Component::Prefix(_) | Component::RootDir => {
                    return Err(InvalidPlanPath::Absolute);
                }
                _ => {}
            }
        }
        Ok(Self(s))
    }
}

fn has_scheme(s: &str) -> bool {
    let Some(idx) = s.find("://") else {
        return false;
    };
    let scheme = &s[..idx];
    !scheme.is_empty()
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

impl AsRef<str> for PlanPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<PlanPath> for String {
    fn from(p: PlanPath) -> Self {
        p.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidPlanPath {
    Empty,
    TooLong { len: usize },
    Absolute,
    ParentTraversal,
    HasScheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_relative_path() {
        let p = PlanPath::parse("plans/api/README.md").unwrap();
        assert_eq!(p.as_ref(), "plans/api/README.md");
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(PlanPath::parse(""), Err(InvalidPlanPath::Empty)));
        assert!(matches!(
            PlanPath::parse("   "),
            Err(InvalidPlanPath::Empty)
        ));
    }

    #[test]
    fn rejects_absolute_unix_path() {
        assert!(matches!(
            PlanPath::parse("/etc/passwd"),
            Err(InvalidPlanPath::Absolute)
        ));
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(matches!(
            PlanPath::parse("../outside.md"),
            Err(InvalidPlanPath::ParentTraversal)
        ));
        assert!(matches!(
            PlanPath::parse("plans/../../escape.md"),
            Err(InvalidPlanPath::ParentTraversal)
        ));
    }

    #[test]
    fn rejects_scheme_prefix() {
        assert!(matches!(
            PlanPath::parse("plans://2026/api"),
            Err(InvalidPlanPath::HasScheme)
        ));
        assert!(matches!(
            PlanPath::parse("repo://docs/plan.md"),
            Err(InvalidPlanPath::HasScheme)
        ));
        assert!(matches!(
            PlanPath::parse("file:///etc/passwd"),
            Err(InvalidPlanPath::HasScheme)
        ));
    }

    #[test]
    fn allows_paths_containing_colon_but_no_scheme() {
        // A colon alone (without ://) is not a scheme — accept it
        let p = PlanPath::parse("plans/foo:bar.md").unwrap();
        assert_eq!(p.as_ref(), "plans/foo:bar.md");
    }
}
