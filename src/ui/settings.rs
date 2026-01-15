use serde::{Deserialize, Serialize};
use egui::Color32;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct UiSettings {
    pub ui_scale: f32,

    // Speaker → color mapping (extensible)
    pub speaker_colors: HashMap<String, [u8; 4]>,
}

impl Default for UiSettings {
    fn default() -> Self {
        let mut speaker_colors = HashMap::new();

        speaker_colors.insert("User".into(), [40, 70, 120, 255]);
        speaker_colors.insert("Narrator".into(), [40, 90, 60, 255]);
        speaker_colors.insert("PartyMember".into(), [90, 60, 120, 255]);
        speaker_colors.insert("Npc".into(), [120, 80, 40, 255]);
        speaker_colors.insert("System".into(), [80, 80, 80, 255]);

        Self {
            ui_scale: 1.0,
            speaker_colors,
        }
    }
}

impl UiSettings {
    pub fn color(&self, key: &str) -> Color32 {
        self.speaker_colors
            .get(key)
            .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
            .unwrap_or(Color32::WHITE)
    }

    pub fn set_color(&mut self, key: &str, color: Color32) {
        self.speaker_colors.insert(
            key.to_string(),
            [color.r(), color.g(), color.b(), color.a()],
        );
    }
}
