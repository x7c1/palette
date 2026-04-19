const MAX_NAME_LEN: usize = 256;
const MAX_BRANCH_LEN: usize = 256;

/// Repository coordinates for a Craft job.
///
/// - `name`: the `owner/repo` slug.
/// - `work_branch`: the branch the Craft commits to. The Orchestrator creates
///   it when absent on the remote; the Worker never creates branches.
/// - `source_branch`: the branch `work_branch` is derived from when it does
///   not yet exist on the remote. When `None`, the Orchestrator falls back
///   to the repository's default branch.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Repository {
    pub name: String,
    pub work_branch: String,
    pub source_branch: Option<String>,
}

impl Repository {
    pub fn parse(
        name: impl Into<String>,
        work_branch: impl Into<String>,
        source_branch: Option<String>,
    ) -> Result<Self, InvalidRepository> {
        let name = name.into();
        let work_branch = work_branch.into();
        if name.is_empty() {
            return Err(InvalidRepository::NameEmpty);
        }
        if name.len() > MAX_NAME_LEN {
            return Err(InvalidRepository::NameTooLong { len: name.len() });
        }
        if work_branch.is_empty() {
            return Err(InvalidRepository::WorkBranchEmpty);
        }
        if work_branch.len() > MAX_BRANCH_LEN {
            return Err(InvalidRepository::WorkBranchTooLong {
                len: work_branch.len(),
            });
        }
        if let Some(ref sb) = source_branch {
            if sb.is_empty() {
                return Err(InvalidRepository::SourceBranchEmpty);
            }
            if sb.len() > MAX_BRANCH_LEN {
                return Err(InvalidRepository::SourceBranchTooLong { len: sb.len() });
            }
        }
        Ok(Self {
            name,
            work_branch,
            source_branch,
        })
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidRepository {
    NameEmpty,
    NameTooLong { len: usize },
    WorkBranchEmpty,
    WorkBranchTooLong { len: usize },
    SourceBranchEmpty,
    SourceBranchTooLong { len: usize },
}
