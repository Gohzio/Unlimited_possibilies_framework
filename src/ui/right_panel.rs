use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::ui::app::{RightTab, UiState};

/// Draws the right-hand panel for editing Player or World info.
pub fn draw_right_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    cmd_tx: &Sender<EngineCommand>,
) {
    egui::SidePanel::right("right_panel")
        .resizable(true)
        .default_width(340.0)
        .min_width(260.0)
        .show(ctx, |ui| {
            // Tab selector
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.right_tab, RightTab::Player, "Player");
                ui.selectable_value(&mut ui_state.right_tab, RightTab::World, "World");
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match ui_state.right_tab {
                    RightTab::Player => draw_player(ui, ui_state),
                    RightTab::World => draw_world(ui, ui_state, cmd_tx),
                }
            });
        });
}

/* =========================
   Player UI
   ========================= */

fn draw_player(ui: &mut egui::Ui, state: &mut UiState) {
    ui.heading("Character");

    // Save / Load buttons
    let mut do_save = false;
    let mut do_load = false;
    let mut do_upload = false;

    ui.horizontal(|ui| {
        if ui.button("🖼 Upload Image").clicked() {
            do_upload = true;
        }
        if ui.button("💾 Save Character").clicked() {
            do_save = true;
        }
        if ui
            .add_enabled(!state.player_locked, egui::Button::new("📂 Load Character"))
            .on_disabled_hover_text("Character is locked in")
            .clicked()
        {
            do_load = true;
        }
    });

    if do_save {
        state.save_character();
    }
    if do_load {
        if let Some(c) = state.load_character_from_dialog(ui.ctx()) {
            state.character = c;
        }
    }
    if do_upload {
        state.load_character_image_from_dialog(ui.ctx());
    }

    if let Some(texture) = &state.character_image {
        let width = ui.available_width();
        let height = match state.character_image_size {
            Some((w, h)) if w > 0 => width * (h as f32 / w as f32),
            _ => width,
        };
        ui.add(
            egui::Image::from_texture(texture)
                .fit_to_exact_size(egui::Vec2::new(width, height)),
        );
    }

    ui.separator();

    let c = &mut state.character;

    ui.collapsing("Details", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut c.name);

            ui.label("Class");
            ui.text_edit_singleline(&mut c.class);

            ui.label("Background");
            ui.text_edit_multiline(&mut c.background);
        });
    });

    ui.collapsing("Stats", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            let mut remove_key: Option<String> = None;
            for key in c.stats.keys().cloned().collect::<Vec<_>>() {
                if let Some(val) = c.stats.get_mut(&key) {
                    ui.horizontal(|ui| {
                        ui.label(&key);
                        ui.add(egui::DragValue::new(val).speed(1));
                        if ui.small_button("❌").clicked() {
                            remove_key = Some(key.clone());
                        }
                    });
                }
            }
            if let Some(key) = remove_key {
                c.stats.remove(&key);
            }

            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.new_stat_name);
                ui.add(egui::DragValue::new(&mut state.new_stat_value).speed(1).range(0..=999));
                if ui.button("Add").clicked() {
                    let name = state.new_stat_name.trim();
                    if !name.is_empty() && !c.stats.contains_key(name) {
                        c.stats.insert(name.to_string(), state.new_stat_value);
                        state.new_stat_name.clear();
                        state.new_stat_value = 10;
                    }
                }
            });
        });
    });

    ui.collapsing("Powers", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Powers", &mut c.powers, "Add power");
        });
    });

    ui.collapsing("Features & Boons", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Features & Boons", &mut c.features, "Add feature");
        });
    });

    ui.collapsing("Inventory", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Inventory", &mut c.inventory, "Add item");
        });
    });

    ui.add_space(6.0);
    if !state.player_locked {
        if ui
            .button("🔒 Lock In Character")
            .on_hover_text("Lock character fields until reset")
            .clicked()
        {
            state.player_locked = true;
        }
    } else if ui
        .button("🔓 Unlock Character")
        .on_hover_text("Unlock character fields for editing")
        .clicked()
    {
        state.player_locked = false;
    }
}

/* =========================
   World UI
   ========================= */

fn draw_world(ui: &mut egui::Ui, state: &mut UiState, cmd_tx: &Sender<EngineCommand>) {
    ui.heading("World Definition");

    let mut do_save = false;
    let mut do_load = false;

    ui.horizontal(|ui| {
        if ui.button("💾 Save World").clicked() {
            do_save = true;
        }
        if ui
            .add_enabled(!state.world_locked, egui::Button::new("📂 Load World"))
            .on_disabled_hover_text("World is locked in")
            .clicked()
        {
            do_load = true;
        }
    });

    if do_save {
        state.save_world();
    }

    if do_load {
        if let Some(world) = UiState::load_world_from_dialog() {
            state.world = world.clone();
            let _ = cmd_tx.send(EngineCommand::InitializeNarrative {
                opening_message: world.opening_message.clone(),
            });
        }
    }

    ui.separator();

    let w = &mut state.world;

    ui.add_enabled_ui(!state.world_locked, |ui| {
        ui.label("Title");
        ui.text_edit_singleline(&mut w.title);

        ui.label("World ID");
        ui.text_edit_singleline(&mut w.world_id);

        ui.label("Author");
        ui.text_edit_singleline(&mut w.author);
    });

    ui.collapsing("Description", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.text_edit_multiline(&mut w.description);
        });
    });

    ui.collapsing("Themes", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            editable_list(ui, "Themes", &mut w.themes, "Add theme");
        });
    });

    ui.collapsing("Tone", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            editable_list(ui, "Tone", &mut w.tone, "Add tone");
        });
    });

    ui.collapsing("Narration & Style", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Narrator Role");
            ui.text_edit_multiline(&mut w.narrator_role);
            ui.separator();
            editable_list(ui, "Style Guidelines", &mut w.style_guidelines, "Add guideline");
        });
    });

    ui.collapsing("Opening Message", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.text_edit_multiline(&mut w.opening_message);
        });
    });

    ui.collapsing("Hard Constraints", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Must NOT");
            editable_list(ui, "Must Not", &mut w.must_not, "Add restriction");

            ui.separator();
            ui.label("Must ALWAYS");
            editable_list(ui, "Must Always", &mut w.must_always, "Add rule");
        });
    });

    ui.add_space(6.0);
    if !state.world_locked {
        if ui
            .button("🔒 Lock In World")
            .on_hover_text("Lock world fields until reset")
            .clicked()
        {
            state.world_locked = true;
        }
    } else if ui
        .button("🔓 Unlock World")
        .on_hover_text("Unlock world fields for editing")
        .clicked()
    {
        state.world_locked = false;
    }
}

/* =========================
   Helper for editable string lists
   ========================= */

fn editable_list(ui: &mut egui::Ui, label: &str, items: &mut Vec<String>, placeholder: &str) {
    let mut remove_index: Option<usize> = None;
    for i in 0..items.len() {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut items[i]);
            if ui.small_button("❌").clicked() {
                remove_index = Some(i);
            }
        });
    }
    if let Some(i) = remove_index {
        items.remove(i);
    }

    ui.horizontal(|ui| {
        let id = ui.make_persistent_id(("editable_list_new_item", label));
        let mut new_item = ui
            .data_mut(|d| d.get_persisted::<String>(id))
            .unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut new_item).hint_text(placeholder));
        if ui.button("➕").clicked() {
            let trimmed = new_item.trim();
            if !trimmed.is_empty() {
                items.push(trimmed.to_string());
                new_item.clear();
            }
        }
        ui.data_mut(|d| d.insert_persisted(id, new_item));
    });
}
