use eframe::egui;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::fs;
use rfd::FileDialog;


use super::left_panel::draw_left_panel;
use super::center_panel::draw_center_panel;
use super::right_panel::draw_right_panel;

use crate::engine::engine::Engine;
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
        }
    }
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
    pub powers: Vec<String>,
    pub features: Vec<String>,
    pub inventory: Vec<String>,
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
            powers: vec!["Basic combat training".into()],
            features: vec![],
            inventory: vec!["Simple clothing".into()],
        }
    }
}

/* =========================
   Party
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartyMember {
    pub name: String,
    pub role: String,
    pub notes: String,
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
    pub should_auto_scroll: bool,

    pub world: WorldDefinition,
    pub character: CharacterDefinition,
    pub party: Vec<PartyMember>,

    pub speaker_colors: SpeakerColors,

    pub show_settings: bool,
    pub show_options: bool,

    pub llm_connected: bool,
    pub llm_status: String,

    pub left_tab: LeftTab,
    pub right_tab: RightTab,      // NEW: track which right panel tab is active
    pub new_stat_name: String,    // NEW: for adding new stats
    pub new_stat_value: i32,      // NEW: for adding new stats
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            input_text: String::new(),
            rendered_messages: Vec::new(),
            snapshot: None,

            ui_scale: 1.0,
            should_auto_scroll: true,

            world: WorldDefinition::default(),
            character: CharacterDefinition::default(),
            party: Vec::new(),

            speaker_colors: SpeakerColors::default(),

            show_settings: false,
            show_options: false,

            llm_connected: false,
            llm_status: "Not connected".into(),

            left_tab: LeftTab::Party,
            right_tab: RightTab::Player, // NEW: default tab
            new_stat_name: String::new(),
            new_stat_value: 10,
        }
    }
}

impl UiState {
    pub fn save_character(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("Character", &["json"])
            .set_file_name("character.json")
            .save_file()
        else {
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(&self.character) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_character_from_dialog() -> Option<CharacterDefinition> {
        let path = FileDialog::new()
            .add_filter("Character", &["json"])
            .pick_file()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str::<CharacterDefinition>(&data).ok()
    }

    pub fn save_world(&self) {
        let Some(path) = FileDialog::new()
            .add_filter("World", &["json"])
            .set_file_name("world.json")
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
            .pick_file()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str::<WorldDefinition>(&data).ok()
    }
}
/* =========================
   Config
   ========================= */

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub ui_scale: f32,
    pub speaker_colors: SpeakerColors,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            speaker_colors: SpeakerColors::default(),
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SerializableColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
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

        Self { ui, cmd_tx, resp_rx }
    }

    pub fn send_command(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn build_game_context(&self) -> GameContext {
        GameContext {
            world: self.ui.world.clone(),
            player: self.ui.character.clone(),
            party: self.ui.party.clone(),
            history: self.ui.rendered_messages.clone(),
            snapshot: self.ui.snapshot.clone(),
        }
    }
}

/* =========================
   egui App
   ========================= */

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.set_pixels_per_point(self.ui.ui_scale);

        while let Ok(resp) = self.resp_rx.try_recv() {
            match resp {
                EngineResponse::FullMessageHistory(msgs) => {
                    self.ui.rendered_messages = msgs;
                    self.ui.should_auto_scroll = true;
                }
                EngineResponse::NarrativeApplied { report, snapshot } => {
                    self.ui.snapshot = Some(snapshot);
                    for a in report.applications {
                        let t = format!("{:?}", a.outcome);
                        self.ui.rendered_messages.push(Message::System(t));
                    }
                }
                EngineResponse::LlmConnectionResult { success, message } => {
                    self.ui.llm_connected = success;
                    self.ui.llm_status = message;
                }
            }
        }

        draw_left_panel(ctx, &mut self.ui, &self.cmd_tx);
        draw_right_panel(ctx, &mut self.ui, &self.cmd_tx);
        draw_center_panel(ctx, self);

        draw_settings_window(ctx, &mut self.ui);
        draw_options_window(ctx, &mut self.ui, &self.cmd_tx);
    }
}

/* =========================
   Settings / Options Windows
   ========================= */

fn draw_settings_window(ctx: &egui::Context, ui_state: &mut UiState) {
    let mut open = ui_state.show_settings;

    egui::Window::new("⚙ Settings")
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("UI Scale");
            ui.add(egui::Slider::new(&mut ui_state.ui_scale, 0.75..=1.5));

            ui.separator();
            ui.heading("Speaker Colors");

            color_picker(ui, "Player", &mut ui_state.speaker_colors.player);
            color_picker(ui, "Narrator", &mut ui_state.speaker_colors.narrator);
            color_picker(ui, "NPC", &mut ui_state.speaker_colors.npc);
            color_picker(ui, "Party", &mut ui_state.speaker_colors.party);
            color_picker(ui, "System", &mut ui_state.speaker_colors.system);

            if ui.button("Save").clicked() {
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
    egui::Window::new("🛠 Options")
        .open(&mut ui_state.show_options)
        .show(ctx, |ui| {
            if ui.button("🔌 Connect to LM Studio").clicked() {
                let _ = cmd_tx.send(EngineCommand::ConnectToLlm);
            }

            ui.add_space(6.0);

            let status_color = if ui_state.llm_connected {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            };

            ui.label(egui::RichText::new(&ui_state.llm_status).color(status_color));
            ui.separator();
            ui.label("Advanced / Debug options will live here.");
        });
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
        speaker_colors: ui.speaker_colors.clone(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        let _ = fs::write(config_path(), json);
    }
}

fn load_config(ui: &mut UiState) {
    if let Ok(data) = fs::read_to_string(config_path()) {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&data) {
            ui.ui_scale = cfg.ui_scale;
            ui.speaker_colors = cfg.speaker_colors;
        }
    }
}
