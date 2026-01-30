use eframe::egui;
use eframe::egui::{FontId, TextFormat};
use egui::text::LayoutJob;

use crate::engine::protocol::EngineCommand;
use rfd::FileDialog;
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
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                for msg in &app.ui.rendered_messages {
                    let (raw_text, color) = match msg {
                        Message::User(t) => (
                            format!("You: {}", t),
                            app.ui.speaker_colors.player.into(),
                        ),

                        Message::Roleplay { speaker, text } => {
                            let c = match speaker {
                                RoleplaySpeaker::Narrator => app.ui.speaker_colors.narrator.into(),
                                RoleplaySpeaker::Npc => app.ui.speaker_colors.npc.into(),
                                RoleplaySpeaker::PartyMember => app.ui.speaker_colors.party.into(),
                            };
                            (text.clone(), c)
                        }

                        Message::System(t) => (
                            t.clone(),
                            app.ui.speaker_colors.system.into(),
                        ),
                    };

                    if raw_text.trim().is_empty() {
                        continue;
                    }

                    // --- Italics parsing (*emotion*)
                    let mut job = LayoutJob::default();
                    let mut italic = false;
                    let mut buffer = String::new();

                    for ch in raw_text.chars() {
                        if ch == '*' {
                            if !buffer.is_empty() {
                                job.append(
                                    &buffer,
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::proportional(14.0),
                                        color,
                                        italics: italic,
                                        ..Default::default()
                                    },
                                );
                                buffer.clear();
                            }
                            italic = !italic;
                        } else {
                            buffer.push(ch);
                        }
                    }

                    if !buffer.is_empty() {
                        job.append(
                            &buffer,
                            0.0,
                            TextFormat {
                                font_id: FontId::proportional(14.0),
                                color,
                                italics: italic,
                                ..Default::default()
                            },
                        );
                    }

                    ui.add(
                        egui::Label::new(job)
                            .wrap()
                            .selectable(true),
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
        let mut reset_session = false;
        let mut reset_all = false;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if ui
                    .small_button("⚙")
                    .on_hover_text("Settings")
                    .clicked()
                {
                    app.ui.show_settings = true;
                }
                if ui
                    .small_button("↺")
                    .on_hover_text("Restart chat (keep world/player)")
                    .clicked()
                {
                    reset_session = true;
                }

                if ui
                    .small_button("💾")
                    .on_hover_text("Save game state")
                    .clicked()
                {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Game Save", &["json"])
                        .set_file_name("save.json")
                        .set_directory(crate::ui::app::UiState::default_save_dir())
                        .save_file()
                    {
                        app.send_command(EngineCommand::SaveGame {
                            path,
                            world: app.ui.world.clone(),
                            player: app.ui.character.clone(),
                            party: app.ui.party.clone(),
                        });
                    }
                }
            });

            ui.vertical(|ui| {
                if ui
                    .small_button("🛠")
                    .on_hover_text("Options")
                    .clicked()
                {
                    app.ui.show_options = true;
                }
                if ui
                    .small_button("🧹")
                    .on_hover_text("Reset everything to defaults")
                    .clicked()
                {
                    reset_all = true;
                }

                if ui
                    .small_button("📂")
                    .on_hover_text("Load game state")
                    .clicked()
                {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Game Save", &["json"])
                        .set_directory(crate::ui::app::UiState::default_save_dir())
                        .pick_file()
                    {
                        app.send_command(EngineCommand::LoadGame { path });
                    }
                }
            });

            let send_button_width = 60.0;
            let text_width = ui.available_width() - send_button_width - 8.0;

            let response = ui.add_sized(
                [text_width.max(0.0), 60.0],
                egui::TextEdit::multiline(&mut app.ui.input_text)
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

            if ui
                .add_sized([send_button_width, 60.0], egui::Button::new("Send"))
                .clicked()
            {
                send_now = true;
            }
        });

        if send_now {
            let text = app.ui.input_text.trim().to_string();

            if !text.is_empty() {
                let context = app.build_game_context();
                app.send_command(EngineCommand::SubmitPlayerInput { text, context });
                app.ui.input_text.clear();
            }

            ui.memory_mut(|m| m.request_focus(input_id));
        }

        if reset_all {
            app.ui = crate::ui::app::UiState::default();
            let opening_message = app.ui.world.opening_message.clone();
            app.send_command(EngineCommand::InitializeNarrative { opening_message });
        } else if reset_session {
            let opening_message = app.ui.world.opening_message.clone();
            app.send_command(EngineCommand::InitializeNarrative { opening_message });
        }
    });
}
