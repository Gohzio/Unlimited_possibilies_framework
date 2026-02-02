use crate::model::game_state::{GameStateSnapshot, Stat};
use crate::model::internal_game_state::InternalGameState;
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
            equipment: state.equipment.values().cloned().collect(),
            party: state.party.values().cloned().collect(),
            quests: state.quests.values().cloned().collect(),
            inventory: state.inventory.values().cloned().collect(),
            loot: state.loot.clone(),
            currencies: state.currencies
                .iter()
                .map(|(currency, amount)| crate::model::game_state::CurrencyBalance {
                    currency: currency.clone(),
                    amount: *amount,
                })
                .collect(),
            npcs: state.npcs.values().cloned().collect(),
            relationships: state.relationships.values().cloned().collect(),
            factions: state.factions.values().cloned().collect(),
            flags: state.flags.iter().cloned().collect(),
        }
    }
}
