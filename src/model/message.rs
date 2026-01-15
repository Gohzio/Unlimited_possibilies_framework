#[derive(Clone)]
pub enum Message {
    User(String),
    System(String),
    Roleplay(String),
}
