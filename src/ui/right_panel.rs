use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::ui::app::{PowerEntry, RightTab, UiState};

/// Draws the right-hand panel for editing Player or World info.
pub fn draw_right_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    cmd_tx: &Sender<EngineCommand>,
) {
    egui::SidePanel::right("right_panel")
        .resizable(true)
        .default_width(ui_state.right_panel_width)
        .min_width(260.0)
        .show(ctx, |ui| {
            ui_state.right_panel_width = ui.available_width().max(0.0);
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

    if ui.button("🖼 Upload Image").clicked() {
        do_upload = true;
    }
    ui.horizontal(|ui| {
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

    if let Some(snapshot) = &state.snapshot {
        let exp_to_next = snapshot.player.exp_to_next.max(1);
        let exp = snapshot.player.exp.max(0);
        let progress = (exp as f32 / exp_to_next as f32).clamp(0.0, 1.0);
        ui.add(
            egui::ProgressBar::new(progress)
                .text(format!("EXP: {}/{}", exp, exp_to_next)),
        );
        ui.label(format!("EXP to next level: {}", exp_to_next));
        ui.add_space(6.0);
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
            editable_power_list(ui, &mut c.powers, state.player_locked);
        });
    });

    ui.collapsing("Weapons", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Weapons", &mut c.weapons, "Add weapon");
        });
    });

    ui.collapsing("Armour", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Armour", &mut c.armor, "Add armour");
        });
    });

    ui.collapsing("Features & Boons", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Features & Boons", &mut c.features, "Add feature");
        });
    });

    ui.collapsing("Clothing", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Clothing", &mut c.clothing, "Add clothing item");
        });
    });

    ui.collapsing("Inventory", |ui| {
        ui.add_enabled_ui(!state.player_locked, |ui| {
            editable_list(ui, "Inventory", &mut c.inventory, "Add item");
        });
    });

    ui.collapsing("Currencies", |ui| {
        draw_currencies(ui, state);
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

fn draw_currencies(ui: &mut egui::Ui, state: &UiState) {
    let Some(snapshot) = &state.snapshot else {
        ui.label("No currencies yet.");
        return;
    };

    if snapshot.currencies.is_empty() {
        ui.label("No currencies yet.");
        return;
    }

    let mut currencies = snapshot.currencies.clone();
    currencies.sort_by(|a, b| a.currency.cmp(&b.currency));

    if let Some(gold) = currencies
        .iter()
        .find(|c| c.currency.eq_ignore_ascii_case("gold"))
    {
        ui.label(format!("Gold: {}", gold.amount));
        ui.add_space(6.0);
    }

    for currency in currencies {
        if currency.currency.eq_ignore_ascii_case("gold") {
            continue;
        }
        ui.label(format!("{}: {}", currency.currency, currency.amount));
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

    ui.collapsing("Loot Rules", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Mode");
            egui::ComboBox::from_id_salt("loot_rules_mode")
                .selected_text(w.loot_rules_mode.as_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut w.loot_rules_mode,
                        "Difficulty based".to_string(),
                        "Difficulty based",
                    );
                    ui.selectable_value(
                        &mut w.loot_rules_mode,
                        "Rarity based".to_string(),
                        "Rarity based",
                    );
                    ui.selectable_value(
                        &mut w.loot_rules_mode,
                        "Custom".to_string(),
                        "Custom",
                    );
                });

            ui.add_space(6.0);
            match w.loot_rules_mode.as_str() {
                "Difficulty based" => {
                    ui.label("Harder tasks yield better rewards.");
                }
                "Rarity based" => {
                    ui.label("Each drop can roll from any rarity tier:");
                    ui.label("Common, Uncommon, Rare, Legendary, Exotic, Godly");
                }
                _ => {}
            }

            ui.add_space(6.0);
            ui.label("Custom rules");
            ui.text_edit_multiline(&mut w.loot_rules_custom);
        });
    });

    ui.collapsing("Experience Rules", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Base EXP to reach level 2 is 100.");
            ui.label("Next level requirement multiplies by this value.");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Multiplier");
                ui.add(
                    egui::DragValue::new(&mut w.exp_multiplier)
                        .speed(0.1)
                        .range(1.0..=10.0),
                );
            });
        });
    });

    ui.collapsing("Skill Progression", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Repetition grants skills in tiers.");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Base threshold");
                ui.add(
                    egui::DragValue::new(&mut w.repetition_threshold)
                        .speed(1)
                        .range(1..=1000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Tier step");
                ui.add(
                    egui::DragValue::new(&mut w.repetition_tier_step)
                        .speed(1)
                        .range(1..=1000),
                );
            });

            ui.add_space(6.0);
            ui.label("Tier names (5):");
            ensure_skill_tier_names(&mut w.skill_tier_names);
            for i in 0..5 {
                let label = format!("Tier {}", i + 1);
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.text_edit_singleline(&mut w.skill_tier_names[i]);
                });
            }

            ui.add_space(8.0);
            ui.label("Per-skill thresholds (override base/step):");
            let mut remove_idx: Option<usize> = None;
            for (idx, entry) in w.skill_thresholds.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut entry.skill);
                    ui.add(
                        egui::DragValue::new(&mut entry.base)
                            .speed(1)
                            .range(1..=10000),
                    );
                    ui.add(
                        egui::DragValue::new(&mut entry.step)
                            .speed(1)
                            .range(1..=10000),
                    );
                    if ui.small_button("❌").clicked() {
                        remove_idx = Some(idx);
                    }
                });
                ensure_skill_tier_names(&mut entry.tier_names);
                for i in 0..5 {
                    let label = format!("  Tier {}", i + 1);
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.text_edit_singleline(&mut entry.tier_names[i]);
                    });
                }
            }
            if let Some(idx) = remove_idx {
                w.skill_thresholds.remove(idx);
            }
            ui.horizontal(|ui| {
                if ui.button("➕ Add Skill Override").clicked() {
                    w.skill_thresholds.push(crate::ui::app::SkillThreshold {
                        skill: "mining".to_string(),
                        base: w.repetition_threshold.max(1),
                        step: w.repetition_tier_step.max(1),
                        tier_names: w.skill_tier_names.clone(),
                    });
                }
            });
        });
    });

    ui.collapsing("Power Evolution", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.label("Power evolution triggers on repeated usage.");
            ui.horizontal(|ui| {
                ui.label("Base uses");
                ui.add(
                    egui::DragValue::new(&mut w.power_evolution_base)
                        .speed(1)
                        .range(1..=10000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Tier step");
                ui.add(
                    egui::DragValue::new(&mut w.power_evolution_step)
                        .speed(1)
                        .range(1..=10000),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Multiplier min");
                ui.add(
                    egui::DragValue::new(&mut w.power_evolution_multiplier_min)
                        .speed(0.1)
                        .range(1.0..=10.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Multiplier max");
                ui.add(
                    egui::DragValue::new(&mut w.power_evolution_multiplier_max)
                        .speed(0.1)
                        .range(1.0..=10.0),
                );
            });
        });
    });

    ui.collapsing("Quest Rules", |ui| {
        ui.add_enabled_ui(!state.world_locked, |ui| {
            ui.checkbox(&mut w.is_rpg_world, "Is an RPG world");
            ui.checkbox(&mut w.world_quests_enabled, "World can generate quests");
            ui.add_enabled_ui(w.world_quests_enabled, |ui| {
                ui.checkbox(
                    &mut w.world_quests_mandatory,
                    "World quests can be mandatory (non-declinable)",
                );
            });
            ui.checkbox(&mut w.npc_quests_enabled, "NPCs can offer quests");
            ui.separator();
            ui.label("World quest offer phrase:");
            ui.label("*ding* the world is offering you a quest.");
            ui.add_space(4.0);
            ui.label("NPC quest offer phrase:");
            ui.label("I hereby offer you a quest.");
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

fn editable_power_list(ui: &mut egui::Ui, items: &mut Vec<PowerEntry>, player_locked: bool) {
    let mut remove_index: Option<usize> = None;
    for i in 0..items.len() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.add_enabled(
                    !player_locked && !items[i].locked,
                    egui::TextEdit::singleline(&mut items[i].name)
                        .hint_text("Power/skill name"),
                );

                let lock_label = if items[i].locked { "🔒" } else { "🔓" };
                if ui
                    .add_enabled(!player_locked, egui::Button::new(lock_label))
                    .on_hover_text("Lock/unlock this power")
                    .clicked()
                {
                    items[i].locked = !items[i].locked;
                }

                if ui
                    .add_enabled(!player_locked && !items[i].locked, egui::Button::new("❌"))
                    .clicked()
                {
                    remove_index = Some(i);
                }
            });

            ui.add_enabled(
                !player_locked && !items[i].locked,
                egui::TextEdit::multiline(&mut items[i].description)
                    .hint_text("Description")
                    .desired_rows(2),
            );
        });
        ui.add_space(4.0);
    }

    if let Some(i) = remove_index {
        items.remove(i);
    }

    ui.separator();
    ui.label("Add power/skill:");
    let name_id = ui.make_persistent_id("powers_new_name");
    let desc_id = ui.make_persistent_id("powers_new_desc");
    let mut new_name = ui
        .data_mut(|d| d.get_persisted::<String>(name_id))
        .unwrap_or_default();
    let mut new_desc = ui
        .data_mut(|d| d.get_persisted::<String>(desc_id))
        .unwrap_or_default();

    ui.add(egui::TextEdit::singleline(&mut new_name).hint_text("Name"));
    ui.add(
        egui::TextEdit::multiline(&mut new_desc)
            .hint_text("Description")
            .desired_rows(2),
    );

    if ui
        .add_enabled(!player_locked, egui::Button::new("➕"))
        .clicked()
    {
        let name = new_name.trim();
        if !name.is_empty() {
            items.push(PowerEntry {
                name: name.to_string(),
                description: new_desc.trim().to_string(),
                locked: false,
            });
            new_name.clear();
            new_desc.clear();
        }
    }

    ui.data_mut(|d| d.insert_persisted(name_id, new_name));
    ui.data_mut(|d| d.insert_persisted(desc_id, new_desc));
}

fn ensure_skill_tier_names(names: &mut Vec<String>) {
    let defaults = [
        "Novice",
        "Adept",
        "Expert",
        "Master",
        "Grandmaster",
    ];
    if names.len() < 5 {
        for i in names.len()..5 {
            names.push(defaults[i].to_string());
        }
    } else if names.len() > 5 {
        names.truncate(5);
    }
    for (i, name) in names.iter_mut().enumerate() {
        if name.trim().is_empty() {
            *name = defaults[i].to_string();
        }
    }
}
