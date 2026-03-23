use palette_domain::review::Verdict;

pub fn verdict_id(verdict: Verdict) -> i64 {
    match verdict {
        Verdict::Approved => 1,
        Verdict::ChangesRequested => 2,
    }
}

pub fn verdict_from_id(id: i64) -> Result<Verdict, String> {
    match id {
        1 => Ok(Verdict::Approved),
        2 => Ok(Verdict::ChangesRequested),
        _ => Err(format!("invalid verdict id: {id}")),
    }
}
