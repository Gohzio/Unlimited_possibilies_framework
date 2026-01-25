use crate::model::message::{Message, RoleplaySpeaker};

pub fn parse_narrative(narrative: &str) -> Vec<Message> {
    let mut messages = Vec::new();

    for line in narrative.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // [NARRATOR]
        if let Some(rest) = line.strip_prefix("[NARRATOR]") {
            messages.push(Message::Roleplay {
                speaker: RoleplaySpeaker::Narrator,
                text: rest.trim().to_string(),
            });
            continue;
        }

        // [NPC: Name]
        if let Some(rest) = line.strip_prefix("[NPC:") {
            if let Some((name, text)) = rest.split_once(']') {
                messages.push(Message::Roleplay {
                    speaker: RoleplaySpeaker::Npc,
                    text: format!("{}: {}", name.trim(), text.trim()),
                });
                continue;
            }
        }

        // [PARTY: Name]
        if let Some(rest) = line.strip_prefix("[PARTY:") {
            if let Some((name, text)) = rest.split_once(']') {
                messages.push(Message::Roleplay {
                    speaker: RoleplaySpeaker::PartyMember,
                    text: format!("{}: {}", name.trim(), text.trim()),
                });
                continue;
            }
        }

        // Fallback
        messages.push(Message::Roleplay {
            speaker: RoleplaySpeaker::Narrator,
            text: line.to_string(),
        });
    }

    messages
}
