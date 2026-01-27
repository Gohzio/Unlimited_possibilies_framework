use eframe::egui;

use crate::engine::protocol::EngineCommand;
use crate::model::message::{Message, RoleplaySpeaker};
use super::app::MyApp;

pub fn draw_center_panel(ctx: &egui::Context, app: &mut MyApp) {
    let input_id = egui::Id::new("chat_input_box");

    /* =========================
       Chat History (CENTER)
       ========================= */

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(app.ui.should_auto_scroll)
            .show(ui, |ui| {
                for msg in &app.ui.rendered_messages {
                    let (mut text, color) = match msg {
                        Message::User(t) => (
                            format!("You: {}", t),
                            egui::Color32::from_rgb(40, 70, 120),
                        ),

                        Message::Roleplay { speaker, text } => {
                            let c = match speaker {
                                RoleplaySpeaker::Narrator =>
                                    egui::Color32::from_rgb(180, 180, 180),
                                RoleplaySpeaker::Npc =>
                                    egui::Color32::from_rgb(120, 200, 160),
                                RoleplaySpeaker::PartyMember =>
                                    egui::Color32::from_rgb(200, 170, 120),
                            };
                            (text.clone(), c)
                        }

                        Message::System(t) => (
                            t.clone(),
                            egui::Color32::from_gray(150),
                        ),
                    };

                    egui::Frame::none()
                        .fill(color.linear_multiply(0.08))
                        .rounding(egui::Rounding::same(6.0))
                        .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut text)
                                    .interactive(false)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                            );
                        });

                    ui.add_space(6.0);
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

            // Enter vs Shift+Enter
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
                    EngineCommand::SubmitPlayerInput {
                        text,
                        context,
                    }
                );

                app.ui.input_text.clear();
            }

            // Keep cursor focused
            ui.memory_mut(|m| m.request_focus(input_id));
        }
    });
}
