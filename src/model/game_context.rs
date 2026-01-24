use serde::{Deserialize, Serialize};

use crate::model::message::Message;
use crate::model::game_state::GameStateSnapshot;
use crate::ui::app::{WorldDefinition, CharacterDefinition, PartyMember};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameContext {
    pub world: WorldDefinition,
    pub player: CharacterDefinition,
    pub party: Vec<PartyMember>,
    pub history: Vec<Message>,
    pub snapshot: Option<GameStateSnapshot>,
}
