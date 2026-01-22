use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use egui::Layout;
use serde::{Deserialize, Serialize};

use crate::engine::engine::Engine;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::message::{Message, RoleplaySpeaker};
use crate::model::event_result::EventApplyOutcome;
use crate::model::game_state::GameStateSnapshot;

/* =========================
   World Definition (UI only)
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldDefinition {
    // Meta
    title: String,
    world_id: String,
    author: String,

    // World
    description: String,
    themes: Vec<String>,
    tone: Vec<String>,

    // Narrator
    narrator_role: String,
    style_guidelines: Vec<String>,
    opening_message: String,

    // Constraints
    must_not: Vec<String>,
    must_always: Vec<String>,
}

impl Default for WorldDefinition {
    fn default() -> Self {
        Self {
            // --- Meta ---
            title: "Untitled World".into(),
            world_id: "world_001".into(),
            author: "Your name or handle".into(),

            // --- World ---
            description: 
                "Describe the world setting, genre, and core premise.\n\
                 Example: A fractured empire ruled by ancient dragon-blooded queens."
                    .into(),

            themes: vec![
                "Power and legacy".into(),
                "Political intrigue".into(),
                "Myth and prophecy".into(),
            ],

            tone: vec![
                "Serious".into(),
                "Epic".into(),
                "Character-driven".into(),
            ],

            // --- Narrator ---
            narrator_role: 
                "Act as the narrator and primary voice of the world.\n\
                 Describe scenes, portray NPCs, and advance the story."
                    .into(),

            style_guidelines: vec![
                "Show, don’t tell".into(),
                "Use vivid but concise descriptions".into(),
                "Stay in third-person unless speaking as an NPC".into(),
            ],

            opening_message: 
                "The story begins with the player arriving at the edge of the known world..."
                    .into(),

            // --- Constraints ---
            must_not: vec![
                "Do not control the player character".into(),
                "Do not reveal hidden information unless discovered".into(),
                "Do not break immersion or reference being an AI".into(),
            ],

            must_always: vec![
                "Respond as the narrator or an in-world character".into(),
                "Respect established world rules and continuity".into(),
                "Use structured events when game state should change".into(),
            ],
        }
    }
}


/* =========================
   Tabs
   ========================= */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftTab {
    Settings,
    Party,
    Options,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightTab {
    Player,
    World,
}

impl Default for LeftTab {
    fn default() -> Self {
        LeftTab::Settings
    }
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
struct UiState {
    input_text: String,
    selected_llm_path: Option<PathBuf>,
    rendered_messages: Vec<Message>,

    snapshot: Option<GameStateSnapshot>,

    ui_scale: f32,
    should_auto_scroll: bool,
    show_theme_window: bool,

    left_tab: LeftTab,
    right_tab: RightTab,

    world: WorldDefinition,
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
    ui: UiState,
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

    fn draw_message(&self, ui: &mut egui::Ui, msg: &Message) {
        let (bg_color, align_right, label) = match msg {
            Message::User(t) => (self.theme.user, true, format!("You: {}", t)),
            Message::Roleplay { speaker, text } => {
                let color = match speaker {
                    RoleplaySpeaker::Narrator => self.theme.narrator,
                    RoleplaySpeaker::Npc => self.theme.npc,
                    RoleplaySpeaker::PartyMember => self.theme.party_member,
                };
                (color, false, text.clone())
            }
            Message::System(t) => (egui::Color32::DARK_GRAY, false, t.clone()),
        };

        ui.add_space(6.0);

        if align_right {
            ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                egui::Frame::none()
                    .fill(bg_color)
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(label).color(egui::Color32::WHITE));
                    });
            });
        } else {
            egui::Frame::none()
                .fill(bg_color)
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(label).color(egui::Color32::WHITE));
                });
        }
    }
}

/* =========================
   egui App impl
   ========================= */

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(self.ui.ui_scale);

        // --- Engine responses ---
        while let Ok(resp) = self.resp_rx.try_recv() {
            match resp {
                EngineResponse::FullMessageHistory(msgs) => {
                    self.ui.rendered_messages = msgs;
                    self.ui.should_auto_scroll = true;
                }
                EngineResponse::NarrativeApplied { report, snapshot } => {
                    self.ui.snapshot = Some(snapshot);
                    for application in report.applications {
                        let text = match application.outcome {
                            EventApplyOutcome::Applied => format!("✔ Applied: {}", application.event.short_name()),
                            EventApplyOutcome::Rejected { reason } =>
                                format!("❌ Rejected: {}\n{}", application.event.short_name(), reason),
                            EventApplyOutcome::Deferred { reason } =>
                                format!("⚠ Deferred: {}\n{}", application.event.short_name(), reason),
                        };
                        self.ui.rendered_messages.push(Message::System(text));
                    }
                    self.ui.should_auto_scroll = true;
                }
            }
        }

        /* LEFT PANEL */
