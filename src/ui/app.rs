use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use egui::Layout;
use crate::engine::engine::Engine;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::message::{Message, RoleplaySpeaker};

#[derive(Default)]
struct UiState {
    input_text: String,
    selected_llm_path: Option<PathBuf>,
    rendered_messages: Vec<Message>,

    ui_scale: f32,
    should_auto_scroll: bool,
    show_theme_window: bool,
}

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
        Message::User(t) => (
            self.theme.user,
            true,
            format!("You: {}", t),


        ),

        Message::Roleplay { speaker, text } => {
            let color = match speaker {
                RoleplaySpeaker::Narrator => self.theme.narrator,
                RoleplaySpeaker::Npc => self.theme.npc,
                RoleplaySpeaker::PartyMember => self.theme.party_member,
            };

            (color, false, text.clone())
        }

        Message::System(t) => (
            egui::Color32::DARK_GRAY,
            false,
            t.clone(),
        ),
    };

    ui.add_space(6.0);

    if align_right {
        ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
            egui::Frame::none()
                .fill(bg_color)
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .color(egui::Color32::WHITE)
                            .size(16.0),
                    );
                });
        });
    } else {
        egui::Frame::none()
            .fill(bg_color)
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .color(egui::Color32::WHITE)
                        .size(16.0),
                );
            });
        }
    }
}


impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(self.ui.ui_scale);

        while let Ok(resp) = self.resp_rx.try_recv() {
            if let EngineResponse::FullMessageHistory(messages) = resp {
                self.ui.rendered_messages = messages;
                self.ui.should_auto_scroll = true;
            }
        }

        // Settings panel
        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .show(ctx, |ui| {
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
            });

        // Theme window
        if self.ui.show_theme_window {
            egui::Window::new("Theme")
                .open(&mut self.ui.show_theme_window)
                .show(ctx, |ui| {
                    ui.label("Speaker Colours");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("User");
                        ui.color_edit_button_srgba(&mut self.theme.user);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Narrator");
                        ui.color_edit_button_srgba(&mut self.theme.narrator);
                    });

                    ui.horizontal(|ui| {
                        ui.label("NPC");
                        ui.color_edit_button_srgba(&mut self.theme.npc);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Party Member");
                        ui.color_edit_button_srgba(&mut self.theme.party_member);
                    });
                });
        }

        // Input
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .default_height(120.0)
            .show(ctx, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.ui.input_text)
                        .hint_text("Start typing…")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );

                let send = response.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);

                if send {
                    let text = self.ui.input_text.trim().to_string();
                    if !text.is_empty() {
                        let _ = self.cmd_tx.send(EngineCommand::UserInput(text));
                        self.ui.input_text.clear();
                    }

                    ui.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    });
                }
            });

        // Messages
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