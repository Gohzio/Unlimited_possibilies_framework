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
    #[serde(default)]
    pub equipment: Vec<EquippedItem>,
    pub party: Vec<PartyMember>,
    pub quests: Vec<Quest>,
    pub inventory: Vec<ItemStack>,
    pub loot: Vec<LootDrop>,
    pub currencies: Vec<CurrencyBalance>,
    pub npcs: Vec<Npc>,
    pub relationships: Vec<Relationship>,
    #[serde(default)]
    pub factions: Vec<FactionRep>,

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
    #[serde(default)]
    pub exp: i32,
    #[serde(default = "default_exp_to_next")]
    pub exp_to_next: i32,
    #[serde(default = "default_exp_multiplier")]
    pub exp_multiplier: f32,
    pub hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub armor: Vec<String>,
    #[serde(default)]
    pub clothing: Vec<String>,
}

fn default_exp_to_next() -> i32 {
    100
}

fn default_exp_multiplier() -> f32 {
    2.0
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
    #[serde(default)]
    pub details: String,
    pub hp: i32,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub armor: Vec<String>,
    #[serde(default)]
    pub clothing: Vec<String>,
    #[serde(default)]
    pub lock_name: bool,
    #[serde(default)]
    pub lock_role: bool,
    #[serde(default)]
    pub lock_details: bool,
    #[serde(default)]
    pub lock_weapons: bool,
    #[serde(default)]
    pub lock_armor: bool,
    #[serde(default)]
    pub lock_clothing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: QuestStatus,
    #[serde(default)]
    pub difficulty: Option<String>,
    #[serde(default)]
    pub negotiable: bool,
    #[serde(default)]
    pub reward_options: Vec<String>,
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
    #[serde(default)]
    pub set_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootDrop {
    pub item: String,
    pub quantity: u32,
    pub description: Option<String>,
    #[serde(default)]
    pub set_id: Option<String>,
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
    #[serde(default = "default_true")]
    pub nearby: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub subject_id: String,
    pub target_id: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquippedItem {
    pub item_id: String,
    pub slot: String,
    #[serde(default)]
    pub set_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionRep {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub reputation: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    Completed,
    Failed,
}
