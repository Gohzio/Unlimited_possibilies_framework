use serde::{Deserialize, Serialize};

/// A full snapshot of the game state sent to LLMs.
/// This is READ-ONLY outside the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    pub version: u32,

    pub player: PlayerState,

    /// Dynamic, user-defined stats (e.g. strength, souls, corruption)
    pub stats: Vec<Stat>,

    pub powers: Vec<Power>,
    pub party: Vec<PartyMember>,
    pub quests: Vec<Quest>,
    pub inventory: Vec<ItemStack>,
    pub loot: Vec<LootDrop>,
    pub currencies: Vec<CurrencyBalance>,
    pub npcs: Vec<Npc>,
    pub relationships: Vec<Relationship>,

    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stat {
    pub id: String,
    pub value: i32, 
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub armor: Vec<String>,
    #[serde(default)]
    pub clothing: Vec<String>,
}
   

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Power {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyMember {
    pub id: String,
    pub name: String,
    pub role: String,
    pub hp: i32,
    pub clothing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: QuestStatus,
    #[serde(default)]
    pub rewards: Vec<String>,
    #[serde(default)]
    pub sub_quests: Vec<QuestStep>,
    #[serde(default)]
    pub rewards_claimed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestStep {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: String,
    pub quantity: u32,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootDrop {
    pub item: String,
    pub quantity: u32,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyBalance {
    pub currency: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Npc {
    pub id: String,
    pub name: String,
    pub role: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub subject_id: String,
    pub target_id: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    Completed,
    Failed,
}
