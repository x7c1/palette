use palette_domain::job::JobId;
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;

pub fn wid(s: &str) -> WorkerId {
    WorkerId::parse(s).unwrap()
}

pub fn jid(s: &str) -> JobId {
    JobId::parse(s).unwrap()
}

pub fn tid(wf_id: &str, key_path: &str) -> TaskId {
    TaskId::parse(format!("{wf_id}:{key_path}")).unwrap()
}
