use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::ui::app::{LeftTab, PartyMember, UiState};
use std::collections::HashMap;

pub fn draw_left_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    cmd_tx: &Sender<EngineCommand>,
) {
    egui::SidePanel::left("left")
        .resizable(false)
        .default_width(180.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui_state.is_left_tab_visible(LeftTab::Party) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Party, "Party");
                }
                if ui_state.is_left_tab_visible(LeftTab::Npcs) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Npcs, "NPCs");
                }
                if ui_state.is_left_tab_visible(LeftTab::Quests) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Quests, "Quests");
                }
                if ui_state.is_left_tab_visible(LeftTab::Factions) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Factions, "Factions");
                }
                if ui_state.is_left_tab_visible(LeftTab::Slaves) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Slaves, "Slaves");
                }
                if ui_state.is_left_tab_visible(LeftTab::Property) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Property, "Property");
                }
                if ui_state.is_left_tab_visible(LeftTab::BondedServants) {
                    let label = bonded_servants_label(ui_state).to_string();
                    ui.selectable_value(
                        &mut ui_state.left_tab,
                        LeftTab::BondedServants,
                        label,
                    );
                }
                if ui_state.is_left_tab_visible(LeftTab::Concubines) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Concubines, "Concubines");
                }
                if ui_state.is_left_tab_visible(LeftTab::HaremMembers) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::HaremMembers, "Harem");
                }
                if ui_state.is_left_tab_visible(LeftTab::Prisoners) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::Prisoners, "Prisoners");
                }
                if ui_state.is_left_tab_visible(LeftTab::NpcsOnMission) {
                    ui.selectable_value(&mut ui_state.left_tab, LeftTab::NpcsOnMission, "Missions");
                }
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| match ui_state.left_tab {
                LeftTab::Party => draw_party(ui, ui_state, cmd_tx),
                LeftTab::Npcs => draw_local_npcs(ui, ui_state, cmd_tx),
                LeftTab::Quests => draw_quests(ui, ui_state),
                LeftTab::Factions => draw_factions(ui, ui_state),
                LeftTab::Slaves => draw_section_cards(ui, ui_state, "slaves", "Slaves"),
                LeftTab::Property => draw_section_cards(ui, ui_state, "property", "Property"),
                LeftTab::BondedServants => {
                    let label = bonded_servants_label(ui_state).to_string();
                    draw_section_cards(ui, ui_state, "bonded_servants", &label)
                }
                LeftTab::Concubines => {
                    draw_section_cards(ui, ui_state, "concubines", "Concubines")
                }
                LeftTab::HaremMembers => {
                    draw_section_cards(ui, ui_state, "harem_members", "Harem Members")
                }
                LeftTab::Prisoners => draw_section_cards(ui, ui_state, "prisoners", "Prisoners"),
                LeftTab::NpcsOnMission => {
                    draw_section_cards(ui, ui_state, "npcs_on_mission", "NPCs on Mission")
                }
            });
        });
}

/* =========================
   Party UI
   ========================= */

fn draw_party(ui: &mut egui::Ui, state: &mut UiState, cmd_tx: &Sender<EngineCommand>) {
    ui.heading("Party");

    if ui.button("➕ Add Member").clicked() {
        let _ = cmd_tx.send(EngineCommand::AddPartyMember {
            name: "New Member".to_string(),
            role: "Unknown".to_string(),
            details: String::new(),
            weapons: Vec::new(),
            armor: Vec::new(),
            clothing: Vec::new(),
        });
    }

    ui.separator();

    let mut remove_index: Option<usize> = None;

    for (i, member) in state.party.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Member {}", i + 1));
                if ui.small_button("❌").clicked() {
                    if let Some(id) = member.id.as_ref() {
                        let _ = cmd_tx.send(EngineCommand::RemovePartyMember { id: id.clone() });
                    } else {
                        remove_index = Some(i);
                    }
                }
            });

            ui.label("Name");
            let mut lock_changed = false;
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut member.name);
                if ui.checkbox(&mut member.lock_name, "Lock").changed() {
                    lock_changed = true;
                }
            });

            ui.label("Role/Class");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut member.role);
                if ui.checkbox(&mut member.lock_role, "Lock").changed() {
                    lock_changed = true;
                }
            });

            ui.label("Details");
            ui.horizontal(|ui| {
                ui.text_edit_multiline(&mut member.details);
                if ui.checkbox(&mut member.lock_details, "Lock").changed() {
                    lock_changed = true;
                }
            });

            ui.label("Weapons");
            ui.horizontal(|ui| {
                editable_list_with_id(ui, &mut member.weapons, ("party_weapons", i));
                if ui.checkbox(&mut member.lock_weapons, "Lock").changed() {
                    lock_changed = true;
                }
            });

            ui.label("Armour");
            ui.horizontal(|ui| {
                editable_list_with_id(ui, &mut member.armor, ("party_armor", i));
                if ui.checkbox(&mut member.lock_armor, "Lock").changed() {
                    lock_changed = true;
                }
            });

            ui.label("Clothing");
            ui.horizontal(|ui| {
                editable_list_with_id(ui, &mut member.clothing, ("party_clothing", i));
                if ui.checkbox(&mut member.lock_clothing, "Lock").changed() {
                    lock_changed = true;
                }
            });

            if let Some(id) = member.id.as_ref() {
                if ui.button("Apply changes").clicked() {
                    let _ = cmd_tx.send(EngineCommand::SetPartyMember {
                        id: id.clone(),
                        name: member.name.clone(),
                        role: member.role.clone(),
                        details: member.details.clone(),
                        weapons: member.weapons.clone(),
                        armor: member.armor.clone(),
                        clothing: member.clothing.clone(),
                    });
                }
            }

            if lock_changed {
                if let Some(id) = member.id.as_ref() {
                    let _ = cmd_tx.send(EngineCommand::SetPartyMemberLocks {
                        id: id.clone(),
                        lock_name: member.lock_name,
                        lock_role: member.lock_role,
                        lock_details: member.lock_details,
                        lock_weapons: member.lock_weapons,
                        lock_armor: member.lock_armor,
                        lock_clothing: member.lock_clothing,
                    });
                } else {
                    state.ui_error = Some("Locks can only be saved after the member exists in the engine.".to_string());
                }
            }
        });

        ui.add_space(6.0);
    }

    if let Some(i) = remove_index {
        state.party.remove(i);
    }
}

