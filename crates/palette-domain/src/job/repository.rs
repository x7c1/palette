const MAX_NAME_LEN: usize = 256;
const MAX_BRANCH_LEN: usize = 256;

/// Repository coordinates for a Craft job.
///
/// - `name`: the `owner/repo` slug.
/// - `branch`: the **work branch** that this Craft commits to. The Orchestrator
///   creates it when absent on the remote.
/// - `source_branch`: the branch this Craft derives from when the work branch
///   does not yet exist on the remote. When `None`, the Orchestrator falls
///   back to the repository's default branch (`refs/remotes/origin/HEAD`).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Repository {
    pub name: String,
    pub branch: String,
    pub source_branch: Option<String>,
}

impl Repository {
    pub fn parse(
        name: impl Into<String>,
        branch: impl Into<String>,
        source_branch: Option<String>,
    ) -> Result<Self, InvalidRepository> {
        let name = name.into();
        let branch = branch.into();
        if name.is_empty() {
            return Err(InvalidRepository::NameEmpty);
        }
        if name.len() > MAX_NAME_LEN {
            return Err(InvalidRepository::NameTooLong { len: name.len() });
        }
        if branch.is_empty() {
            return Err(InvalidRepository::BranchEmpty);
        }
        if branch.len() > MAX_BRANCH_LEN {
            return Err(InvalidRepository::BranchTooLong { len: branch.len() });
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
            branch,
            source_branch,
        })
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidRepository {
    NameEmpty,
    NameTooLong { len: usize },
    BranchEmpty,
    BranchTooLong { len: usize },
    SourceBranchEmpty,
    SourceBranchTooLong { len: usize },
}
