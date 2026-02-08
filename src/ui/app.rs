use eframe::egui;

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::fs;
use std::fs::File;
use rfd::FileDialog;
use image::ImageReader;


use super::left_panel::draw_left_panel;
use super::center_panel::draw_center_panel;
use super::right_panel::draw_right_panel;

use crate::engine::engine::Engine;
use crate::engine::llm_client::{LlmApiMode, LlmConfig};
use crate::engine::protocol::{EngineCommand, EngineResponse};

use crate::model::game_state::GameStateSnapshot;
use crate::model::message::{Message,};
use crate::model::game_context::GameContext;

/* =========================
   World Definition
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDefinition {
    pub title: String,
    pub world_id: String,
    pub author: String,
    pub description: String,
    pub themes: Vec<String>,
    pub tone: Vec<String>,
    pub narrator_role: String,
    pub style_guidelines: Vec<String>,
    pub opening_message: String,
    pub must_not: Vec<String>,
    pub must_always: Vec<String>,
    #[serde(default)]
    pub loot_rules_mode: String,
    #[serde(default)]
    pub loot_rules_custom: String,
    #[serde(default)]
    pub world_quests_enabled: bool,
    #[serde(default)]
    pub world_quests_mandatory: bool,
    #[serde(default)]
    pub npc_quests_enabled: bool,
    #[serde(default)]
    pub is_rpg_world: bool,
    #[serde(default = "default_exp_multiplier")]
    pub exp_multiplier: f32,
    #[serde(default = "default_repetition_threshold")]
    pub repetition_threshold: u32,
    #[serde(default = "default_repetition_tier_step")]
    pub repetition_tier_step: u32,
    #[serde(default = "default_skill_tier_names")]
    pub skill_tier_names: Vec<String>,
    #[serde(default)]
    pub skill_thresholds: Vec<SkillThreshold>,
    #[serde(default = "default_power_evolution_base")]
    pub power_evolution_base: u32,
    #[serde(default = "default_power_evolution_step")]
    pub power_evolution_step: u32,
    #[serde(default = "default_power_evolution_multiplier_min")]
    pub power_evolution_multiplier_min: f32,
    #[serde(default = "default_power_evolution_multiplier_max")]
    pub power_evolution_multiplier_max: f32,
}

impl Default for WorldDefinition {
    fn default() -> Self {
        Self {
            title: "Untitled World".into(),
            world_id: "world_001".into(),
            author: "Your name".into(),
            description: "Describe the world, its rules, factions, and overall premise.".into(),
            themes: vec!["Power".into(), "Legacy".into()],
            tone: vec!["Serious".into(), "Epic".into()],
            narrator_role: "Act as the narrator and all NPCs. Never control the player.".into(),
            style_guidelines: vec!["Show, don’t tell".into(), "Stay immersive".into()],
            opening_message: "The adventure begins at the edge of the known world…".into(),
            must_not: vec![
                "Do not control the player character".into(),
                "Do not break immersion".into(),
            ],
            must_always: vec![
                "Respect established lore".into(),
                "Use structured events for state changes".into(),
            ],
            loot_rules_mode: "Difficulty based".into(),
            loot_rules_custom: String::new(),
            world_quests_enabled: false,
            world_quests_mandatory: false,
            npc_quests_enabled: false,
            is_rpg_world: false,
            exp_multiplier: 2.0,
            repetition_threshold: 5,
            repetition_tier_step: 5,
            skill_tier_names: default_skill_tier_names(),
            skill_thresholds: Vec::new(),
            power_evolution_base: 10,
            power_evolution_step: 10,
            power_evolution_multiplier_min: 1.1,
            power_evolution_multiplier_max: 3.0,
        }
    }
}

fn default_exp_multiplier() -> f32 {
    2.0
}

fn default_repetition_threshold() -> u32 {
    5
}

fn default_repetition_tier_step() -> u32 {
    5
}

fn default_skill_tier_names() -> Vec<String> {
    vec![
        "Novice".to_string(),
        "Adept".to_string(),
        "Expert".to_string(),
        "Master".to_string(),
        "Grandmaster".to_string(),
    ]
}

fn default_power_evolution_base() -> u32 {
    10
}

fn default_power_evolution_step() -> u32 {
    10
}

fn default_power_evolution_multiplier_min() -> f32 {
    1.1
}

fn default_power_evolution_multiplier_max() -> f32 {
    3.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillThreshold {
    pub skill: String,
    pub base: u32,
    pub step: u32,
    #[serde(default)]
    pub tier_names: Vec<String>,
}

/* =========================
   Character Definition
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterDefinition {
    pub name: String,
    pub class: String,
    pub background: String,
    pub stats: HashMap<String, i32>,
    #[serde(default, deserialize_with = "deserialize_power_entries")]
    pub powers: Vec<PowerEntry>,
    pub features: Vec<String>,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub armor: Vec<String>,
    pub inventory: Vec<String>,
    #[serde(default)]
    pub clothing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PowerEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub locked: bool,
}

fn deserialize_power_entries<'de, D>(deserializer: D) -> Result<Vec<PowerEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PowerEntryOrString {
        Name(String),
        Entry(PowerEntry),
    }

    let items: Option<Vec<PowerEntryOrString>> = Option::deserialize(deserializer)?;
    let Some(items) = items else {
        return Ok(Vec::new());
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            PowerEntryOrString::Name(name) => out.push(PowerEntry {
                name,
                ..PowerEntry::default()
            }),
            PowerEntryOrString::Entry(entry) => out.push(entry),
        }
    }

    Ok(out)
}

impl Default for CharacterDefinition {
    fn default() -> Self {
        let mut stats = HashMap::new();
        for k in ["strength", "constitution", "agility", "intelligence", "luck"] {
            stats.insert(k.into(), 10);
        }

        Self {
            name: "Unnamed Hero".into(),
            class: "Adventurer".into(),
            background: "Describe your character’s origin.".into(),
            stats,
            powers: vec![PowerEntry {
                name: "Basic combat training".into(),
                description: String::new(),
                locked: false,
            }],
            features: vec![],
            weapons: vec![],
            armor: vec![],
            inventory: vec![],
            clothing: vec!["Simple clothing".into()],
        }
    }
}

/* =========================
   Party
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartyMember {
    pub id: Option<String>,
    pub name: String,
    pub role: String,
    pub details: String,
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub armor: Vec<String>,
    #[serde(default)]
    pub clothing: Vec<String>,
    #[serde(default)]
    pub lock_name: bool,
    #[serde(default)]
    pub lock_role: bool,
    #[serde(default)]
    pub lock_details: bool,
    #[serde(default)]
    pub lock_weapons: bool,
    #[serde(default)]
    pub lock_armor: bool,
    #[serde(default)]
    pub lock_clothing: bool,
}


/* =========================
   Speaker Colors
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerColors {
    pub player: SerializableColor,
    pub narrator: SerializableColor,
    pub npc: SerializableColor,
    pub party: SerializableColor,
    pub system: SerializableColor,
}


impl Default for SpeakerColors {
    fn default() -> Self {
        Self {
            player: SerializableColor { r: 120, g: 200, b: 255, a: 255 },
            narrator: SerializableColor { r: 220, g: 220, b: 220, a: 255 },
            npc: SerializableColor { r: 255, g: 180, b: 120, a: 255 },
            party: SerializableColor { r: 160, g: 255, b: 160, a: 255 },
            system: SerializableColor { r: 255, g: 120, b: 120, a: 255 },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Party,
    Npcs,
    Quests,
    Factions,
    Slaves,
    Property,
    BondedServants,
    Concubines,
    HaremMembers,
    Prisoners,
    NpcsOnMission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Player,
    World,
}

/* =========================
   UI State
   ========================= */