/* =========================
   NPC UI
   ========================= */

#[derive(Clone)]
struct LocalNpc {
    id: String,
    name: String,
    role: String,
    notes: String,
}

fn draw_local_npcs(
    ui: &mut egui::Ui,
    state: &mut UiState,
    cmd_tx: &Sender<EngineCommand>,
) {
    ui.heading("Local NPCs");

    let mut npcs = collect_local_npcs(state);
    npcs.sort_by(|a, b| a.name.cmp(&b.name));

    if npcs.is_empty() {
        ui.label("No known NPCs yet.");
        return;
    }

            for npc in npcs {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(&npc.name);
                        if ui.small_button("➕ Add to Party").clicked() {
                            if !state
                                .party
                                .iter()
                                .any(|m| m.name.eq_ignore_ascii_case(&npc.name))
                            {
                                state.party.push(PartyMember {
                                    id: Some(npc.id.clone()),
                                    name: npc.name.clone(),
                                    role: npc.role.clone(),
                                    details: npc.notes.clone(),
                                    weapons: Vec::new(),
                                    armor: Vec::new(),
                                    clothing: Vec::new(),
                                    lock_name: false,
                                    lock_role: false,
                                    lock_details: false,
                                    lock_weapons: false,
                                    lock_armor: false,
                                    lock_clothing: false,
                                });
                            }
                            let _ = cmd_tx.send(EngineCommand::AddNpcToParty {
                                id: npc.id.clone(),
                                name: npc.name.clone(),
                                role: npc.role.clone(),
                                details: npc.notes.clone(),
                            });
                            state.left_tab = LeftTab::Party;
                        }
                    });

                    if !npc.role.is_empty() {
                        ui.label(format!("Role: {}", npc.role));
                    }
                    if !npc.notes.is_empty() {
                        ui.label(format!("Notes: {}", npc.notes));
                    }
        });

        ui.add_space(6.0);
    }
}

fn collect_local_npcs(state: &UiState) -> Vec<LocalNpc> {
    let mut map: HashMap<String, LocalNpc> = HashMap::new();

    if let Some(snapshot) = &state.snapshot {
        for npc in &snapshot.npcs {
            if !npc.nearby {
                continue;
            }
            map.insert(
                npc.id.clone(),
                LocalNpc {
                    id: npc.id.clone(),
                    name: npc.name.clone(),
                    role: npc.role.clone(),
                    notes: npc.notes.clone(),
                },
            );
        }
    }

    map.into_values().collect()
}

/* =========================
   Quest UI
   ========================= */

