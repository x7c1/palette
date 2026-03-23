//! Mapping between domain enums and their integer IDs in lookup tables.
//!
//! These IDs correspond to the seed data in schema.rs.

mod craft_status;
pub use craft_status::{craft_status_from_id, craft_status_id};

mod job_status;
pub use job_status::{job_status_from_id, job_status_id};

mod priority;
pub use priority::{priority_from_id, priority_id};

mod job_type;
pub use job_type::{job_type_from_id, job_type_id};

mod review_status;
pub use review_status::{review_status_from_id, review_status_id};

mod task_status;
pub use task_status::{task_status_from_id, task_status_id};

mod verdict;
pub use verdict::{verdict_from_id, verdict_id};

mod workflow_status;
pub use workflow_status::{workflow_status_from_id, workflow_status_id};
