use std::collections::HashMap;

use crate::model::game_state::{
    GameStateSnapshot,
    PlayerState,
    PlayerStats,
    Power,
    PartyMember,
    Quest,
};

#[derive(Debug)]
pub struct InternalGameState {
    pub version: u32,

    pub player: PlayerState,
    pub stats: PlayerStats,

    pub powers: HashMap<String, Power>,
    pub party: HashMap<String, PartyMember>,
    pub quests: HashMap<String, Quest>,

    pub flags: Vec<String>,
}

impl InternalGameState {
    /// Produce a read-only snapshot for LLMs / UI
    pub fn snapshot(&self) -> GameStateSnapshot {
        GameStateSnapshot {
            version: self.version,
            player: self.player.clone(),
            stats: self.stats.clone(),
            powers: self.powers.values().cloned().collect(),
            party: self.party.values().cloned().collect(),
            quests: self.quests.values().cloned().collect(),
            flags: self.flags.clone(),
        }
    }
}

impl Default for InternalGameState {
    fn default() -> Self {
        Self {
            version: 1,

            player: PlayerState {
                name: "Player".to_string(),
                level: 1,
                hp: 100,
                max_hp: 100,
            },

            stats: PlayerStats {
                strength: 10,
                dexterity: 10,
                intelligence: 10,
            },

            powers: HashMap::new(),
            party: HashMap::new(),
            quests: HashMap::new(),

            flags: Vec::new(),
        }
    }
}