pub struct UiState {
    pub input_text: String,
    pub rendered_messages: Vec<Message>,
    pub snapshot: Option<GameStateSnapshot>,

    pub ui_scale: f32,
    pub text_scale: f32,
    pub chat_text_scale: f32,
    pub should_auto_scroll: bool,
    pub chat_user_scrolled_up: bool,

    pub world: WorldDefinition,
    pub character: CharacterDefinition,
    pub party: Vec<PartyMember>,

    pub speaker_colors: SpeakerColors,

    pub show_settings: bool,
    pub show_options: bool,

    pub llm_connected: bool,
    pub llm_status: String,
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_api_key: String,
    pub llm_api_mode: UiLlmApiMode,
    pub ui_error: Option<String>,
    pub chat_log_limit: Option<usize>,
    pub save_full_chat_log: bool,
    pub prompt_history_limit: Option<usize>,
    pub timing_enabled: bool,
    pub npc_recent_messages_limit: usize,

    pub left_tab: LeftTab,
    pub right_tab: RightTab,      // NEW: track which right panel tab is active
    pub player_locked: bool,
    pub world_locked: bool,
    pub new_stat_name: String,    // NEW: for adding new stats
    pub new_stat_value: i32,      // NEW: for adding new stats
    pub right_panel_width: f32,

    pub new_npc_name: String,
    pub new_npc_role: String,
    pub new_npc_notes: String,

    pub is_generating: bool,

    pub character_image: Option<egui::TextureHandle>,
    pub character_image_rgba: Option<Vec<u8>>,
    pub character_image_size: Option<(u32, u32)>,

    pub optional_tabs: OptionalTabs,
    pub base_tabs: BaseTabs,
    pub base_text_sizes: Option<HashMap<egui::TextStyle, f32>>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            input_text: String::new(),
            rendered_messages: Vec::new(),
            snapshot: None,

            ui_scale: 1.0,
            text_scale: 1.0,
            chat_text_scale: 1.0,
            should_auto_scroll: true,
            chat_user_scrolled_up: false,

            world: WorldDefinition::default(),
            character: CharacterDefinition::default(),
            party: Vec::new(),

            speaker_colors: SpeakerColors::default(),

            show_settings: false,
            show_options: false,

            llm_connected: false,
            llm_status: "Not connected".into(),
            llm_base_url: "http://localhost:1234/v1".into(),
            llm_model: "local-model".into(),
            llm_api_key: String::new(),
            llm_api_mode: UiLlmApiMode::OpenAiChat,
            ui_error: None,
            chat_log_limit: None,
            save_full_chat_log: false,
            prompt_history_limit: Some(50),
            timing_enabled: true,
            npc_recent_messages_limit: 10,

            left_tab: LeftTab::Party,
            right_tab: RightTab::Player, // NEW: default tab
            player_locked: false,
            world_locked: false,
            new_stat_name: String::new(),
            new_stat_value: 10,
            right_panel_width: 340.0,

            new_npc_name: String::new(),
            new_npc_role: String::new(),
            new_npc_notes: String::new(),

            is_generating: false,

            character_image: None,
            character_image_rgba: None,
            character_image_size: None,

            optional_tabs: OptionalTabs::default(),
            base_tabs: BaseTabs::default(),
            base_text_sizes: None,
        }
    }
}

