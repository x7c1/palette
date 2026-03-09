use palette_domain::Verdict;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictApi {
    Approved,
    ChangesRequested,
}

impl From<VerdictApi> for Verdict {
    fn from(v: VerdictApi) -> Self {
        match v {
            VerdictApi::Approved => Verdict::Approved,
            VerdictApi::ChangesRequested => Verdict::ChangesRequested,
        }
    }
}

impl From<Verdict> for VerdictApi {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Approved => VerdictApi::Approved,
            Verdict::ChangesRequested => VerdictApi::ChangesRequested,
        }
    }
}
