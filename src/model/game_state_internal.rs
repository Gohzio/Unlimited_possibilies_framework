use crate::model::game_state::GameStateSnapshot;
use crate::model::game_state::{
    PlayerState,
    PlayerStats,
    Power,
    PartyMember,
    Quest,
};

/// Authoritative, mutable game state.
/// NEVER sent directly to an LLM.
#[derive(Debug)]
pub struct GameState {
    version: u32,

    player: PlayerState,
    stats: PlayerStats,

    powers: Vec<Power>,
    party: Vec<PartyMember>,
    quests: Vec<Quest>,

    flags: Vec<String>,
}

impl GameState {
    /// Create a brand-new game state
    pub fn new() -> Self {
        Self {
            version: 1,
            player: PlayerState {
                name: "Player".into(),
                level: 1,
                hp: 10,
                max_hp: 10,
            },
            stats: PlayerStats {
                strength: 1,
                dexterity: 1,
                intelligence: 1,
            },
            powers: Vec::new(),
            party: Vec::new(),
            quests: Vec::new(),
            flags: Vec::new(),
        }
    }

    /// Produce a read-only snapshot for LLMs
    pub fn snapshot(&self) -> GameStateSnapshot {
        GameStateSnapshot {
            version: self.version,
            player: self.player.clone(),
            stats: self.stats.clone(),
            powers: self.powers.clone(),
            party: self.party.clone(),
            quests: self.quests.clone(),
            flags: self.flags.clone(),
        }
    }
}
