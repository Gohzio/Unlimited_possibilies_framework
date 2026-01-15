use std::path::PathBuf;
use crate::model::message::Message;

pub enum EngineCommand {
    UserInput(String),
    LoadLlm(PathBuf),
}

pub enum EngineResponse {
    FullMessageHistory(Vec<Message>),
}