impl UiState {
    pub fn llm_config(&self) -> LlmConfig {
        let base_url = if self.llm_base_url.trim().is_empty() {
            match self.llm_api_mode {
                UiLlmApiMode::OpenAiChat => "http://localhost:1234/v1".to_string(),
                UiLlmApiMode::KoboldCpp => "http://localhost:5001".to_string(),
            }
        } else {
            self.llm_base_url.trim().to_string()
        };

        let model = if self.llm_model.trim().is_empty() {
            "local-model".to_string()
        } else {
            self.llm_model.trim().to_string()
        };

        let api_key = self.llm_api_key.trim();
        LlmConfig {
            base_url,
            model,
            api_key: if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            },
            api_mode: match self.llm_api_mode {
                UiLlmApiMode::OpenAiChat => LlmApiMode::OpenAiChat,
                UiLlmApiMode::KoboldCpp => LlmApiMode::KoboldCpp,
            },
        }
    }

    pub fn apply_chat_log_limit(&mut self) {
        if let Some(limit) = self.chat_log_limit {
            if self.rendered_messages.len() > limit {
                let excess = self.rendered_messages.len() - limit;
                self.rendered_messages.drain(0..excess);
            }
        }
    }

    pub fn trim_messages_after_last_user(&mut self) -> Option<String> {
        let mut idx = self.rendered_messages.len();
        while idx > 0 {
            if matches!(self.rendered_messages[idx - 1], Message::User(_)) {
                break;
            }
            idx -= 1;
        }

        if idx == 0 {
            return None;
        }

        let last_user = match &self.rendered_messages[idx - 1] {
            Message::User(text) => text.clone(),
            _ => return None,
        };

        self.rendered_messages.truncate(idx);
        Some(last_user)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OptionalTabState {
    pub enabled: bool,
    pub unlocked: bool,
}

impl Default for OptionalTabState {
    fn default() -> Self {
        Self {
            enabled: true,
            unlocked: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionalTabs {
    pub slaves: OptionalTabState,
    pub property: OptionalTabState,
    pub bonded_servants: OptionalTabState,
    pub concubines: OptionalTabState,
    pub harem_members: OptionalTabState,
    pub prisoners: OptionalTabState,
    pub npcs_on_mission: OptionalTabState,
    pub bonded_servants_label: String,
}

impl Default for OptionalTabs {
    fn default() -> Self {
        Self {
            slaves: OptionalTabState::default(),
            property: OptionalTabState::default(),
            bonded_servants: OptionalTabState::default(),
            concubines: OptionalTabState::default(),
            harem_members: OptionalTabState::default(),
            prisoners: OptionalTabState::default(),
            npcs_on_mission: OptionalTabState::default(),
            bonded_servants_label: "Bonded".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BaseTabs {
    pub party: bool,
    pub npcs: bool,
    pub quests: bool,
    pub factions: bool,
}

impl Default for BaseTabs {
    fn default() -> Self {
        Self {
            party: true,
            npcs: true,
            quests: true,
            factions: true,
        }
    }
}

impl UiState {
    pub fn default_save_dir() -> PathBuf {
        let mut path = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("UPF Saves");
        fs::create_dir_all(&path).ok();
        path
    }

    pub fn load_character_image_from_dialog(&mut self, ctx: &egui::Context) {
        let path = FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg"])
            .pick_file();
        let Some(path) = path else {
            return;
        };
        if let Ok((width, height, rgba)) = load_image_rgba(&path) {
            self.set_character_image_from_rgba(ctx, width, height, rgba);
        }
    }

    pub fn save_character(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("Character Image", &["png"])
            .set_file_name("character.png")
            .set_directory(Self::default_save_dir())
            .save_file()
        else {
            return;
        };

        let (width, height) = match self.character_image_size {
            Some(size) => size,
            None => return,
        };
        let Some(rgba) = self.character_image_rgba.as_ref() else {
            return;
        };

        let Some(path) = force_png_extension(path) else {
            return;
        };

        if let Ok(json) = serde_json::to_string_pretty(&self.character) {
            let _ = write_png_with_character_json(&path, width, height, rgba, &json);
        }
    }

    pub fn load_character_from_dialog(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<CharacterDefinition> {
        let path = FileDialog::new()
            .add_filter("Character Image", &["png"])
            .add_filter("Character Json", &["json"])
            .set_directory(Self::default_save_dir())
            .pick_file()?;

        let mut character = match path.extension().and_then(|s| s.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("png") => {
                let json = extract_character_json_from_png(&path)?;
                if let Ok((width, height, rgba)) = load_image_rgba(&path) {
                    self.set_character_image_from_rgba(ctx, width, height, rgba);
                }
                serde_json::from_str::<CharacterDefinition>(&json).ok()
            }
            _ => {
                let data = fs::read_to_string(path).ok()?;
                serde_json::from_str::<CharacterDefinition>(&data).ok()
            }
        }?;

        migrate_character_clothing(&mut character);
        Some(character)
    }

    pub fn save_world(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("World", &["json"])
            .set_file_name("world.json")
            .set_directory(Self::default_save_dir())
            .save_file()
        else {
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(&self.world) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_world_from_dialog() -> Option<WorldDefinition> {
        let path = FileDialog::new()
            .add_filter("World", &["json"])
            .set_directory(Self::default_save_dir())
            .pick_file()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str::<WorldDefinition>(&data).ok()
    }

    fn set_character_image_from_rgba(
        &mut self,
        ctx: &egui::Context,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &rgba,
        );
        let texture =
            ctx.load_texture("character_portrait", image, egui::TextureOptions::LINEAR);
        self.character_image = Some(texture);
        self.character_image_rgba = Some(rgba);
        self.character_image_size = Some((width, height));
    }

    pub fn apply_party_updates_from_report(
        &mut self,
        report: &crate::model::event_result::NarrativeApplyReport,
    ) {
        for app in &report.applications {
            use crate::model::narrative_event::NarrativeEvent;
            match &app.event {
                NarrativeEvent::AddPartyMember { id, name, role } => {
                    self.upsert_party_member(
                        Some(id),
                        Some(name),
                        Some(role),
                        None,
                        None,
                        None,
                        None,
                    );
                }
                NarrativeEvent::NpcJoinParty { id, name, role, details, clothing, weapons, armor } => {
                    self.upsert_party_member(
                        id.as_deref(),
                        name.as_deref(),
                        role.as_deref(),
                        details.as_deref(),
                        clothing.as_deref(),
                        weapons.as_deref(),
                        armor.as_deref(),
                    );
                }
                NarrativeEvent::NpcLeaveParty { id } => {
                    if let Some(idx) = self.party.iter().position(|m| m.id.as_deref() == Some(id)) {
                        self.party.remove(idx);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn sync_party_from_snapshot(
        &mut self,
        snapshot: &crate::model::game_state::GameStateSnapshot,
    ) {
        self.party.clear();
        for member in &snapshot.party {
            self.party.push(PartyMember {
                id: Some(member.id.clone()),
                name: member.name.clone(),
                role: member.role.clone(),
                details: member.details.clone(),
                weapons: member.weapons.clone(),
                armor: member.armor.clone(),
                clothing: member.clothing.clone(),
                lock_name: member.lock_name,
                lock_role: member.lock_role,
                lock_details: member.lock_details,
                lock_weapons: member.lock_weapons,
                lock_armor: member.lock_armor,
                lock_clothing: member.lock_clothing,
            });
        }
    }

    pub fn sync_player_from_snapshot(
        &mut self,
        snapshot: &crate::model::game_state::GameStateSnapshot,
    ) {
        if let Some(tab) = self.update_optional_tabs_from_snapshot(snapshot) {
            if self.is_left_tab_visible(tab) {
                self.left_tab = tab;
            }
        }

        for item in &snapshot.player.weapons {
            if !contains_case_insensitive(&self.character.weapons, item) {
                self.character.weapons.push(item.clone());
            }
        }

        for item in &snapshot.player.armor {
            if !contains_case_insensitive(&self.character.armor, item) {
                self.character.armor.push(item.clone());
            }
        }

        for item in &snapshot.player.clothing {
            if !contains_case_insensitive(&self.character.clothing, item) {
                self.character.clothing.push(item.clone());
            }
        }

        for stack in &snapshot.inventory {
            let label = inventory_label(&stack.id, stack.quantity);
            remove_inventory_entry(&mut self.character.inventory, &stack.id);
            self.character.inventory.push(label);
        }

        if !snapshot.stats.is_empty() {
            for stat in &snapshot.stats {
                self.character.stats.insert(stat.id.clone(), stat.value);
            }
        }
    }

    pub fn sync_party_from_messages(&mut self) {
        // Party details should only be updated via structured events/snapshots,
        // not inferred from narration or dialogue.
    }

    fn upsert_party_member(
        &mut self,
        id: Option<&str>,
        name: Option<&str>,
        role: Option<&str>,
        details: Option<&str>,
        clothing: Option<&[String]>,
        weapons: Option<&[String]>,
        armor: Option<&[String]>,
    ) {
        let mut index = None;
        if let Some(id) = id {
            index = self.party.iter().position(|m| m.id.as_deref() == Some(id));
        }
        if index.is_none() {
            if let Some(name) = name {
                let needle = name.trim();
                if !needle.is_empty() {
                    index = self
                        .party
                        .iter()
                        .position(|m| m.name.eq_ignore_ascii_case(needle));
                }
            }
        }

        let name_value = name.unwrap_or("Unknown").trim();
        let role_value = role.unwrap_or("Unknown").trim();

        if let Some(i) = index {
            let member = &mut self.party[i];
            if member.id.is_none() {
                member.id = id.map(|v| v.to_string());
            }

            if !member.lock_name
                && !name_value.is_empty()
                && (member.name.trim().is_empty() || member.name.eq_ignore_ascii_case("unknown"))
            {
                member.name = name_value.to_string();
            }

            if !member.lock_role
                && !role_value.is_empty()
                && (member.role.trim().is_empty() || member.role.eq_ignore_ascii_case("unknown"))
            {
                member.role = role_value.to_string();
            }

            if !member.lock_details {
                if let Some(details) = details {
                    let trimmed = details.trim();
                    if !trimmed.is_empty() {
                        if member.details.trim().is_empty() {
                            member.details = trimmed.to_string();
                        } else if !member.details.contains(trimmed) {
                            member.details =
                                format!("{}\n{}", member.details.trim_end(), trimmed);
                        }
                    }
                }
            }
            if !member.lock_clothing {
                if let Some(clothing) = clothing {
                    if !clothing.is_empty() {
                        member.clothing = clothing.to_vec();
                    }
                }
            }
            if !member.lock_weapons {
                if let Some(weapons) = weapons {
                    if !weapons.is_empty() {
                        member.weapons = weapons.to_vec();
                    }
                }
            }
            if !member.lock_armor {
                if let Some(armor) = armor {
                    if !armor.is_empty() {
                        member.armor = armor.to_vec();
                    }
                }
            }
        } else {
            if id.is_none() && name.map(|v| v.trim().is_empty()).unwrap_or(true) {
                return;
            }
            self.party.push(PartyMember {
                id: id.map(|v| v.to_string()),
                name: name_value.to_string(),
                role: role_value.to_string(),
                details: details.unwrap_or("").trim().to_string(),
                weapons: weapons.unwrap_or(&[]).to_vec(),
                armor: armor.unwrap_or(&[]).to_vec(),
                clothing: clothing.unwrap_or(&[]).to_vec(),
                lock_name: false,
                lock_role: false,
                lock_details: false,
                lock_weapons: false,
                lock_armor: false,
                lock_clothing: false,
            });
        }
    }

    pub fn is_left_tab_visible(&self, tab: LeftTab) -> bool {
        match tab {
            LeftTab::Party => self.base_tabs.party,
            LeftTab::Npcs => self.base_tabs.npcs,
            LeftTab::Quests => self.base_tabs.quests,
            LeftTab::Factions => self.base_tabs.factions,
            LeftTab::Slaves => self.optional_tabs.slaves.unlocked && self.optional_tabs.slaves.enabled,
            LeftTab::Property => self.optional_tabs.property.unlocked && self.optional_tabs.property.enabled,
            LeftTab::BondedServants => {
                self.optional_tabs.bonded_servants.unlocked
                    && self.optional_tabs.bonded_servants.enabled
            }
            LeftTab::Concubines => {
                self.optional_tabs.concubines.unlocked && self.optional_tabs.concubines.enabled
            }
            LeftTab::HaremMembers => {
                self.optional_tabs.harem_members.unlocked && self.optional_tabs.harem_members.enabled
            }
            LeftTab::Prisoners => {
                self.optional_tabs.prisoners.unlocked && self.optional_tabs.prisoners.enabled
            }
            LeftTab::NpcsOnMission => {
                self.optional_tabs.npcs_on_mission.unlocked
                    && self.optional_tabs.npcs_on_mission.enabled
            }
        }
    }

    pub fn ensure_left_tab_visible(&mut self) {
        if !self.is_left_tab_visible(self.left_tab) {
            self.left_tab = first_visible_left_tab(self);
        }
    }

    fn update_optional_tabs_from_snapshot(
        &mut self,
        snapshot: &crate::model::game_state::GameStateSnapshot,
    ) -> Option<LeftTab> {
        let mut opened: Option<LeftTab> = None;
        for flag in &snapshot.flags {
            let flag = flag.trim().to_lowercase();
            if flag.is_empty() {
                continue;
            }

            if matches_flag(&flag, &["unlock:slaves", "slaves", "slave", "owned_slaves", "owns_slaves"])
                && unlock_if_needed(&mut self.optional_tabs.slaves, LeftTab::Slaves, &mut opened)
            {
                continue;
            }

            if matches_flag(&flag, &["unlock:property", "property", "owned_property", "owns_property"])
                && unlock_if_needed(&mut self.optional_tabs.property, LeftTab::Property, &mut opened)
            {
                continue;
            }

            if matches_flag(
                &flag,
                &[
                    "unlock:bonded_servants",
                    "bonded_servants",
                    "bonded-servants",
                    "bonded servants",
                    "bondservants",
                    "hirð",
                ],
            ) && unlock_if_needed(
                &mut self.optional_tabs.bonded_servants,
                LeftTab::BondedServants,
                &mut opened,
            ) {
                continue;
            }

            if matches_flag(&flag, &["unlock:concubines", "concubines", "concubine"])
                && unlock_if_needed(&mut self.optional_tabs.concubines, LeftTab::Concubines, &mut opened)
            {
                continue;
            }

            if matches_flag(&flag, &["unlock:harem_members", "harem_members", "harem", "harem members"])
                && unlock_if_needed(&mut self.optional_tabs.harem_members, LeftTab::HaremMembers, &mut opened)
            {
                continue;
            }

            if matches_flag(&flag, &["unlock:prisoners", "prisoners", "prisoner", "captives"])
                && unlock_if_needed(&mut self.optional_tabs.prisoners, LeftTab::Prisoners, &mut opened)
            {
                continue;
            }

            if matches_flag(
                &flag,
                &[
                    "unlock:npcs_on_mission",
                    "npcs_on_mission",
                    "npc_missions",
                    "npc missions",
                    "missions",
                ],
            ) && unlock_if_needed(
                &mut self.optional_tabs.npcs_on_mission,
                LeftTab::NpcsOnMission,
                &mut opened,
            ) {
                continue;
            }
        }
        opened
    }
}

fn matches_flag(flag: &str, aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| flag == *alias)
}

fn unlock_if_needed(
    tab: &mut OptionalTabState,
    left_tab: LeftTab,
    opened: &mut Option<LeftTab>,
) -> bool {
    if !tab.unlocked {
        tab.unlocked = true;
        if tab.enabled && opened.is_none() {
            *opened = Some(left_tab);
        }
        return true;
    }
    false
}

fn migrate_character_clothing(character: &mut CharacterDefinition) {
    if !character.clothing.is_empty() {
        return;
    }

    let mut remaining = Vec::new();
    for item in character.inventory.drain(..) {
        if looks_like_clothing(&item) {
            character.clothing.push(item);
        } else {
            remaining.push(item);
        }
    }
    character.inventory = remaining;
}

fn looks_like_clothing(item: &str) -> bool {
    let item = item.to_lowercase();
    let keywords = [
        "clothing",
        "underwear",
        "bra",
        "bras",
        "lingerie",
        "panties",
        "briefs",
        "boxers",
        "boxer",
        "thong",
        "g-string",
        "gstring",
        "bikini",
        "swimwear",
        "swimsuit",
        "swim suit",
        "one-piece",
        "one piece",
        "two-piece",
        "two piece",
        "trunks",
        "boardshorts",
        "board shorts",
        "rashguard",
        "rash guard",
        "socks",
        "sock",
        "stockings",
        "tights",
        "leggings",
        "shirt",
        "blouse",
        "top",
        "tee",
        "t-shirt",
        "tshirt",
        "shirts",
        "blouses",
        "tops",
        "tees",
        "t-shirts",
        "tshirts",
        "sweater",
        "jumper",
        "hoodie",
        "coat",
        "overcoat",
        "parka",
        "scarf",
        "shawl",
        "sweaters",
        "jumpers",
        "hoodies",
        "coats",
        "overcoats",
        "parkas",
        "scarves",
        "shawls",
        "pants",
        "jeans",
        "trousers",
        "shorts",
        "skirt",
        "kilt",
        "skirts",
        "kilts",
        "dress",
        "gown",
        "robe",
        "cloak",
        "hood",
        "jacket",
        "tunic",
        "vest",
        "dresses",
        "gowns",
        "robes",
        "cloaks",
        "hoods",
        "jackets",
        "tunics",
        "vests",
        "armor",
        "armour",
        "armors",
        "armours",
        "pauldron",
        "pauldrons",
        "sabatons",
        "sabaton",
        "greaves",
        "cuirass",
        "breastplate",
        "gauntlet",
        "gauntlets",
        "vambrace",
        "vambraces",
        "bracer",
        "bracers",
        "pauldron",
        "pauldrons",
        "helm",
        "helmet",
        "helms",
        "helmets",
        "shield",
        "mail",
        "chainmail",
        "chain mail",
        "scale mail",
        "scalemail",
        "leather armor",
        "leather armour",
        "plate armor",
        "plate armour",
        "hauberk",
        "coif",
        "boots",
        "gloves",
        "gauntlets",
        "sabatons",
        "cap",
        "hat",
        "belt",
        "boot",
        "glove",
        "caps",
        "hats",
        "belts",
        "trainer",
        "trainers",
        "plimsoll",
        "plimsolls",
        "glasses",
        "goggles",
        "sunglasses",
        "eyeglasses",
        "spectacles",
        "jewelry",
        "jewellery",
        "jewelries",
        "jewelleries",
        "ring",
        "rings",
        "amulet",
        "amulets",
        "necklace",
        "necklaces",
        "earring",
        "earrings",
        "bracelet",
        "bracelets",
        "brooch",
        "brooches",
        "tiara",
        "crown",
        "circlet",
        "tiaras",
        "crowns",
        "circlets",
        "hairpin",
        "hairpins",
        "hair clip",
        "hair clips",
        "barrette",
        "barrettes",
        "ribbon",
        "ribbons",
        "headband",
        "headbands",
        "scrunchie",
        "scrunchies",
    ];
    keywords.iter().any(|k| item.contains(k))
}

fn contains_case_insensitive(list: &[String], value: &str) -> bool {
    list.iter().any(|v| v.eq_ignore_ascii_case(value))
}

fn inventory_label(id: &str, quantity: u32) -> String {
    if quantity <= 1 {
        id.to_string()
    } else {
        format!("{} x{}", id, quantity)
    }
}

fn remove_inventory_entry(list: &mut Vec<String>, id: &str) {
    let needle = id.to_lowercase();
    let prefix = format!("{} x", needle);
    list.retain(|item| {
        let lower = item.to_lowercase();
        !(lower == needle || lower.starts_with(&prefix))
    });
}
/* =========================
   Config
   ========================= */

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub ui_scale: f32,
    #[serde(default)]
    pub text_scale: f32,
    #[serde(default)]
    pub chat_text_scale: f32,
    pub speaker_colors: SpeakerColors,
    #[serde(default)]
    pub llm_base_url: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default)]
    pub llm_api_key: String,
    #[serde(default)]
    pub llm_api_mode: UiLlmApiMode,
    #[serde(default)]
    pub chat_log_limit: Option<usize>,
    #[serde(default)]
    pub save_full_chat_log: bool,
    #[serde(default)]
    pub prompt_history_limit: Option<usize>,
    #[serde(default = "default_timing_enabled")]
    pub timing_enabled: bool,
    #[serde(default = "default_npc_recent_messages_limit")]
    pub npc_recent_messages_limit: usize,
}

fn default_npc_recent_messages_limit() -> usize {
    10
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            text_scale: 1.0,
            chat_text_scale: 1.0,
            speaker_colors: SpeakerColors::default(),
            llm_base_url: "http://localhost:1234/v1".into(),
            llm_model: "local-model".into(),
            llm_api_key: String::new(),
            llm_api_mode: UiLlmApiMode::OpenAiChat,
            chat_log_limit: None,
            save_full_chat_log: false,
            prompt_history_limit: Some(50),
            timing_enabled: default_timing_enabled(),
        }
    }
}

fn default_timing_enabled() -> bool {
    true
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SerializableColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLlmApiMode {
    OpenAiChat,
    KoboldCpp,
}

impl Default for UiLlmApiMode {
    fn default() -> Self {
        Self::OpenAiChat
    }
}

impl From<SerializableColor> for egui::Color32 {
    fn from(c: SerializableColor) -> Self {
        egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
    }
}

impl From<egui::Color32> for SerializableColor {
    fn from(c: egui::Color32) -> Self {
        let [r, g, b, a] = c.to_array();
        Self { r, g, b, a }
    }
}

/* =========================
   App
   ========================= */

pub struct MyApp {
    pub ui: UiState,
    cmd_tx: mpsc::Sender<EngineCommand>,
    resp_rx: mpsc::Receiver<EngineResponse>,
}

impl MyApp {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();

        std::thread::spawn(move || {
            let mut engine = Engine::new(cmd_rx, resp_tx);
            engine.run();
        });

        let mut ui = UiState::default();
        load_config(&mut ui);
        let _ = cmd_tx.send(EngineCommand::SetTimingEnabled {
            enabled: ui.timing_enabled,
        });
        let _ = cmd_tx.send(EngineCommand::SetNpcRecencyLimit {
            limit: ui.npc_recent_messages_limit.max(1),
        });

        Self { ui, cmd_tx, resp_rx }
    }

    pub fn send_command(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn build_game_context(&self) -> GameContext {
        let history = match self.ui.prompt_history_limit {
            Some(0) => Vec::new(),
            Some(limit) => {
                if self.ui.rendered_messages.len() > limit {
                    self.ui.rendered_messages[self.ui.rendered_messages.len() - limit..].to_vec()
                } else {
                    self.ui.rendered_messages.clone()
                }
            }
            None => self.ui.rendered_messages.clone(),
        };
        GameContext {
            world: self.ui.world.clone(),
            player: self.ui.character.clone(),
            party: self.ui.party.clone(),
            history,
            snapshot: self.ui.snapshot.clone(),
        }
    }
}

/* =========================
   egui App
   ========================= */

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        sanitize_ui_scales(&mut self.ui);
        ctx.set_pixels_per_point(self.ui.ui_scale);
        apply_text_scale(ctx, &mut self.ui);

        let mut received_response = false;
        while let Ok(resp) = self.resp_rx.try_recv() {
            received_response = true;
            match resp {
                EngineResponse::FullMessageHistory(msgs) => {
                    self.ui.rendered_messages = msgs;
                    self.ui.should_auto_scroll = true;
                    self.ui.apply_chat_log_limit();
                    self.ui.sync_party_from_messages();
                    self.ui.ensure_left_tab_visible();
                    self.ui.is_generating = false;
                }
                EngineResponse::AppendMessages(msgs) => {
                    if !msgs.is_empty() {
                        self.ui.rendered_messages.extend(msgs);
                        self.ui.should_auto_scroll = true;
                        self.ui.apply_chat_log_limit();
                        self.ui.sync_party_from_messages();
                        self.ui.ensure_left_tab_visible();
                    }
                    self.ui.is_generating = false;
                }
                EngineResponse::UiError { message } => {
                    self.ui.ui_error = Some(message);
                    self.ui.is_generating = false;
                }
                EngineResponse::NarrativeApplied { report, snapshot } => {
                    self.ui.snapshot = Some(snapshot.clone());
                    self.ui.apply_party_updates_from_report(&report);
                    self.ui.sync_party_from_snapshot(&snapshot);
                    self.ui.sync_player_from_snapshot(&snapshot);
                    self.ui.sync_party_from_messages();
                    self.ui.ensure_left_tab_visible();
                    for a in report.applications {
                        let t = format!("{:?}", a.outcome);
                        self.ui.rendered_messages.push(Message::System(t));
                    }
                    self.ui.apply_chat_log_limit();
                    self.ui.is_generating = false;
                }
                EngineResponse::GameLoaded { save, snapshot } => {
                    self.ui.world = save.world;
                    self.ui.character = save.player;
                    self.ui.party = Vec::new();
                    self.ui.rendered_messages = save.messages;
                    self.ui.speaker_colors = save.speaker_colors;
                    self.ui.character_image = None;
                    self.ui.character_image_rgba = None;
                    self.ui.character_image_size = None;
                    if let (Some(rgba), Some((width, height))) = (
                        save.character_image_rgba.clone(),
                        save.character_image_size,
                    ) {
                        self.ui.set_character_image_from_rgba(ctx, width, height, rgba);
                    }
                    self.ui.snapshot = Some(snapshot.clone());
                    self.ui.apply_chat_log_limit();
                    self.ui.sync_party_from_snapshot(&snapshot);
                    self.ui.sync_player_from_snapshot(&snapshot);
                    self.ui.ensure_left_tab_visible();
                }
                EngineResponse::LlmConnectionResult { success, message } => {
                    self.ui.llm_connected = success;
                    self.ui.llm_status = message;
                }
            }
        }
        if received_response {
            // Ensure async engine responses are rendered immediately.
            ctx.request_repaint();
        }

        draw_left_panel(ctx, &mut self.ui, &self.cmd_tx);
        draw_right_panel(ctx, &mut self.ui, &self.cmd_tx);
        draw_center_panel(ctx, self);

        draw_settings_window(ctx, &mut self.ui, &self.cmd_tx);
        draw_options_window(ctx, &mut self.ui, &self.cmd_tx);
    }
}

/* =========================
   Settings / Options Windows
   ========================= */

fn draw_settings_window(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    cmd_tx: &mpsc::Sender<EngineCommand>,
) {
    let mut open = ui_state.show_settings;

    egui::Window::new("⚙ Settings")
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("UI Scale");
            let ui_scale_changed = ui
                .add(egui::Slider::new(&mut ui_state.ui_scale, 0.75..=1.5))
                .changed();

            ui.label("Text Size");
            let text_scale_changed = ui
                .add(egui::Slider::new(&mut ui_state.text_scale, 0.75..=1.5))
                .changed();

            ui.label("Chat Text Size");
            let chat_text_scale_changed = ui
                .add(egui::Slider::new(&mut ui_state.chat_text_scale, 0.75..=2.0))
                .changed();

            ui.separator();
            ui.label("Chat Log Limit");
            let mut chat_limit = ui_state.chat_log_limit;
            let mut chat_limit_changed = false;
            egui::ComboBox::from_id_salt("chat_log_limit")
                .selected_text(match chat_limit {
                    None => "All".to_string(),
                    Some(value) => value.to_string(),
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(chat_limit.is_none(), "All").clicked() {
                        chat_limit = None;
                        chat_limit_changed = true;
                    }
                    for value in [25_usize, 50, 100, 150, 200] {
                        if ui
                            .selectable_label(chat_limit == Some(value), value.to_string())
                            .clicked()
                        {
                            chat_limit = Some(value);
                            chat_limit_changed = true;
                        }
                    }
                });

            let save_chat_log_changed = ui
                .checkbox(
                    &mut ui_state.save_full_chat_log,
                    "Save full chat log with game save",
                )
                .changed();

            ui.separator();
            ui.label("Prompt History (messages)");
            let mut prompt_history = ui_state.prompt_history_limit;
            let mut prompt_history_changed = false;
            egui::ComboBox::from_id_salt("prompt_history_limit")
                .selected_text(match prompt_history {
                    None => "All".to_string(),
                    Some(0) => "Off".to_string(),
                    Some(value) => value.to_string(),
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(prompt_history == Some(0), "Off").clicked() {
                        prompt_history = Some(0);
                        prompt_history_changed = true;
                    }
                    for value in [10_usize, 25, 50, 100, 200, 400, 800] {
                        if ui
                            .selectable_label(prompt_history == Some(value), value.to_string())
                            .clicked()
                        {
                            prompt_history = Some(value);
                            prompt_history_changed = true;
                        }
                    }
                    if ui.selectable_label(prompt_history.is_none(), "All").clicked() {
                        prompt_history = None;
                        prompt_history_changed = true;
                    }
                });

            let timing_changed = ui
                .checkbox(&mut ui_state.timing_enabled, "Show timing debug lines")
                .changed();

            ui.heading("Speaker Colors");

            color_picker(ui, "Player", &mut ui_state.speaker_colors.player);
            color_picker(ui, "Narrator", &mut ui_state.speaker_colors.narrator);
            color_picker(ui, "NPC", &mut ui_state.speaker_colors.npc);
            color_picker(ui, "Party", &mut ui_state.speaker_colors.party);
            color_picker(ui, "System", &mut ui_state.speaker_colors.system);

            if ui_scale_changed
                || text_scale_changed
                || chat_text_scale_changed
                || chat_limit_changed
                || save_chat_log_changed
                || prompt_history_changed
                || timing_changed
                || ui.button("Save").clicked()
            {
                if chat_limit_changed {
                    ui_state.chat_log_limit = chat_limit;
                    ui_state.apply_chat_log_limit();
                }
                if prompt_history_changed {
                    ui_state.prompt_history_limit = prompt_history;
                }
                if timing_changed {
                    let _ = cmd_tx.send(EngineCommand::SetTimingEnabled {
                        enabled: ui_state.timing_enabled,
                    });
                }
                save_config(ui_state);
            }
        });

    ui_state.show_settings = open;
}

fn draw_options_window(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    cmd_tx: &mpsc::Sender<EngineCommand>,
) {
    let mut open = ui_state.show_options;

    egui::Window::new("🛠 Options")
        .open(&mut open)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.label("LLM Base URL");
                    let mut llm_changed = ui
                        .add(
                            egui::TextEdit::singleline(&mut ui_state.llm_base_url)
                                .hint_text("http://localhost:1234/v1"),
                        )
                        .changed();

                    ui.label("LLM Model");
                    llm_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut ui_state.llm_model)
                                .hint_text("local-model"),
                        )
                        .changed();

                    ui.label("LLM API Key (optional)");
                    llm_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut ui_state.llm_api_key)
                                .password(true)
                                .hint_text(""),
                        )
                        .changed();

                    ui.add_space(6.0);
                    ui.label("API Mode");
                    llm_changed |= ui
                        .radio_value(
                            &mut ui_state.llm_api_mode,
                            UiLlmApiMode::OpenAiChat,
                            "OpenAI-compatible",
                        )
                        .changed();
                    llm_changed |= ui
                        .radio_value(
                            &mut ui_state.llm_api_mode,
                            UiLlmApiMode::KoboldCpp,
                            "KoboldCpp native",
                        )
                        .changed();

                    ui.add_space(6.0);
                    ui.label("KoboldCpp Presets");
                    ui.horizontal(|ui| {
                        if ui.button("Use OpenAI-compatible").clicked() {
                            ui_state.llm_api_mode = UiLlmApiMode::OpenAiChat;
                            ui_state.llm_base_url = "http://localhost:5001/v1".to_string();
                            llm_changed = true;
                        }
                        if ui.button("Use KoboldCpp native").clicked() {
                            ui_state.llm_api_mode = UiLlmApiMode::KoboldCpp;
                            ui_state.llm_base_url = "http://localhost:5001".to_string();
                            llm_changed = true;
                        }
                    });

                    ui.add_space(6.0);
                    ui.label("KoboldCpp Connection Protocols");
                    ui.label("OpenAI-compatible: POST /v1/chat/completions");
                    ui.label("KoboldCpp native: POST /api/v1/generate");
                    ui.label("KoboldCpp abort: POST /api/extra/abort");

                    if llm_changed {
                        save_config(ui_state);
                    }

                    if ui.button("🔌 Connect to LLM").clicked() {
                        let _ = cmd_tx.send(EngineCommand::ConnectToLlm {
                            llm: ui_state.llm_config(),
                        });
                    }

                    ui.add_space(6.0);
                    ui.separator();
                    ui.heading("NPC Proximity");
                    ui.label("Hide NPCs if they haven't spoken in the last N messages.");
                    let mut npc_limit = ui_state.npc_recent_messages_limit as i32;
                    if ui
                        .add(egui::DragValue::new(&mut npc_limit).clamp_range(1..=200))
                        .changed()
                    {
                        ui_state.npc_recent_messages_limit = npc_limit.max(1) as usize;
                        let _ = cmd_tx.send(EngineCommand::SetNpcRecencyLimit {
                            limit: ui_state.npc_recent_messages_limit,
                        });
                        save_config(ui_state);
                    }

                    let status_color = if ui_state.llm_connected {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };

                    ui.label(egui::RichText::new(&ui_state.llm_status).color(status_color));
                    ui.separator();
                    ui.heading("Left Tabs");
                    let mut tabs_changed = false;
                    tabs_changed |= ui.checkbox(&mut ui_state.base_tabs.party, "Party").changed();
                    tabs_changed |= ui.checkbox(&mut ui_state.base_tabs.npcs, "NPCs").changed();
                    tabs_changed |= ui.checkbox(&mut ui_state.base_tabs.quests, "Quests").changed();
                    tabs_changed |= ui.checkbox(&mut ui_state.base_tabs.factions, "Factions").changed();
                    if tabs_changed {
                        ui_state.ensure_left_tab_visible();
                    }

                    ui.separator();
                    ui.heading("Optional Tabs");
                    ui.label("Tabs unlock when the engine sets a flag like: unlock:slaves");

                    ui.checkbox(&mut ui_state.optional_tabs.slaves.enabled, "Slaves");
                    ui.checkbox(&mut ui_state.optional_tabs.property.enabled, "Property");
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut ui_state.optional_tabs.bonded_servants.enabled,
                            "Bonded servants",
                        );
                        ui.add_space(6.0);
                        ui.label("Tab name");
                        ui.add(
                            egui::TextEdit::singleline(
                                &mut ui_state.optional_tabs.bonded_servants_label,
                            )
                            .hint_text("Bonded"),
                        );
                    });
                    ui.checkbox(&mut ui_state.optional_tabs.concubines.enabled, "Concubines");
                    ui.checkbox(&mut ui_state.optional_tabs.harem_members.enabled, "Harem members");
                    ui.checkbox(&mut ui_state.optional_tabs.prisoners.enabled, "Prisoners");
                    ui.checkbox(&mut ui_state.optional_tabs.npcs_on_mission.enabled, "NPCs on mission");

                    ui.add_space(6.0);
                    let status = optional_tabs_status(ui_state);
                    ui.label(format!("Unlocked: {}", status));
                });
        });

    ui_state.show_options = open;
}

