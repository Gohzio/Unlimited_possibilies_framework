use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventResult {
    Applied,
    Rejected { reason: String },
    Deferred { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeApplyReport {
    pub results: Vec<EventResult>,
}
