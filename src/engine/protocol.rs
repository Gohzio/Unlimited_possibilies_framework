use crate::model::message::Message;
use crate::model::event_result::NarrativeApplyReport;
use crate::model::game_state::GameStateSnapshot;
use crate::model::game_context::GameContext;
use crate::model::game_save::GameSave;
use crate::engine::llm_client::LlmConfig;

#[derive(Debug)]
pub enum EngineCommand {
    /// Player typed something in the chat box
    SubmitPlayerInput {
        text: String,
        context: GameContext,
        llm: LlmConfig,
    },
    /// UI-driven: regenerate the last LLM response without duplicating the user message
    RegenerateLastResponse {
        text: String,
        context: GameContext,
        llm: LlmConfig,
    },

    /// Initialize narrative with opening message (world load)
    InitializeNarrative {
        opening_message: String,
    },

    /// Load / switch LLM backend
    ConnectToLlm {
        llm: LlmConfig,
    },

    /// UI-driven: move an NPC into the party without LLM involvement
    AddNpcToParty {
        id: String,
        name: String,
        role: String,
        details: String,
    },
    /// UI-driven: create a new NPC without LLM involvement
    CreateNpc {
        name: String,
        role: String,
        details: String,
    },
    /// UI-driven: stop the current LLM generation (best effort)
    StopGeneration,

    /// UI-driven: add a party member directly
    AddPartyMember {
        name: String,
        role: String,
        details: String,
        weapons: Vec<String>,
        armor: Vec<String>,
        clothing: Vec<String>,
    },

    /// UI-driven: overwrite a party member's fields
    SetPartyMember {
        id: String,
        name: String,
        role: String,
        details: String,
        weapons: Vec<String>,
        armor: Vec<String>,
        clothing: Vec<String>,
    },
    /// UI-driven: remove a party member
    RemovePartyMember {
        id: String,
    },

    /// UI-driven: lock party member fields to prevent LLM/engine edits
    SetPartyMemberLocks {
        id: String,
        lock_name: bool,
        lock_role: bool,
        lock_details: bool,
        lock_weapons: bool,
        lock_armor: bool,
        lock_clothing: bool,
    },

    /// UI-driven: toggle timing debug output
    SetTimingEnabled {
        enabled: bool,
    },
    /// UI-driven: set NPC recency window for "nearby" classification
    SetNpcRecencyLimit {
        limit: usize,
    },

    SaveGame {
        path: std::path::PathBuf,
        world: crate::ui::app::WorldDefinition,
        player: crate::ui::app::CharacterDefinition,
        party: Vec<crate::ui::app::PartyMember>,
        speaker_colors: crate::ui::app::SpeakerColors,
        save_chat_log: bool,
        character_image_rgba: Option<Vec<u8>>,
        character_image_size: Option<(u32, u32)>,
    },

    LoadGame {
        path: std::path::PathBuf,
    },
}


#[derive(Debug)]
pub enum EngineResponse {
    FullMessageHistory(Vec<Message>),
    AppendMessages(Vec<Message>),
    UiError { message: String },
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