fn optional_tabs_status(ui_state: &UiState) -> String {
    let mut unlocked = Vec::new();
    if ui_state.optional_tabs.slaves.unlocked {
        unlocked.push("Slaves");
    }
    if ui_state.optional_tabs.property.unlocked {
        unlocked.push("Property");
    }
    if ui_state.optional_tabs.bonded_servants.unlocked {
        unlocked.push("Bonded servants");
    }
    if ui_state.optional_tabs.concubines.unlocked {
        unlocked.push("Concubines");
    }
    if ui_state.optional_tabs.harem_members.unlocked {
        unlocked.push("Harem members");
    }
    if ui_state.optional_tabs.prisoners.unlocked {
        unlocked.push("Prisoners");
    }
    if ui_state.optional_tabs.npcs_on_mission.unlocked {
        unlocked.push("NPCs on mission");
    }
    if unlocked.is_empty() {
        "none".to_string()
    } else {
        unlocked.join(", ")
    }
}

fn first_visible_left_tab(ui_state: &UiState) -> LeftTab {
    let ordered = [
        LeftTab::Party,
        LeftTab::Npcs,
        LeftTab::Quests,
        LeftTab::Factions,
        LeftTab::Slaves,
        LeftTab::Property,
        LeftTab::BondedServants,
        LeftTab::Concubines,
        LeftTab::HaremMembers,
        LeftTab::Prisoners,
        LeftTab::NpcsOnMission,
    ];

    for tab in ordered {
        if ui_state.is_left_tab_visible(tab) {
            return tab;
        }
    }

    LeftTab::Party
}

