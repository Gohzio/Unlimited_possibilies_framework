#[derive(Clone)]
pub enum RoleplaySpeaker {
    Narrator,
    PartyMember,
    Npc,
}

#[derive(Clone)]
pub enum Message {
    User(String),
    Roleplay {
        speaker: RoleplaySpeaker,
        text: String,
    },
    System(String),
}


