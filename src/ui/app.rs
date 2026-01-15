use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;

use crate::engine::engine::Engine;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::message::Message;

#[derive(Default)]
struct UiState {
    input_text: String,
    selected_llm_path: Option<PathBuf>,
    rendered_messages: Vec<Message>,
}

pub struct MyApp {
    ui: UiState,

    cmd_tx: mpsc::Sender<EngineCommand>,
    resp_rx: mpsc::Receiver<EngineResponse>,

    engine: Engine,
}

impl MyApp {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();

        let engine = Engine::new(cmd_rx, resp_tx);

        Self {
            ui: UiState::default(),
            cmd_tx,
            resp_rx,
            engine,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.engine.update();

        while let Ok(resp) = self.resp_rx.try_recv() {
            match resp {
                EngineResponse::FullMessageHistory(messages) => {
                    self.ui.rendered_messages = messages;
                }
            }
        }

        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                if ui.button("Select LLM File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Select LLM File")
                        .pick_file()
                    {
                        self.ui.selected_llm_path = Some(path.clone());
                        let _ = self.cmd_tx.send(EngineCommand::LoadLlm(path));
                    }
                }
            });

        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .default_height(120.0)
            .show(ctx, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.ui.input_text)
                        .hint_text("Start typing...")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let text = self.ui.input_text.trim().to_string();
                    if !text.is_empty() {
                        let _ = self.cmd_tx.send(EngineCommand::UserInput(text));
                        self.ui.input_text.clear();
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Messages");
            ui.separator();

            for msg in &self.ui.rendered_messages {
                match msg {
                    Message::User(t) => {
                        ui.colored_label(egui::Color32::LIGHT_BLUE, format!("You: {}", t));
                    }
                    Message::Roleplay(t) => {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, t);
                    }
                    Message::System(t) => {
                        ui.colored_label(egui::Color32::GRAY, t);
                    }
                }
            }
        });
    }
}