/* =========================
   Config Helpers
   ========================= */

fn color_picker(ui: &mut egui::Ui, label: &str, color: &mut SerializableColor) {
    let mut temp: egui::Color32 = (*color).into();
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.color_edit_button_srgba(&mut temp).changed() {
            *color = temp.into();
        }
    });
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("UnlimitedRPG");
    fs::create_dir_all(&path).ok();
    path.push("config.json");
    path
}

pub(crate) fn save_config(ui: &UiState) {
    let cfg = AppConfig {
        ui_scale: ui.ui_scale,
        text_scale: ui.text_scale,
        chat_text_scale: ui.chat_text_scale,
        speaker_colors: ui.speaker_colors.clone(),
        llm_base_url: ui.llm_base_url.clone(),
        llm_model: ui.llm_model.clone(),
        llm_api_key: ui.llm_api_key.clone(),
        llm_api_mode: ui.llm_api_mode,
        chat_log_limit: ui.chat_log_limit,
        save_full_chat_log: ui.save_full_chat_log,
        prompt_history_limit: ui.prompt_history_limit,
        timing_enabled: ui.timing_enabled,
        npc_recent_messages_limit: ui.npc_recent_messages_limit.max(1),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        let _ = fs::write(config_path(), json);
    }
}

fn load_config(ui: &mut UiState) {
    if let Ok(data) = fs::read_to_string(config_path()) {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&data) {
            ui.ui_scale = cfg.ui_scale;
            ui.text_scale = cfg.text_scale;
            ui.chat_text_scale = cfg.chat_text_scale;
            ui.speaker_colors = cfg.speaker_colors;
            ui.llm_base_url = if cfg.llm_base_url.is_empty() {
                "http://localhost:1234/v1".into()
            } else {
                cfg.llm_base_url
            };
            ui.llm_model = if cfg.llm_model.is_empty() {
                "local-model".into()
            } else {
                cfg.llm_model
            };
            ui.llm_api_key = cfg.llm_api_key;
            ui.llm_api_mode = cfg.llm_api_mode;
            ui.chat_log_limit = cfg.chat_log_limit;
            ui.save_full_chat_log = cfg.save_full_chat_log;
            ui.prompt_history_limit = cfg.prompt_history_limit;
            ui.timing_enabled = cfg.timing_enabled;
            ui.npc_recent_messages_limit = cfg.npc_recent_messages_limit.max(1);
            sanitize_ui_scales(ui);
            ui.apply_chat_log_limit();
        }
    }
}

