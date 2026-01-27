use eframe::egui;

use crate::engine::protocol::EngineCommand;
use crate::model::message::Message;
use super::app::MyApp;

pub fn draw_center_panel(ctx: &egui::Context, app: &mut MyApp) {
    let input_id = egui::Id::new("chat_input_box");

    /* =========================
       Chat History (CENTER)
       ========================= */

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                for msg in &app.ui.rendered_messages {
                    let text = match msg {
                        Message::User(t) => format!("You: {}", t),
                        Message::Roleplay { text, .. } => text.clone(),
                        Message::System(t) => t.clone(),
                    };

                    // Skip empty / whitespace-only messages
                    if text.trim().is_empty() {
                        continue;
                    }

                    ui.add(
                        egui::Label::new(text)
                            .wrap()
                    );

                    ui.add_space(8.0);
                }
            });
    });

    /* =========================
       Input Bar (BOTTOM)
       ========================= */

    egui::TopBottomPanel::bottom("chat_input").show(ctx, |ui| {
        let mut send_now = false;

        ui.horizontal(|ui| {
            let response = ui.add_sized(
                [ui.available_width() - 60.0, 60.0],
                egui::TextEdit::multiline(&mut app.ui.input_text)
                    .id(input_id)
                    .hint_text("Say something…")
                    .lock_focus(true),
            );

            if response.has_focus() {
                let input = ui.input(|i| i.clone());
                if input.key_pressed(egui::Key::Enter) && !input.modifiers.shift {
                    send_now = true;
                }
            }

            if ui.button("Send").clicked() {
                send_now = true;
            }
        });

        if send_now {
            let text = app.ui.input_text.trim().to_string();

            if !text.is_empty() {
                let context = app.build_game_context();

                app.send_command(
                    EngineCommand::SubmitPlayerInput { text, context }
                );

                app.ui.input_text.clear();
            }

            ui.memory_mut(|m| m.request_focus(input_id));
        }
    });
}
