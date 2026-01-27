use serde::{Deserialize, Serialize};

use crate::model::narrative_event;

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
}

}

impl NarrativeEvent {
    pub fn short_name(&self) -> &'static str {
        match self {
            NarrativeEvent::GrantPower { .. } => "GrantPower",
            NarrativeEvent::AddPartyMember { .. } => "AddPartyMember",
            NarrativeEvent::AddItem { .. } => "AddItem",
            NarrativeEvent::Drop { .. } => "Drop",
            NarrativeEvent::ModifyStat { .. } => "ModifyStat",
            NarrativeEvent::StartQuest { .. } => "StartQuest",
            NarrativeEvent::SetFlag { .. } => "SetFlag",
            NarrativeEvent::RequestRetcon { .. } => "RequestRetcon",
            NarrativeEvent::Combat { .. } => "Combat",
            NarrativeEvent::Dialogue { .. } => "Dialogue",
            NarrativeEvent::Travel { .. } => "Travel",
            NarrativeEvent::Rest { .. } => "Rest",
        }
    }
}
