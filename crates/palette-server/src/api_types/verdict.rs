use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approved,
    ChangesRequested,
}

impl From<Verdict> for domain::Verdict {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Approved => domain::Verdict::Approved,
            Verdict::ChangesRequested => domain::Verdict::ChangesRequested,
        }
    }
}

impl From<domain::Verdict> for Verdict {
    fn from(v: domain::Verdict) -> Self {
        match v {
            domain::Verdict::Approved => Verdict::Approved,
            domain::Verdict::ChangesRequested => Verdict::ChangesRequested,
        }
    }
}
