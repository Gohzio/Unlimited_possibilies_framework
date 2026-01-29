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
                ui.selectable_value(&mut ui_state.left_tab, LeftTab::Party, "Party");
                ui.selectable_value(&mut ui_state.left_tab, LeftTab::Npcs, "NPCs");
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| match ui_state.left_tab {
                LeftTab::Party => draw_party(ui, ui_state),
                LeftTab::Npcs => draw_local_npcs(ui, ui_state, cmd_tx),
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
