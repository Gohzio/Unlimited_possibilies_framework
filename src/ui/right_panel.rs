use eframe::egui;

use super::app::{
    editable_list, CharacterDefinition, RightTab, UiState, WorldDefinition,
};

pub fn draw_right_panel(ctx: &egui::Context, ui_state: &mut UiState) {
    egui::SidePanel::right("right")
        .resizable(true)
        .default_width(340.0)
        .min_width(260.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.right_tab, RightTab::Player, "Player");
                ui.selectable_value(&mut ui_state.right_tab, RightTab::World, "World");
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match ui_state.right_tab {
                    RightTab::Player => draw_character(ui, ui_state),
                    RightTab::World => draw_world(ui, &mut ui_state.world),
                }
            });
        });
}

/* =========================
   Character UI
   ========================= */

fn draw_character(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.character;

    ui.heading("Character");

    ui.label("Name");
    ui.text_edit_singleline(&mut c.name);

    ui.label("Class");
    ui.text_edit_singleline(&mut c.class);

    ui.collapsing("Background", |ui| {
        ui.text_edit_multiline(&mut c.background);
    });

    ui.collapsing("Stats", |ui| {
        let mut to_remove: Option<String> = None;

        let keys: Vec<String> = c.stats.keys().cloned().collect();
        for key in keys {
            if let Some(value) = c.stats.get_mut(&key) {
                ui.horizontal(|ui| {
                    ui.label(&key);
                    ui.add(egui::DragValue::new(value).speed(1));

                    if ui.small_button("❌").clicked() {
                        to_remove = Some(key.clone());
                    }
                });
            }
        }

        if let Some(key) = to_remove {
            c.stats.remove(&key);
        }

        ui.separator();

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.new_stat_name);
            ui.add(
                egui::DragValue::new(&mut state.new_stat_value)
                    .speed(1)
                    .clamp_range(0..=999),
            );

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

    list(ui, "Powers", &c.powers);
    list(ui, "Features & Boons", &c.features);
    list(ui, "Inventory", &c.inventory);
}

fn list(ui: &mut egui::Ui, label: &str, items: &Vec<String>) {
    ui.collapsing(label, |ui| {
        if items.is_empty() {
            ui.label("None");
        } else {
            for i in items {
                ui.label(format!("• {i}"));
            }
        }
    });
}

/* =========================
   World UI
   ========================= */

fn draw_world(ui: &mut egui::Ui, w: &mut WorldDefinition) {
    ui.heading("World Definition");

    ui.separator();
    ui.label("Title");
    ui.text_edit_singleline(&mut w.title);

    ui.label("World ID");
    ui.text_edit_singleline(&mut w.world_id);

    ui.label("Author");
    ui.text_edit_singleline(&mut w.author);

    ui.separator();
    ui.collapsing("Description", |ui| {
        ui.text_edit_multiline(&mut w.description);
    });

    ui.collapsing("Themes", |ui| {
        editable_list(ui, &mut w.themes, "Add theme");
    });

    ui.collapsing("Tone", |ui| {
        editable_list(ui, &mut w.tone, "Add tone");
    });

    ui.separator();
    ui.collapsing("Narration & Style", |ui| {
        ui.label("Narrator Role");
        ui.text_edit_multiline(&mut w.narrator_role);

        ui.separator();
        ui.label("Style Guidelines");
        editable_list(ui, &mut w.style_guidelines, "Add guideline");
    });

    ui.separator();
    ui.collapsing("Opening Message", |ui| {
        ui.text_edit_multiline(&mut w.opening_message);
    });

    ui.separator();
    ui.collapsing("Hard Constraints", |ui| {
        ui.label("Must NOT");
        editable_list(ui, &mut w.must_not, "Add restriction");

        ui.separator();
        ui.label("Must ALWAYS");
        editable_list(ui, &mut w.must_always, "Add rule");
    });
}
