use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]


pub enum NarrativeEvent {
    GrantPower {
        id: String,
        name: String,
        description: String,
    },

    AddPartyMember {
        id: String,
        name: String,
        role: String,
    },

    ModifyStat {
        stat: String,
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
}
impl NarrativeEvent {
    pub fn short_name(&self) -> &'static str {
        match self {
            NarrativeEvent::GrantPower { .. } => "GrantPower",
            NarrativeEvent::AddPartyMember { .. } => "AddPartyMember",
            NarrativeEvent::AddItem { .. } => "AddItem",
            NarrativeEvent::ModifyStat { .. } => "ModifyStat",
            NarrativeEvent::StartQuest { .. } => "StartQuest",
            NarrativeEvent::SetFlag { .. } => "SetFlag",
            NarrativeEvent::RequestRetcon { .. } => "RequestRetcon",
        }
    }
}

