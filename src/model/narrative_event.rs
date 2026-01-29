use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrativeEvent {
    GrantPower {
        id: String,
        name: String,
        description: String,
    },
    Combat {
        description: String,
    },

    Dialogue {
        speaker: String,
        text: String,
    },
    Travel {
        from: String,
        to: String,
    },
    Rest {
        description: String,
    },

    AddPartyMember {
        id: String,
        name: String,
        role: String,
    },
    NpcSpawn {
        id: String,
        name: String,
        role: String,
        notes: Option<String>,
    },
    NpcJoinParty {
        id: String,
        name: Option<String>,
        role: Option<String>,
        notes: Option<String>,
    },
    NpcLeaveParty {
        id: String,
    },
    RelationshipChange {
        subject_id: String,
        target_id: String,
        delta: i32,
    },

    ModifyStat {
        stat_id: String,
        delta: i32,
    },

    StartQuest {
        id: String,
        title: String,
        description: String,
    },

    SetFlag {
        flag: String,
    },

    RequestRetcon {
        reason: String,
    },

    AddItem {
        item_id: String,
        quantity: u32,
    },
    #[serde(rename = "drop")]
    Drop {
        item: String,
        quantity: Option<i32>,
        description: Option<String>,
    },
    SpawnLoot {
        item: String,
        quantity: Option<i32>,
        description: Option<String>,
    },
    CurrencyChange {
        currency: String,
        delta: i32,
    },
    Unknown {
        event_type: String,
        raw: serde_json::Value,
    },
}

impl NarrativeEvent {
    pub fn short_name(&self) -> &'static str {
        match self {
            NarrativeEvent::GrantPower { .. } => "GrantPower",
            NarrativeEvent::AddPartyMember { .. } => "AddPartyMember",
            NarrativeEvent::NpcSpawn { .. } => "NpcSpawn",
            NarrativeEvent::NpcJoinParty { .. } => "NpcJoinParty",
            NarrativeEvent::NpcLeaveParty { .. } => "NpcLeaveParty",
            NarrativeEvent::RelationshipChange { .. } => "RelationshipChange",
            NarrativeEvent::AddItem { .. } => "AddItem",
            NarrativeEvent::Drop { .. } => "Drop",
            NarrativeEvent::SpawnLoot { .. } => "SpawnLoot",
            NarrativeEvent::CurrencyChange { .. } => "CurrencyChange",
            NarrativeEvent::ModifyStat { .. } => "ModifyStat",
            NarrativeEvent::StartQuest { .. } => "StartQuest",
            NarrativeEvent::SetFlag { .. } => "SetFlag",
            NarrativeEvent::RequestRetcon { .. } => "RequestRetcon",
            NarrativeEvent::Combat { .. } => "Combat",
            NarrativeEvent::Dialogue { .. } => "Dialogue",
            NarrativeEvent::Travel { .. } => "Travel",
            NarrativeEvent::Rest { .. } => "Rest",
            NarrativeEvent::Unknown { .. } => "Unknown",
        }
    }
}
