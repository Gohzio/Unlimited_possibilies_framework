use serde::{Deserialize, Serialize};

/// A full snapshot of the game state sent to LLMs.
/// This is READ-ONLY outside the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    pub version: u32,

    pub player: PlayerState,
    pub stats: PlayerStats,

    pub powers: Vec<Power>,
    pub party: Vec<PartyMember>,
    pub quests: Vec<Quest>,

    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStats {
    pub strength: i32,
    pub dexterity: i32,
    pub intelligence: i32,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quest {
    pub id: String,
    pub title: String,
    pub status: QuestStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Active,
    Completed,
    Failed,
}
