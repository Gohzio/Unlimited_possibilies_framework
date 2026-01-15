use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;

// =====================
// MESSAGE MODEL
// =====================

#[derive(Clone)]
enum Message {
    User(String),
    System(String),
    Roleplay(String),
}

// =====================
// ENGINE SIDE
// =====================

enum EngineCommand {
    UserInput(String),
    LoadLlm(PathBuf),
}

enum EngineResponse {
    FullMessageHistory(Vec<Message>),
}

struct Engine {
    rx: mpsc::Receiver<EngineCommand>,
    tx: mpsc::Sender<EngineResponse>,

    messages: Vec<Message>,
}

impl Engine {
    fn new(
        rx: mpsc::Receiver<EngineCommand>,
        tx: mpsc::Sender<EngineResponse>,
    ) -> Self {
        Self {
            rx,
            tx,
            messages: Vec::new(),
        }
    }

    fn update(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                EngineCommand::UserInput(text) => {
                    // Engine assigns meaning
                    self.messages.push(Message::User(text.clone()));
                    self.messages.push(Message::Roleplay(format!(
                        "Echoing back: {}",
                        text
                    )));

                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                }

                EngineCommand::LoadLlm(path) => {
                    self.messages.push(Message::System(format!(
                        "Loaded LLM at: {}",
                        path.display()
                    )));

                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                }
            }
        }
    }
}

// =====================
// UI STATE
// =====================

#[derive(Default)]
struct UiState {
    input_text: String,
    selected_llm_path: Option<PathBuf>,

    // UI only mirrors engine output
    rendered_messages: Vec<Message>,
}

// =====================
// APP
// =====================

struct MyApp {
    ui: UiState,

    cmd_tx: mpsc::Sender<EngineCommand>,
    resp_rx: mpsc::Receiver<EngineResponse>,

    engine: Engine,
}

impl MyApp {
    fn new() -> Self {
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
        // Update engine
        self.engine.update();

        // Process engine responses
        while let Ok(resp) = self.resp_rx.try_recv() {
            match resp {
                EngineResponse::FullMessageHistory(messages) => {
                    self.ui.rendered_messages = messages;
                }
            }
        }

        // =====================
        // SETTINGS PANEL
        // =====================
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

                if let Some(path) = &self.ui.selected_llm_path {
                    ui.separator();
                    ui.label("Selected file:");
                    ui.small(path.display().to_string());
                }
            });

        // =====================
        // INPUT PANEL
        // =====================
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .default_height(120.0)
            .show(ctx, |ui| {
                ui.label("Type something:");

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

        // =====================
        // MESSAGE PANEL
        // =====================
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Messages");
            ui.separator();

            for msg in &self.ui.rendered_messages {
                match msg {
                    Message::User(text) => {
                        ui.colored_label(egui::Color32::LIGHT_BLUE, format!("You: {}", text));
                    }
                    Message::Roleplay(text) => {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, text);
                    }
                    Message::System(text) => {
                        ui.colored_label(egui::Color32::GRAY, text);
                    }
                }
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Egui Settings App",
        options,
        Box::new(|_cc| Box::new(MyApp::new())),
    )
}

