use serde::{Deserialize, Serialize};

/// Events requested by the Narrative LLM.
/// These are *intentions*, not guarantees.
/// The engine validates and applies them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrativeEvent {
    /// Player gains a new power or ability
    GrantPower {
        id: String,
        name: String,
        description: String,
    },

    /// Player loses a power
    RevokePower {
        id: String,
    },

    /// Add a party member
    AddPartyMember {
        id: String,
        name: String,
        role: String,
        hp: i32,
    },

    /// Remove a party member
    RemovePartyMember {
        id: String,
    },

    /// Create or update a quest
    UpdateQuest {
        id: String,
        title: String,
        status: QuestStatus,
    },

    /// Set a narrative flag
    SetFlag {
        flag: String,
    },

    /// Clear a narrative flag
    ClearFlag {
        flag: String,
    },
}

/// Quest lifecycle status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    Completed,
    Failed,
}
