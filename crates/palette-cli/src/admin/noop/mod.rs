pub(super) mod blueprint;
pub(super) mod container;
pub(super) mod github_review;
pub(super) mod terminal;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

fn unsupported<T>(name: &str) -> Result<T, BoxErr> {
    Err(std::io::Error::other(format!("{name} is not available in admin mode")).into())
}