egui::SidePanel::left("left_panel")
    .default_width(220.0)
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.ui.left_tab, LeftTab::Settings, "Settings");
            ui.selectable_value(&mut self.ui.left_tab, LeftTab::Party, "Party");
            ui.selectable_value(&mut self.ui.left_tab, LeftTab::Options, "Options");
        });

        ui.separator();

        match self.ui.left_tab {
            LeftTab::Settings => {
                ui.heading("Settings");
                ui.separator();

                ui.label("UI Scale");
                ui.add(
                    egui::Slider::new(&mut self.ui.ui_scale, 0.75..=2.0)
                        .step_by(0.05),
                );

                ui.separator();

                if ui.button("Theme…").clicked() {
                    self.ui.show_theme_window = true;
                }

                ui.separator();

                if ui.button("Select LLM File").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.ui.selected_llm_path = Some(path.clone());
                        let _ = self.cmd_tx.send(EngineCommand::LoadLlm(path));
                    }
                }
            }

            LeftTab::Party => {
                ui.heading("Party");

                if let Some(snapshot) = &self.ui.snapshot {
                    if snapshot.party.is_empty() {
                        ui.label("No party members yet");
                    } else {
                        for member in &snapshot.party {
                            ui.group(|ui| {
                                ui.label(&member.name);
                                ui.label(format!("Role: {}", member.role));
                                ui.label(format!("HP: {}", member.hp));
                            });
                        }
                    }
                } else {
                    ui.label("No snapshot yet");
                }
            }

            LeftTab::Options => {
                ui.heading("Options");
                ui.label("LLM configuration coming soon");
                ui.label("• Temperature");
                ui.label("• Context window");
                ui.label("• System prompt");
            }
        }
    });


        /* RIGHT PANEL */
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ui.right_tab, RightTab::Player, "Player");
                ui.selectable_value(&mut self.ui.right_tab, RightTab::World, "World");
            });
            ui.separator();

            match self.ui.right_tab {
                RightTab::Player => {
                    if let Some(snapshot) = &self.ui.snapshot {
                        ui.label(format!("Name: {}", snapshot.player.name));
                        ui.label(format!("Level: {}", snapshot.player.level));
                        ui.label(format!("HP: {}/{}", snapshot.player.hp, snapshot.player.max_hp));
                        ui.separator();
                        for stat in &snapshot.stats {
                            ui.label(format!("{}: {}", stat.id, stat.value));
                        }
                    }
                }

                RightTab::World => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading("World");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Save").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().save_file() {
                                        if let Ok(json) = serde_json::to_string_pretty(&self.ui.world) {
                                            let _ = std::fs::write(path, json);
                                        }
                                    }
                                }
                                if ui.button("Load").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        if let Ok(text) = std::fs::read_to_string(path) {
                                            if let Ok(world) = serde_json::from_str(&text) {
                                                self.ui.world = world;
                                            }
                                        }
                                    }
                                }
                            });
                        });

                        ui.separator();

                        ui.collapsing("Meta", |ui| {
                            ui.text_edit_singleline(&mut self.ui.world.title);
                            ui.text_edit_singleline(&mut self.ui.world.world_id);
                            ui.text_edit_singleline(&mut self.ui.world.author);
                        });

                        ui.collapsing("World", |ui| {
                            ui.text_edit_multiline(&mut self.ui.world.description);
                            multiline_vec(ui, &mut self.ui.world.themes);
                            multiline_vec(ui, &mut self.ui.world.tone);
                        });

                        ui.collapsing("Narrator", |ui| {
                            ui.text_edit_multiline(&mut self.ui.world.narrator_role);
                            multiline_vec(ui, &mut self.ui.world.style_guidelines);
                            ui.text_edit_multiline(&mut self.ui.world.opening_message);
                        });

                        ui.collapsing("Constraints", |ui| {
                            multiline_vec(ui, &mut self.ui.world.must_not);
                            multiline_vec(ui, &mut self.ui.world.must_always);
                        });
                    });
                }
            }
        });

        /* CENTER */
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(self.ui.should_auto_scroll)
                .show(ui, |ui| {
                    for msg in &self.ui.rendered_messages {
                        self.draw_message(ui, msg);
                    }
                });
        });

        self.ui.should_auto_scroll = false;
    }
}

/* =========================
   Helpers
   ========================= */

fn multiline_vec(ui: &mut egui::Ui, vec: &mut Vec<String>) {
    let mut text = vec.join("\n");
    if ui.text_edit_multiline(&mut text).changed() {
        *vec = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
    }
}
