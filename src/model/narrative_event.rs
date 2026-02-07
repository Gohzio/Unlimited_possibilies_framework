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
    Craft {
        recipe: String,
        #[serde(default)]
        quantity: Option<u32>,
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        result: Option<String>,
        #[serde(default)]
        set_id: Option<String>,
    },
    Gather {
        resource: String,
        #[serde(default)]
        quantity: Option<u32>,
        #[serde(default)]
        quality: Option<String>,
        #[serde(default)]
        set_id: Option<String>,
    },

    AddPartyMember {
        id: String,
        name: String,
        role: String,
    },
    PartyUpdate {
        id: String,
        name: Option<String>,
        role: Option<String>,
        details: Option<String>,
        #[serde(default, alias = "clothing")]
        clothing_add: Option<Vec<String>>,
        #[serde(default)]
        clothing_remove: Option<Vec<String>>,
        #[serde(default, alias = "weapons")]
        weapons_add: Option<Vec<String>>,
        #[serde(default)]
        weapons_remove: Option<Vec<String>>,
        #[serde(default, alias = "armor")]
        armor_add: Option<Vec<String>>,
        #[serde(default)]
        armor_remove: Option<Vec<String>>,
    },
    SectionCardUpsert {
        section: String,
        id: String,
        name: String,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        details: Option<String>,
        #[serde(default)]
        notes: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        items: Option<Vec<String>>,
    },
    SectionCardRemove {
        section: String,
        id: String,
    },
    PlayerCardUpdate {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        details: Option<String>,
        #[serde(default)]
        notes: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        items: Option<Vec<String>>,
    },
    TimePassed {
        minutes: u32,
        #[serde(default)]
        reason: Option<String>,
    },
    NpcSpawn {
        #[serde(default)]
        id: Option<String>,
        name: String,
        role: String,
        #[serde(alias = "notes")]
        details: Option<String>,
    },
    NpcJoinParty {
        #[serde(default)]
        id: Option<String>,
        name: Option<String>,
        role: Option<String>,
        #[serde(alias = "notes")]
        details: Option<String>,
        #[serde(default)]
        clothing: Option<Vec<String>>,
        #[serde(default)]
        weapons: Option<Vec<String>>,
        #[serde(default)]
        armor: Option<Vec<String>>,
    },
    NpcUpdate {
        #[serde(default)]
        id: Option<String>,
        name: Option<String>,
        role: Option<String>,
        #[serde(alias = "notes")]
        details: Option<String>,
    },
    NpcDespawn {
        id: String,
        reason: Option<String>,
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
    AddExp {
        amount: i32,
    },
    LevelUp {
        levels: u32,
    },
    EquipItem {
        item_id: String,
        slot: String,
        #[serde(default)]
        set_id: Option<String>,
        #[serde(default)]
        description: Option<String>,
    },
    UnequipItem {
        item_id: String,
    },

    StartQuest {
        id: String,
        title: String,
        description: String,
        #[serde(default)]
        difficulty: Option<String>,
        #[serde(default)]
        negotiable: Option<bool>,
        #[serde(default)]
        reward_options: Option<Vec<String>>,
        #[serde(default)]
        rewards: Option<Vec<String>>,
        #[serde(default, rename = "sub_quests", alias = "subquests", alias = "objectives")]
        sub_quests: Option<Vec<crate::model::game_state::QuestStep>>,
        #[serde(default)]
        declinable: Option<bool>,
    },
    UpdateQuest {
        id: String,
        title: Option<String>,
        description: Option<String>,
        status: Option<QuestStatus>,
        #[serde(default)]
        difficulty: Option<String>,
        #[serde(default)]
        negotiable: Option<bool>,
        #[serde(default)]
        reward_options: Option<Vec<String>>,
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
        #[serde(default)]
        set_id: Option<String>,
    },
    #[serde(rename = "drop")]
    Drop {
        item: String,
        quantity: Option<i32>,
        description: Option<String>,
        #[serde(default)]
        set_id: Option<String>,
    },
    SpawnLoot {
        item: String,
        quantity: Option<i32>,
        description: Option<String>,
        #[serde(default)]
        set_id: Option<String>,
    },
    CurrencyChange {
        currency: String,
        delta: i32,
    },
    FactionSpawn {
        id: String,
        name: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        description: Option<String>,
    },
    FactionUpdate {
        id: String,
        name: Option<String>,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        description: Option<String>,
    },
    FactionRepChange {
        id: String,
        delta: i32,
    },
    Unknown {
        event_type: String,
        raw: serde_json::Value,
    },
}
