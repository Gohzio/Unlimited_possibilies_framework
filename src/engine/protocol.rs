use std::path::PathBuf;
use crate::model::message::Message;
use crate::model::event_result::NarrativeApplyReport;

pub enum EngineCommand {
    UserInput(String),
    LoadLlm(PathBuf),
}

pub enum EngineResponse {
    FullMessageHistory(Vec<Message>),

    NarrativeApplied {
        report: NarrativeApplyReport,
    }, 
}
