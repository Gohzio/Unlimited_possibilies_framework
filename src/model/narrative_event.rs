use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};
use serde_json::Value;

use crate::model::game_state::QuestStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestStepUpdate {
    pub id: String,
    pub description: Option<String>,
    pub completed: Option<bool>,
}

fn deserialize_topics<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(vec![s]),
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    Value::String(s) => out.push(s),
                    _ => return Err(de::Error::custom("topics must be strings")),
                }
            }
            Ok(out)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(de::Error::custom("topics must be a string or array of strings")),
    }
}

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
        #[serde(alias = "notes")]
        details: Option<String>,
    },
    NpcJoinParty {
        id: String,
        name: Option<String>,
        role: Option<String>,
        #[serde(alias = "notes")]
        details: Option<String>,
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
        #[serde(default)]
        rewards: Option<Vec<String>>,
        #[serde(default, rename = "sub_quests", alias = "subquests", alias = "objectives")]
        sub_quests: Option<Vec<crate::model::game_state::QuestStep>>,
    },
    UpdateQuest {
        id: String,
        title: Option<String>,
        description: Option<String>,
        status: Option<QuestStatus>,
        rewards: Option<Vec<String>>,
        #[serde(rename = "sub_quests", alias = "subquests", alias = "objectives")]
        sub_quests: Option<Vec<QuestStepUpdate>>,
    },
    RequestContext {
        #[serde(default, alias = "topic", deserialize_with = "deserialize_topics")]
        topics: Vec<String>,
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
