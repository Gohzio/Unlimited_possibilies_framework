use serde::{Deserialize, Serialize};

use crate::model::internal_game_state::InternalGameState;
use crate::model::message::Message;
use crate::ui::app::{CharacterDefinition, PartyMember, WorldDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSave {
    pub version: u32,
    pub world: WorldDefinition,
    pub player: CharacterDefinition,
    pub party: Vec<PartyMember>,
    pub messages: Vec<Message>,
    pub internal_state: InternalGameState,
    #[serde(default)]
    pub speaker_colors: crate::ui::app::SpeakerColors,
    #[serde(default)]
    pub character_image_rgba: Option<Vec<u8>>,
    #[serde(default)]
    pub character_image_size: Option<(u32, u32)>,
}
