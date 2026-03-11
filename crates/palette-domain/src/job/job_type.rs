use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Craft,
    Review,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::Craft => "craft",
            JobType::Review => "review",
        }
    }
}

impl FromStr for JobType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "craft" => Ok(JobType::Craft),
            "review" => Ok(JobType::Review),
            _ => Err(format!("invalid job type: {s}")),
        }
    }
}
