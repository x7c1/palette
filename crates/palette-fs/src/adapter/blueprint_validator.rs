use crate::blueprint::TaskNode;
use palette_domain::job::{
    JobDetail, JobType, PerspectiveName, PullRequest, Repository, ReviewTarget,
};
use palette_domain::task::TaskKey;
use std::collections::{HashMap, HashSet};

use super::blueprint_error::BlueprintError;

pub(super) struct BlueprintValidator<'a> {
    known_perspectives: &'a HashSet<String>,
}

/// Result of validating a Blueprint node tree.
pub(super) struct Validated<'a> {
    pub keys: HashMap<&'a str, TaskKey>,
    /// Parsed job details keyed by the raw task key string.
    /// `None` value means the node has no job_type (composite-only).
    pub job_details: HashMap<&'a str, Option<JobDetail>>,
}

/// Per-node validation result.
type NodeResult<'a> = (
    Vec<BlueprintError>,
    HashMap<&'a str, TaskKey>,
    HashMap<&'a str, Option<JobDetail>>,
);

impl<'a> BlueprintValidator<'a> {
    pub fn new(known_perspectives: &'a HashSet<String>) -> Self {
        Self { known_perspectives }
    }

    /// Validate all nodes recursively. Returns parsed keys and job details on
    /// success, or all collected errors on failure.
    pub fn validate<'n>(&self, root: &'n TaskNode) -> Result<Validated<'n>, Vec<BlueprintError>> {
        let (errors, keys, job_details) = self.validate_node(root);
        if errors.is_empty() {
            Ok(Validated { keys, job_details })
        } else {
            Err(errors)
        }
    }

    fn validate_node<'n>(&self, node: &'n TaskNode) -> NodeResult<'n> {
        let (key_errors, keys) = collect_keys(node);
        let structure_errors = check_craft_has_review(node);
        let (detail_errors, job_detail) = self.build_job_detail(node);
        let dep_errors = validate_depends_on(node);

        let (child_errors, child_keys, child_details) =
            node.children.iter().map(|c| self.validate_node(c)).fold(
                (Vec::new(), HashMap::new(), HashMap::new()),
                |(mut errs, mut keys, mut details), (ce, ck, cd)| {
                    errs.extend(ce);
                    keys.extend(ck);
                    details.extend(cd);
                    (errs, keys, details)
                },
            );

        let errors = key_errors
            .into_iter()
            .chain(structure_errors)
            .chain(detail_errors)
            .chain(dep_errors)
            .chain(child_errors)
            .collect();

        let mut all_keys = keys;
        all_keys.extend(child_keys);

        let mut all_details = HashMap::new();
        all_details.insert(node.key.as_str(), job_detail);
        all_details.extend(child_details);

        (errors, all_keys, all_details)
    }

    /// Validate constraints and build [`JobDetail`] for this node.
    ///
    /// Returns (errors, parsed job detail). On error the detail is `None`; the
    /// caller still collects the error so that all problems are reported at once.
    fn build_job_detail(&self, node: &TaskNode) -> (Vec<BlueprintError>, Option<JobDetail>) {
        let Some(job_type_yaml) = node.job_type else {
            let mut errors = validate_repository_format(node);
            errors.extend(perspective_on_non_review(node));
            return (errors, None);
        };

        let job_type = JobType::from(job_type_yaml);
        match job_type {
            JobType::Craft => {
                let mut errors: Vec<BlueprintError> =
                    perspective_on_non_review(node).into_iter().collect();
                match node.repository.as_ref() {
                    None => {
                        errors.push(BlueprintError::MissingRepository {
                            task_key: node.key.clone(),
                        });
                        (errors, None)
                    }
                    Some(repo) => match Repository::parse(&repo.name, &repo.branch) {
                        Ok(repository) => (errors, Some(JobDetail::Craft { repository })),
                        Err(cause) => {
                            errors.push(BlueprintError::InvalidRepository {
                                task_key: node.key.clone(),
                                cause,
                            });
                            (errors, None)
                        }
                    },
                }
            }
            JobType::Review => {
                let mut errors = validate_repository_format(node);
                let perspective = self.build_perspective(node, &mut errors);
                let target = self.build_review_target(node, &mut errors);
                (
                    errors,
                    Some(JobDetail::Review {
                        perspective,
                        target,
                    }),
                )
            }
            JobType::ReviewIntegrate => {
                let mut errors = validate_repository_format(node);
                errors.extend(perspective_on_non_review(node));
                let target = self.build_review_target(node, &mut errors);
                (errors, Some(JobDetail::ReviewIntegrate { target }))
            }
            JobType::Orchestrator => {
                let mut errors = validate_repository_format(node);
                errors.extend(perspective_on_non_review(node));
                (
                    errors,
                    Some(JobDetail::Orchestrator {
                        command: node.command.clone(),
                    }),
                )
            }
            JobType::Operator => {
                let mut errors = validate_repository_format(node);
                errors.extend(perspective_on_non_review(node));
                (errors, Some(JobDetail::Operator))
            }
        }
    }

    /// Parse and validate perspective for a review task node.
    /// On validation failure, pushes errors and returns `None`.
    fn build_perspective(
        &self,
        node: &TaskNode,
        errors: &mut Vec<BlueprintError>,
    ) -> Option<PerspectiveName> {
        let raw = node.perspective.as_ref()?;

        if !self.known_perspectives.contains(raw) {
            errors.push(BlueprintError::UnknownPerspective {
                task_key: node.key.clone(),
                perspective: raw.clone(),
            });
            return None;
        }

        match PerspectiveName::parse(raw) {
            Ok(name) => Some(name),
            Err(_) => {
                errors.push(BlueprintError::UnknownPerspective {
                    task_key: node.key.clone(),
                    perspective: raw.clone(),
                });
                None
            }
        }
    }

    /// Build [`ReviewTarget`] from the node's `pull_request` field.
    /// Returns `CraftOutput` when no pull_request is specified.
    fn build_review_target(
        &self,
        node: &TaskNode,
        errors: &mut Vec<BlueprintError>,
    ) -> ReviewTarget {
        let Some(pr_yaml) = node.pull_request.clone() else {
            return ReviewTarget::CraftOutput;
        };
        match PullRequest::parse(pr_yaml.owner, pr_yaml.repo, pr_yaml.number) {
            Ok(pr) => ReviewTarget::PullRequest(pr),
            Err(cause) => {
                errors.push(BlueprintError::InvalidPullRequest {
                    task_key: node.key.clone(),
                    cause,
                });
                ReviewTarget::CraftOutput
            }
        }
    }
}

