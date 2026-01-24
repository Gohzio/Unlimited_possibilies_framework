use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoleplaySpeaker {
    Narrator,
    Npc,
    PartyMember,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    User(String),
    Roleplay { speaker: RoleplaySpeaker, text: String },
    System(String),
}



