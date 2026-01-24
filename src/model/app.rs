use crate::model::game_context::GameContext;
use crate::ui::app::{WorldDefinition, CharacterDefinition, PartyMember};

#[derive(Debug, Clone)]
pub struct GameApp {
    pub world: WorldDefinition,
    pub player: CharacterDefinition,
    pub party: Vec<PartyMember>,
}

impl Default for GameApp {
    fn default() -> Self {
        Self {
            world: WorldDefinition::default(),
            player: CharacterDefinition::default(),
            party: Vec::new(),
        }
    }
}

impl GameApp {
    pub fn build_context(
        &self,
        history: Vec<crate::model::message::Message>,
        snapshot: Option<crate::model::game_state::GameStateSnapshot>,
    ) -> GameContext {
        GameContext {
            world: self.world.clone(),
            player: self.player.clone(),
            party: self.party.clone(),
            history,
            snapshot,
        }
    }
}