/// Parse the node's own key and depends_on keys.
fn collect_keys(node: &TaskNode) -> (Vec<BlueprintError>, HashMap<&str, TaskKey>) {
    std::iter::once(node.key.as_str())
        .chain(node.depends_on.iter().map(String::as_str))
        .fold(
            (Vec::new(), HashMap::new()),
            |(mut errors, mut keys), raw| {
                match TaskKey::parse(raw) {
                    Ok(k) => {
                        keys.insert(raw, k);
                    }
                    Err(e) => errors.push(BlueprintError::InvalidKey(e)),
                }
                (errors, keys)
            },
        )
}

/// Check that craft tasks have at least one review child.
fn check_craft_has_review(node: &TaskNode) -> Option<BlueprintError> {
    let job_type = node.job_type?;
    if !matches!(JobType::from(job_type), JobType::Craft) {
        return None;
    }
    let has_review = node.children.iter().any(|c| {
        c.job_type.is_some_and(|jt| {
            matches!(
                JobType::from(jt),
                JobType::Review | JobType::ReviewIntegrate
            )
        })
    });
    if has_review {
        None
    } else {
        Some(BlueprintError::MissingReviewChild {
            task_key: node.key.clone(),
        })
    }
}

/// Validate repository format (if present) for non-craft nodes.
fn validate_repository_format(node: &TaskNode) -> Vec<BlueprintError> {
    let Some(repo) = node.repository.as_ref() else {
        return vec![];
    };
    match Repository::parse(&repo.name, &repo.branch) {
        Ok(_) => vec![],
        Err(cause) => vec![BlueprintError::InvalidRepository {
            task_key: node.key.clone(),
            cause,
        }],
    }
}

/// If a non-review node has a `perspective` field, report an error.
fn perspective_on_non_review(node: &TaskNode) -> Option<BlueprintError> {
    node.perspective
        .as_ref()
        .map(|_| BlueprintError::PerspectiveOnNonReview {
            task_key: node.key.clone(),
        })
}

/// Check depends_on for self-dependency and duplicates.
fn validate_depends_on(node: &TaskNode) -> Vec<BlueprintError> {
    node.depends_on
        .iter()
        .fold(
            (Vec::new(), HashSet::new()),
            |(mut errors, mut seen), dep| {
                if dep == &node.key {
                    errors.push(BlueprintError::SelfDependency {
                        task_key: node.key.clone(),
                    });
                } else if !seen.insert(dep.as_str()) {
                    errors.push(BlueprintError::DuplicateDependency {
                        task_key: node.key.clone(),
                        dep: dep.clone(),
                    });
                }
                (errors, seen)
            },
        )
        .0
}
