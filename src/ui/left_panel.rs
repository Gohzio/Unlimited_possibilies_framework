use eframe::egui;

use super::app::{UiState, LeftTab};

pub fn draw_left_panel(ctx: &egui::Context, ui_state: &mut UiState) {
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

            if ui_state.left_tab == LeftTab::Settings {
                ui.label("UI Scale");
                ui.add(egui::Slider::new(&mut ui_state.ui_scale, 0.75..=2.0));
            }
        });
}
