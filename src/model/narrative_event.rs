use serde::{Deserialize, Serialize};

/// Events requested by the narration LLM.
/// These are *requests*, not guarantees.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NarrativeEvent {
    /// Player gains a new ability or power
    GrantPower {
        power_id: String,
        name: String,
        description: String,
    },

    /// Add a party member
    AddPartyMember {
        character_id: String,
        name: String,
        role: String,
    },

    /// Start a quest
    StartQuest {
        quest_id: String,
        title: String,
        description: String,
    },

    /// Update an existing stat
    ModifyStat {
        stat: String,
        delta: i32,
    },

    /// Add an item to inventory
    AddItem {
        item_id: String,
        name: String,
        quantity: u32,
    },

    /// Request narrative rollback (rare, but powerful)
    RequestRetcon {
        reason: String,
    },
}
