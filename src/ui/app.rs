use eframe::egui;
use egui::Layout;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::engine::engine::Engine;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::event_result::EventApplyOutcome;
use crate::model::game_state::GameStateSnapshot;
use crate::model::message::{Message, RoleplaySpeaker};

/* =========================
   World Definition
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldDefinition {
    title: String,
    world_id: String,
    author: String,

    description: String,
    themes: Vec<String>,
    tone: Vec<String>,

    narrator_role: String,
    style_guidelines: Vec<String>,
    opening_message: String,

    must_not: Vec<String>,
    must_always: Vec<String>,
}

impl Default for WorldDefinition {
    fn default() -> Self {
        Self {
            title: "Untitled World".into(),
            world_id: "world_001".into(),
            author: "Your name".into(),

            description:
                "Describe the world, its rules, factions, and overall premise."
                    .into(),

            themes: vec!["Power".into(), "Legacy".into()],
            tone: vec!["Serious".into(), "Epic".into()],

            narrator_role:
                "Act as the narrator and all NPCs. Never control the player."
                    .into(),

            style_guidelines: vec![
                "Show, don’t tell".into(),
                "Stay immersive".into(),
            ],

            opening_message:
                "The adventure begins at the edge of the known world…"
                    .into(),

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
struct CharacterDefinition {
    name: String,
    class: String,
    background: String,

    /// Free-form stats chosen by the player
    stats: HashMap<String, i32>,

    /// Active abilities
    powers: Vec<String>,

    /// Innate traits, boons, curses, blessings
    features: Vec<String>,

    /// Items / equipment
    inventory: Vec<String>,
}

impl Default for CharacterDefinition {
    fn default() -> Self {
        let mut stats = HashMap::new();
        stats.insert("strength".into(), 10);
        stats.insert("constitution".into(), 10);
        stats.insert("agility".into(), 10);
        stats.insert("intelligence".into(), 10);
        stats.insert("luck".into(), 10);

        Self {
            name: "Unnamed Hero".into(),
            class: "Adventurer".into(),
            background:
                "Describe your character’s origin, motivations, and past."
                    .into(),

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
enum LeftTab {
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
enum RightTab {
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
struct UiState {
    input_text: String,
    selected_llm_path: Option<PathBuf>,
    rendered_messages: Vec<Message>,
    snapshot: Option<GameStateSnapshot>,
    new_stat_name: String,
    new_stat_value: i32,

    ui_scale: f32,
    should_auto_scroll: bool,
    show_theme_window: bool,

    left_tab: LeftTab,
    right_tab: RightTab,

    world: WorldDefinition,
    character: CharacterDefinition,
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

        /* LEFT PANEL */
        egui::SidePanel::left("left").resizable(false).default_width(180.0).show(ctx, |ui| {
        
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ui.left_tab, LeftTab::Settings, "Settings");
                ui.selectable_value(&mut self.ui.left_tab, LeftTab::Party, "Party");
                ui.selectable_value(&mut self.ui.left_tab, LeftTab::Options, "Options");
            });

            ui.separator();

            if self.ui.left_tab == LeftTab::Settings {
                ui.label("UI Scale");
                ui.add(egui::Slider::new(&mut self.ui.ui_scale, 0.75..=2.0));
            }
        });

        /* RIGHT PANEL */
        egui::SidePanel::right("right").resizable(true).default_width(340.0).min_width(260.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ui.right_tab, RightTab::Player, "Player");
                ui.selectable_value(&mut self.ui.right_tab, RightTab::World, "World");
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.ui.right_tab {
                    RightTab::Player => draw_character(ui, &mut self.ui),
                    RightTab::World => draw_world(ui, &mut self.ui.world),
                }
            });
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
   UI Helpers
   ========================= */
fn editable_list(ui: &mut egui::Ui, items: &mut Vec<String>, hint: &str) {
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

fn draw_character(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.character;

    ui.heading("Character");

    ui.label("Name");
    ui.text_edit_singleline(&mut c.name);

    ui.label("Class");
    ui.text_edit_singleline(&mut c.class);

    ui.collapsing("Background", |ui| {
        ui.text_edit_multiline(&mut c.background);
    });
fn list(ui: &mut egui::Ui, label: &str, items: &Vec<String>) {
    ui.collapsing(label, |ui| {
        if items.is_empty() {
            ui.label("None");
        } else {
            for i in items {
                ui.label(format!("• {i}"));
            }
        }
    });
}

    /* -------- Stats -------- */

    ui.collapsing("Stats", |ui| {
        let mut to_remove: Option<String> = None;

        // Existing stats
        let keys: Vec<String> = c.stats.keys().cloned().collect();
        for key in keys {
            if let Some(value) = c.stats.get_mut(&key) {
                ui.horizontal(|ui| {
                    ui.label(&key);
                    ui.add(egui::DragValue::new(value).speed(1));

                    if ui.small_button("❌").clicked() {
                        to_remove = Some(key.clone());
                    }
                });
            }
        }

        if let Some(key) = to_remove {
            c.stats.remove(&key);
        }

        ui.separator();

        // Add new stat
        ui.label("Add new stat");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.new_stat_name);
            ui.add(
                egui::DragValue::new(&mut state.new_stat_value)
                    .speed(1)
                    .clamp_range(0..=999),
            );

            if ui.button("Add").clicked() {
                let name = state.new_stat_name.trim();

                if !name.is_empty() && !c.stats.contains_key(name) {
                    c.stats.insert(name.to_string(), state.new_stat_value);
                    state.new_stat_name.clear();
                    state.new_stat_value = 10;
                }
            }
        });
    });

    /* -------- Lists -------- */

    list(ui, "Powers", &c.powers);
    list(ui, "Features & Boons", &c.features);
    list(ui, "Inventory", &c.inventory);
}

fn draw_world(ui: &mut egui::Ui, w: &mut WorldDefinition) {
    ui.heading("World Definition");

    ui.separator();
    ui.label("Title");
    ui.text_edit_singleline(&mut w.title);

    ui.label("World ID");
    ui.text_edit_singleline(&mut w.world_id);

    ui.label("Author");
    ui.text_edit_singleline(&mut w.author);

    ui.separator();
    ui.collapsing("Description", |ui| {
        ui.text_edit_multiline(&mut w.description);
    });

    ui.collapsing("Themes", |ui| {
        editable_list(ui, &mut w.themes, "Add theme");
    });

    ui.collapsing("Tone", |ui| {
        editable_list(ui, &mut w.tone, "Add tone");
    });

    ui.separator();
    ui.collapsing("Narration & Style", |ui| {
        ui.label("Narrator Role");
        ui.text_edit_multiline(&mut w.narrator_role);

        ui.separator();
        ui.label("Style Guidelines");
        editable_list(ui, &mut w.style_guidelines, "Add guideline");
    });

    ui.separator();
    ui.collapsing("Opening Message", |ui| {
        ui.text_edit_multiline(&mut w.opening_message);
    });

    ui.separator();
    ui.collapsing("Hard Constraints", |ui| {
        ui.label("Must NOT");
        editable_list(ui, &mut w.must_not, "Add restriction");

        ui.separator();
        ui.label("Must ALWAYS");
        editable_list(ui, &mut w.must_always, "Add rule");
    });
}

