use crate::model::game_state::{GameStateSnapshot, Stat};
use std::collections::HashSet;

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