const MIN_UI_SCALE: f32 = 0.75;
const MAX_UI_SCALE: f32 = 1.5;
const MIN_TEXT_SCALE: f32 = 0.75;
const MAX_TEXT_SCALE: f32 = 1.5;
const MIN_CHAT_TEXT_SCALE: f32 = 0.75;
const MAX_CHAT_TEXT_SCALE: f32 = 2.0;

fn sanitize_ui_scales(ui: &mut UiState) {
    ui.ui_scale = sanitize_scale(ui.ui_scale, 1.0, MIN_UI_SCALE, MAX_UI_SCALE);
    ui.text_scale = sanitize_scale(ui.text_scale, 1.0, MIN_TEXT_SCALE, MAX_TEXT_SCALE);
    ui.chat_text_scale =
        sanitize_scale(ui.chat_text_scale, 1.0, MIN_CHAT_TEXT_SCALE, MAX_CHAT_TEXT_SCALE);
}

fn sanitize_scale(value: f32, default: f32, min: f32, max: f32) -> f32 {
    if !value.is_finite() {
        return default;
    }
    value.clamp(min, max)
}

fn apply_text_scale(ctx: &egui::Context, ui_state: &mut UiState) {
    if ui_state.base_text_sizes.is_none() {
        let mut base = HashMap::new();
        for (style, font_id) in &ctx.style().text_styles {
            base.insert(style.clone(), font_id.size);
        }
        ui_state.base_text_sizes = Some(base);
    }

    let Some(base) = ui_state.base_text_sizes.as_ref() else {
        return;
    };

    let mut style = (*ctx.style()).clone();
    for (text_style, base_size) in base {
        if let Some(font_id) = style.text_styles.get_mut(text_style) {
            font_id.size = base_size * ui_state.text_scale;
        } else {
            style.text_styles.insert(
                text_style.clone(),
                egui::FontId::proportional(base_size * ui_state.text_scale),
            );
        }
    }
    ctx.set_style(style);
}

