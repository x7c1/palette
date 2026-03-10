use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approved,
    ChangesRequested,
}

impl From<Verdict> for domain::review::Verdict {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Approved => domain::review::Verdict::Approved,
            Verdict::ChangesRequested => domain::review::Verdict::ChangesRequested,
        }
    }
}

impl From<domain::review::Verdict> for Verdict {
    fn from(v: domain::review::Verdict) -> Self {
        match v {
            domain::review::Verdict::Approved => Verdict::Approved,
            domain::review::Verdict::ChangesRequested => Verdict::ChangesRequested,
        }
    }
}
