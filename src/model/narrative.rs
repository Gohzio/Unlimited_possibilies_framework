use serde::{Deserialize, Serialize};

/// Output returned by the Narrator LLM.
/// This does NOT mutate state directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeResponse {
    pub text: Vec<NarrativeLine>,
    pub events: Vec<NarrativeEvent>,
}

/// A single piece of narration or dialogue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeLine {
    pub speaker: Speaker,
    pub content: String,

    /// Optional speaker name (for party members / NPCs)
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Narrator,
    PartyMember,
    Npc,
    System,
}

/// A *proposal* to modify game state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrativeEvent {
    GrantPower {
        power_id: String,
    },

    AddPartyMember {
        member_id: String,
    },

    UpdateQuest {
        quest_id: String,
        new_status: QuestStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    Completed,
    Failed,
}