const CHARACTER_PNG_KEY: &str = "UPF_CHARACTER_JSON";

fn load_image_rgba(path: &Path) -> anyhow::Result<(u32, u32, Vec<u8>)> {
    let image = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((width, height, rgba.into_raw()))
}

fn extract_character_json_from_png(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info().ok()?;
    let info = reader.info();

    for chunk in &info.utf8_text {
        if chunk.keyword == CHARACTER_PNG_KEY {
            if let Ok(text) = chunk.get_text() {
                return Some(text);
            }
        }
    }
    for chunk in &info.uncompressed_latin1_text {
        if chunk.keyword == CHARACTER_PNG_KEY {
            return Some(chunk.text.clone());
        }
    }
    for chunk in &info.compressed_latin1_text {
        if chunk.keyword == CHARACTER_PNG_KEY {
            if let Ok(text) = chunk.get_text() {
                return Some(text);
            }
        }
    }
    None
}

fn force_png_extension(mut path: PathBuf) -> Option<PathBuf> {
    let needs_png = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => !ext.eq_ignore_ascii_case("png"),
        None => true,
    };
    if needs_png {
        path.set_extension("png");
    }
    Some(path)
}

fn write_png_with_character_json(
    path: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
    json: &str,
) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.add_itxt_chunk(CHARACTER_PNG_KEY.to_string(), json.to_string())?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}
