use eframe::egui;
use std::path::PathBuf;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Egui Settings App",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
}

#[derive(Default)]
struct MyApp {
    input_text: String,
    selected_llm_path: Option<PathBuf>,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Left side panel (Settings)
        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                if ui.button("Select LLM File").clicked() {
                    self.selected_llm_path = rfd::FileDialog::new()
                        .set_title("Select LLM File")
                        .pick_file();
                }

                // Optional: show selected file (purely informational)
                if let Some(path) = &self.selected_llm_path {
                    ui.separator();
                    ui.label("Selected file:");
                    ui.small(path.display().to_string());
                }
            });

        // Bottom input panel
        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .default_height(120.0)
            .show(ctx, |ui| {
                ui.label("Type something:");

                ui.add(
                    egui::TextEdit::multiline(&mut self.input_text)
                        .hint_text("Start typing...")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
            });

        // Central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Main Content");
            ui.separator();
            ui.label("LLM interaction or output will go here.");
        });
    }
}


