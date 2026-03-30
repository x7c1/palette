const MAX_NAME_LEN: usize = 256;
const MAX_BRANCH_LEN: usize = 256;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Repository {
    pub name: String,
    pub branch: String,
}

impl Repository {
    pub fn parse(
        name: impl Into<String>,
        branch: impl Into<String>,
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
        Ok(Self { name, branch })
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "repository"]
pub enum InvalidRepository {
    NameEmpty,
    NameTooLong { len: usize },
    BranchEmpty,
    BranchTooLong { len: usize },
}
