use eframe::egui;
use std::sync::mpsc::Sender;

use crate::engine::protocol::EngineCommand;
use crate::ui::app::{PartyMember, UiState, LeftTab};

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
                ui.selectable_value(&mut ui_state.left_tab, LeftTab::Settings, "Settings");
                ui.selectable_value(&mut ui_state.left_tab, LeftTab::Party, "Party");
                ui.selectable_value(&mut ui_state.left_tab, LeftTab::Options, "Options");
            });

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                match ui_state.left_tab {
                    LeftTab::Settings => {
                        ui.label("UI Scale");
                        ui.add(
                            egui::Slider::new(&mut ui_state.ui_scale, 0.75..=2.0)
                                .text("Scale"),
                        );
                    }

                    LeftTab::Party => {
                        draw_party(ui, ui_state);
                    }

                    LeftTab::Options => {
                        if ui.button("🔌 Connect to LM Studio").clicked() {
                            let _ = cmd_tx.send(EngineCommand::ConnectToLlm);
                        }

                        ui.add_space(6.0);

                        let status_color = if ui_state.llm_connected {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::RED
                        };

                        ui.label(
                            egui::RichText::new(&ui_state.llm_status)
                                .color(status_color),
                        );
                    }
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
