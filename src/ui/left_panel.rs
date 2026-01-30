use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::model::message::{Message, RoleplaySpeaker};
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
                LeftTab::Party => draw_party(ui, ui_state),
                LeftTab::Npcs => draw_local_npcs(ui, ui_state, cmd_tx),
                LeftTab::Quests => draw_quests(ui, ui_state),
                LeftTab::Slaves => draw_placeholder(ui, "Slaves", "No slaves tracked yet."),
                LeftTab::Property => draw_placeholder(ui, "Property", "No property tracked yet."),
                LeftTab::BondedServants => {
                    draw_placeholder(ui, "Bonded Servants", "No bonded servants tracked yet.")
                }
                LeftTab::Concubines => {
                    draw_placeholder(ui, "Concubines", "No concubines tracked yet.")
                }
                LeftTab::HaremMembers => {
                    draw_placeholder(ui, "Harem Members", "No harem members tracked yet.")
                }
                LeftTab::Prisoners => draw_placeholder(ui, "Prisoners", "No prisoners tracked yet."),
                LeftTab::NpcsOnMission => {
                    draw_placeholder(ui, "NPCs on Mission", "No missions tracked yet.")
                }
            });
        });
}

/* =========================
   Party UI
   ========================= */

fn draw_party(ui: &mut egui::Ui, state: &mut UiState) {
    ui.heading("Party");

    if ui.button("➕ Add Member").clicked() {
        state.party.push(PartyMember::default());
    }

    ui.separator();

    let mut remove_index: Option<usize> = None;

    for (i, member) in state.party.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Member {}", i + 1));
                if ui.small_button("❌").clicked() {
                    remove_index = Some(i);
                }
            });

            ui.label("Name");
            ui.text_edit_singleline(&mut member.name);

            ui.label("Role/Class");
            ui.text_edit_singleline(&mut member.role);

            ui.label("Details");
            ui.text_edit_multiline(&mut member.details);

            ui.label("Clothing");
            editable_list_with_id(ui, &mut member.clothing, ("party_clothing", i));
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
                                    clothing: Vec::new(),
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

    for msg in &state.rendered_messages {
        let Message::Roleplay { speaker, text } = msg else { continue };
        if !matches!(speaker, RoleplaySpeaker::Npc) {
            continue;
        }
        let Some((name, _body)) = text.split_once(':') else { continue };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let id = npc_id_from_name(name);
        map.entry(id.clone()).or_insert(LocalNpc {
            id,
            name: name.to_string(),
            role: "Unknown".to_string(),
            notes: String::new(),
        });
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

fn draw_placeholder(ui: &mut egui::Ui, title: &str, message: &str) {
    ui.heading(title);
    ui.label(message);
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

fn npc_id_from_name(name: &str) -> String {
    let mut id = String::from("npc_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    id.trim_end_matches('_').to_string()
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
