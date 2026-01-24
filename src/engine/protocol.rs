use std::path::PathBuf;

use crate::model::message::Message;
use crate::model::event_result::NarrativeApplyReport;
use crate::model::game_state::GameStateSnapshot;
use crate::model::game_context::GameContext;

#[derive(Debug)]
pub enum EngineCommand {
    /// Player typed something in the chat box
    SubmitPlayerInput {
        text: String,
        context: GameContext,
    },

    /// Load / switch LLM backend (LM Studio, etc.)
    ConnectToLlm,
    LoadLlm(PathBuf),
}

#[derive(Debug)]
pub enum EngineResponse {
    FullMessageHistory(Vec<Message>),
    NarrativeApplied {
        report: NarrativeApplyReport,
        snapshot: GameStateSnapshot,
    },
    LlmConnectionResult {
        success: bool,
        message: String,
    },
}

