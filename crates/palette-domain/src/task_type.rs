use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Work,
    Review,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Work => "work",
            TaskType::Review => "review",
        }
    }
}

impl FromStr for TaskType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "work" => Ok(TaskType::Work),
            "review" => Ok(TaskType::Review),
            _ => Err(format!("invalid task type: {s}")),
        }
    }
}
