mod ui;
mod engine;
mod model;
use eframe;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Unlimited Possibilities Framework",
        options,
        Box::new(|_cc| {
            Ok(Box::new(ui::app::MyApp::new()))
        }),
    )
}
