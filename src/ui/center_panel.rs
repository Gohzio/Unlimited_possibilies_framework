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
                            speaker_colors: app.ui.speaker_colors.clone(),
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
                app.send_command(EngineCommand::SubmitPlayerInput {
                    text,
                    context,
                    llm: app.ui.llm_config(),
                });
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

    /* =========================
       Chat History (CENTER)
       ========================= */

    egui::CentralPanel::default().show(ctx, |ui| {
        let panel_rect = ui.max_rect();
        let scroll_output = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
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
                    let font_size = 14.0 * app.ui.chat_text_scale;

                    for ch in raw_text.chars() {
                        if ch == '*' {
                            if !buffer.is_empty() {
                                job.append(
                                    &buffer,
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::proportional(font_size),
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
                                font_id: FontId::proportional(font_size),
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

                if app.ui.should_auto_scroll {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    app.ui.should_auto_scroll = false;
                }
            });

        let is_at_bottom = is_scroll_at_bottom(&scroll_output);
        let input = ctx.input(|i| i.clone());
        let pointer_over_log = input
            .pointer
            .hover_pos()
            .map(|pos| scroll_output.inner_rect.contains(pos))
            .unwrap_or(false);
        if pointer_over_log && input.raw_scroll_delta.y.abs() > 0.0 {
            app.ui.chat_user_scrolled_up = !is_at_bottom;
        }
        if is_at_bottom {
            app.ui.chat_user_scrolled_up = false;
        }

        if app.ui.chat_user_scrolled_up && !is_at_bottom {
            let button_size = egui::vec2(26.0, 26.0);
            let button_pos = egui::pos2(
                panel_rect.left() + 8.0,
                panel_rect.bottom() - button_size.y - 8.0,
            );
            egui::Area::new(egui::Id::new("jump_to_latest_button"))
                .order(egui::Order::Foreground)
                .fixed_pos(button_pos)
                .show(ctx, |ui| {
                    let button = egui::Button::new("↓")
                        .corner_radius(egui::CornerRadius::same(255))
                        .min_size(button_size);
                    if ui
                        .add(button)
                        .on_hover_text("Jump to latest message")
                        .clicked()
                    {
                        app.ui.should_auto_scroll = true;
                    }
                });
        }
    });

}

fn is_scroll_at_bottom<R>(output: &egui::scroll_area::ScrollAreaOutput<R>) -> bool {
    let view_height = output.inner_rect.height();
    let content_height = output.content_size.y;
    if content_height <= view_height + 0.5 {
        return true;
    }
    let max_offset = (content_height - view_height).max(0.0);
    output.state.offset.y >= max_offset - 2.0
}
