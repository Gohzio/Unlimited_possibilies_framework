use eframe::egui;
use egui::Layout;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use super::left_panel::draw_left_panel;
use super::center_panel::draw_center_panel;
use super::right_panel::draw_right_panel;

use crate::engine::engine::Engine;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::event_result::EventApplyOutcome;
use crate::model::game_state::GameStateSnapshot;
use crate::model::message::{Message, RoleplaySpeaker};

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
            background: "Describe your character’s origin, motivations, and past.".into(),
            stats,
            powers: vec!["Basic combat training".into()],
            features: vec![],
            inventory: vec!["Simple clothing".into()],
        }
    }
}

/* =========================
   Tabs
   ========================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Settings,
    Party,
    Options,
}

impl Default for LeftTab {
    fn default() -> Self {
        LeftTab::Settings
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Player,
    World,
}

impl Default for RightTab {
    fn default() -> Self {
        RightTab::Player
    }
}

/* =========================
   UI State
   ========================= */

#[derive(Default)]
pub struct UiState {
    pub input_text: String,
    pub selected_llm_path: Option<PathBuf>,
    pub rendered_messages: Vec<Message>,
    pub snapshot: Option<GameStateSnapshot>,
    pub new_stat_name: String,
    pub new_stat_value: i32,

    pub ui_scale: f32,
    pub should_auto_scroll: bool,

    pub left_tab: LeftTab,
    pub right_tab: RightTab,

    pub world: WorldDefinition,
    pub character: CharacterDefinition,
}

/* =========================
   Theme
   ========================= */

#[derive(Clone)]
struct Theme {
    user: egui::Color32,
    narrator: egui::Color32,
    npc: egui::Color32,
    party_member: egui::Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            user: egui::Color32::from_rgb(40, 70, 120),
            narrator: egui::Color32::from_rgb(80, 80, 80),
            npc: egui::Color32::from_rgb(40, 90, 60),
            party_member: egui::Color32::from_rgb(90, 60, 40),
        }
    }
}

/* =========================
   App
   ========================= */

pub struct MyApp {
    pub ui: UiState,
    theme: Theme,

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

        Self {
            ui: UiState {
                ui_scale: 1.0,
                ..Default::default()
            },
            theme: Theme::default(),
            cmd_tx,
            resp_rx,
        }
    }

    
    pub fn draw_message(&self, ui: &mut egui::Ui, msg: &Message) {
        let (bg, right, text) = match msg {
            Message::User(t) => (self.theme.user, true, format!("You: {t}")),
            Message::Roleplay { speaker, text } => {
                let c = match speaker {
                    RoleplaySpeaker::Narrator => self.theme.narrator,
                    RoleplaySpeaker::Npc => self.theme.npc,
                    RoleplaySpeaker::PartyMember => self.theme.party_member,
                };
                (c, false, text.clone())
            }
            Message::System(t) => (egui::Color32::DARK_GRAY, false, t.clone()),
        };
impl MyApp {
    pub fn send_command(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

        ui.add_space(6.0);

        if right {
            ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                bubble(ui, bg, &text);
            });
        } else {
            bubble(ui, bg, &text);
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
                        let t = match a.outcome {
                            EventApplyOutcome::Applied =>
                                format!("✔ Applied: {}", a.event.short_name()),
                            EventApplyOutcome::Rejected { reason } =>
                                format!("❌ Rejected: {}\n{}", a.event.short_name(), reason),
                            EventApplyOutcome::Deferred { reason } =>
                                format!("⚠ Deferred: {}\n{}", a.event.short_name(), reason),
                        };
                        self.ui.rendered_messages.push(Message::System(t));
                    }
                    self.ui.should_auto_scroll = true;
                }
            }
        }

        draw_left_panel(ctx, &mut self.ui);
        draw_right_panel(ctx, &mut self.ui);
        draw_center_panel(ctx, self);


        self.ui.should_auto_scroll = false;
    }
}

/* =========================
   Shared UI Helpers
   ========================= */

pub fn editable_list(ui: &mut egui::Ui, items: &mut Vec<String>, hint: &str) {
    let mut to_remove: Option<usize> = None;
    let mut new_item = String::new();

    for (i, item) in items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(item);
            if ui.small_button("❌").clicked() {
                to_remove = Some(i);
            }
        });
    }

    if let Some(i) = to_remove {
        items.remove(i);
    }

    ui.separator();

    ui.horizontal(|ui| {
        ui.add_sized(
            [200.0, 20.0],
            egui::TextEdit::singleline(&mut new_item).hint_text(hint),
        );

        if ui.button("Add").clicked() && !new_item.trim().is_empty() {
            items.push(new_item);
        }
    });
}

fn bubble(ui: &mut egui::Ui, color: egui::Color32, text: &str) {
    egui::Frame::none()
        .fill(color)
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(egui::Color32::WHITE));
        });
}
