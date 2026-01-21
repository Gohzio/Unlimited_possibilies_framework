use std::collections::{HashMap, HashSet};

use crate::model::game_state::{
    GameStateSnapshot,
    PlayerState,
    Stat,
    Power,
    PartyMember,
    Quest,
};

#[derive(Debug)]
pub struct InternalGameState {
    pub version: u32,

    pub player: PlayerState,

    /// Authoritative internal stats store
    /// Key = stat id (e.g. "strength", "souls")
    pub stats: HashMap<String, i32>,

    pub powers: HashMap<String, Power>,
    pub party: HashMap<String, PartyMember>,
    pub quests: HashMap<String, Quest>,

    pub flags: HashSet<String>,
}

impl From<&InternalGameState> for GameStateSnapshot {
    fn from(state: &InternalGameState) -> Self {
        GameStateSnapshot {
            version: state.version,
            player: state.player.clone(),
            stats: state.stats
                .iter()
                .map(|(id, value)| Stat {
                    id: id.clone(),
                    value: *value,
                })
                .collect(),
            powers: state.powers.values().cloned().collect(),
            party: state.party.values().cloned().collect(),
            quests: state.quests.values().cloned().collect(),
            flags: state.flags.iter().cloned().collect(),
        }
    }
}

impl Default for InternalGameState {
    fn default() -> Self {
        let mut stats = HashMap::new();
        stats.insert("strength".into(), 10);
        stats.insert("dexterity".into(), 10);
        stats.insert("intelligence".into(), 10);

        Self {
            version: 1,

            player: PlayerState {
                name: "Player".to_string(),
                level: 1,
                hp: 100,
                max_hp: 100,
            },

            stats,

            powers: HashMap::new(),
            party: HashMap::new(),
            quests: HashMap::new(),

            flags: HashSet::new(),
        }
    }
}
