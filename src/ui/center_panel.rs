use eframe::egui;

use crate::engine::protocol::EngineCommand;
use super::app::MyApp;

pub fn draw_center_panel(ctx: &egui::Context, app: &mut MyApp) {
    // ---------- Input bar (BOTTOM) ----------
    egui::TopBottomPanel::bottom("chat_input").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let send = ui
                .add_sized(
                    [ui.available_width() - 60.0, 24.0],
                    egui::TextEdit::singleline(&mut app.ui.input_text)
                        .hint_text("Say something…"),
                )
                .lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if ui.button("Send").clicked() || send {
                let text = app.ui.input_text.trim().to_string();
                if !text.is_empty() {
                    app.send_command(EngineCommand::UserInput(text));
                    app.ui.input_text.clear();
                }
            }
        });
    });

    // ---------- Chat history (CENTER) ----------
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .stick_to_bottom(app.ui.should_auto_scroll)
            .show(ui, |ui| {
                for msg in &app.ui.rendered_messages {
                    app.draw_message(ui, msg);
                }
            });
    });
}
