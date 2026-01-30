use crate::model::message::Message;
use crate::model::event_result::NarrativeApplyReport;
use crate::model::game_state::GameStateSnapshot;
use crate::model::game_context::GameContext;
use crate::model::game_save::GameSave;

#[derive(Debug)]
pub enum EngineCommand {
    /// Player typed something in the chat box
    SubmitPlayerInput {
        text: String,
        context: GameContext,
    },

    /// Initialize narrative with opening message (world load)
    InitializeNarrative {
        opening_message: String,
    },

    /// Load / switch LLM backend
    ConnectToLlm,

    /// UI-driven: move an NPC into the party without LLM involvement
    AddNpcToParty {
        id: String,
        name: String,
        role: String,
        details: String,
    },

    SaveGame {
        path: std::path::PathBuf,
        world: crate::ui::app::WorldDefinition,
        player: crate::ui::app::CharacterDefinition,
        party: Vec<crate::ui::app::PartyMember>,
    },

    LoadGame {
        path: std::path::PathBuf,
    },
}


#[derive(Debug)]
pub enum EngineResponse {
    FullMessageHistory(Vec<Message>),
    NarrativeApplied {
        report: NarrativeApplyReport,
        snapshot: GameStateSnapshot,
    },
    GameLoaded {
        save: GameSave,
        snapshot: GameStateSnapshot,
    },
    LlmConnectionResult {
        success: bool,
        message: String,
    },
}
