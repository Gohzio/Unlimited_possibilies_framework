use std::path::PathBuf;
use crate::model::message::Message;
use crate::model::event_result::NarrativeApplyReport;
use crate::model::game_state::GameStateSnapshot;

pub enum EngineCommand {
    UserInput(String),
    LoadLlm(PathBuf),
}

pub enum EngineResponse {
    NarrativeApplied {
        report: NarrativeApplyReport,
        snapshot: GameStateSnapshot,
    },
    FullMessageHistory(Vec<Message>),
}
