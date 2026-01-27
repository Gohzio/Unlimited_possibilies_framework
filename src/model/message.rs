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

impl Message {
    pub fn as_text(&self) -> String {
        match self {
            Message::User(t) => format!("You: {}", t),

            Message::Roleplay { speaker, text } => {
                let tag = match speaker {
                    RoleplaySpeaker::Narrator => "[NARRATOR]",
                    RoleplaySpeaker::Npc => "[NPC]",
                    RoleplaySpeaker::PartyMember => "[PARTY]",
                };
                format!("{} {}", tag, text)
            }

            Message::System(t) => format!("[SYSTEM] {}", t),
        }
    }
}