fn draw_quests(ui: &mut egui::Ui, state: &UiState) {
    ui.heading("Quests");
    ui.set_width(ui.available_width());

    let Some(snapshot) = &state.snapshot else {
        ui.label("No quests yet.");
        return;
    };

    if snapshot.quests.is_empty() {
        ui.label("No quests yet.");
        return;
    }

    let mut quests = snapshot.quests.clone();
    quests.sort_by(|a, b| a.title.cmp(&b.title));

    for quest in quests {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Label::new(&quest.title).wrap());
                ui.add_space(4.0);
                ui.add(egui::Label::new(format!("({})", quest_status_label(&quest.status))).wrap());
            });

            if !quest.description.trim().is_empty() {
                ui.add(egui::Label::new(&quest.description).wrap());
            }
            if let Some(diff) = &quest.difficulty {
                let trimmed = diff.trim();
                if !trimmed.is_empty() {
                    ui.add(egui::Label::new(format!("Difficulty: {}", trimmed)).wrap());
                }
            }
            if quest.negotiable {
                ui.add(egui::Label::new("Negotiable rewards: yes").wrap());
            }
            if !quest.reward_options.is_empty() {
                ui.add(egui::Label::new("Reward options:").wrap());
                for opt in &quest.reward_options {
                    ui.add(egui::Label::new(format!("- {}", opt)).wrap());
                }
            }

            if !quest.rewards.is_empty() {
                ui.add(egui::Label::new("Rewards:").wrap());
                for reward in &quest.rewards {
                    ui.add(egui::Label::new(format!("- {}", reward)).wrap());
                }
            }

            if !quest.sub_quests.is_empty() {
                ui.add(egui::Label::new("Sub-quests:").wrap());
                for step in &quest.sub_quests {
                    let mut completed = step.completed;
                    ui.add_enabled(
                        false,
                        egui::Checkbox::new(&mut completed, step.description.as_str()),
                    );
                }
            }
        });

        ui.add_space(6.0);
    }
}

fn draw_factions(ui: &mut egui::Ui, state: &UiState) {
    ui.heading("Factions");
    ui.set_width(ui.available_width());

    let Some(snapshot) = &state.snapshot else {
        ui.label("No factions yet.");
        return;
    };

    if snapshot.factions.is_empty() {
        ui.label("No factions yet.");
        return;
    }

    let mut factions = snapshot.factions.clone();
    factions.sort_by(|a, b| a.name.cmp(&b.name));

    for faction in factions {
        ui.group(|ui| {
            let kind = faction.kind.as_deref().unwrap_or("unknown");
            ui.label(format!("{} ({})", faction.name, kind));
            ui.label(format!("Reputation: {}", faction.reputation));
            if let Some(desc) = &faction.description {
                let trimmed = desc.trim();
                if !trimmed.is_empty() {
                    ui.add(egui::Label::new(trimmed).wrap());
                }
            }
        });
        ui.add_space(6.0);
    }
}

fn draw_section_cards(ui: &mut egui::Ui, state: &UiState, section: &str, title: &str) {
    ui.heading(title);
    ui.set_width(ui.available_width());

    let Some(snapshot) = &state.snapshot else {
        ui.label("No data yet.");
        return;
    };

    let Some(cards) = snapshot.sections.get(section) else {
        ui.label("No data yet.");
        return;
    };

    if cards.is_empty() {
        ui.label("No data yet.");
        return;
    }

    for card in cards {
        ui.group(|ui| {
            ui.label(&card.name);
            if !card.role.trim().is_empty() {
                ui.label(format!("Role: {}", card.role.trim()));
            }
            if !card.status.trim().is_empty() {
                ui.label(format!("Status: {}", card.status.trim()));
            }
            if !card.details.trim().is_empty() {
                ui.add(egui::Label::new(card.details.trim()).wrap());
            }
            if !card.notes.trim().is_empty() {
                ui.add(egui::Label::new(format!("Notes: {}", card.notes.trim())).wrap());
            }
            if !card.tags.is_empty() {
                ui.label("Tags:");
                for tag in &card.tags {
                    ui.label(format!("- {}", tag));
                }
            }
            if !card.items.is_empty() {
                ui.label("Items:");
                for item in &card.items {
                    ui.label(format!("- {}", item));
                }
            }
        });
        ui.add_space(6.0);
    }
}

fn bonded_servants_label(state: &UiState) -> &str {
    let label = state.optional_tabs.bonded_servants_label.trim();
    if label.is_empty() {
        "Bonded"
    } else {
        label
    }
}

fn quest_status_label(status: &crate::model::game_state::QuestStatus) -> &'static str {
    match status {
        crate::model::game_state::QuestStatus::Active => "active",
        crate::model::game_state::QuestStatus::Completed => "completed",
        crate::model::game_state::QuestStatus::Failed => "failed",
    }
}

fn editable_list_with_id<T: std::hash::Hash>(
    ui: &mut egui::Ui,
    items: &mut Vec<String>,
    id_key: T,
) {
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
        let id = ui.make_persistent_id(id_key);
        let mut new_item = ui
            .data_mut(|d| d.get_persisted::<String>(id))
            .unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut new_item).hint_text("Add clothing item"));
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
