use ui::app::MyApp;
mod ui;
mod engine;
mod model;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Egui Settings App",
        options,
        Box::new(|_cc| Box::new(ui::app::MyApp::new())),
    )
}
