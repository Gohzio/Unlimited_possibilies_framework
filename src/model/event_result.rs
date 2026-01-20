use serde::{Deserialize, Serialize};
use crate::model::narrative_event::NarrativeEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EventApplyOutcome {
    Applied,

    Rejected {
        reason: String,
    },

    Deferred {
        reason: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventApplication {
    pub event: NarrativeEvent,
    pub outcome: EventApplyOutcome,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeApplyReport {
    /// One entry per requested event, in order
    pub applications: Vec<EventApplication>,
}
