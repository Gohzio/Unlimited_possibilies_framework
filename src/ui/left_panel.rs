use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::ui::app::{PartyMember, UiState};

pub fn draw_left_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    _cmd_tx: &Sender<EngineCommand>,
) {
    egui::SidePanel::left("left")
        .resizable(false)
        .default_width(180.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_party(ui, ui_state);
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

            ui.label("Role");
            ui.text_edit_singleline(&mut member.role);

            ui.label("Notes");
            ui.text_edit_multiline(&mut member.notes);
        });

        ui.add_space(6.0);
    }

    if let Some(i) = remove_index {
        state.party.remove(i);
    }
}
