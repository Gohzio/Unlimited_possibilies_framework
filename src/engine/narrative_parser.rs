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

        // [Name] (treat as NPC for common LLM tags like [GUARD], [SMITH])
        if let Some(rest) = line.strip_prefix('[') {
            if let Some((tag, text)) = rest.split_once(']') {
                let tag = tag.trim();
                let text = text.trim();
                if !tag.is_empty()
                    && !text.is_empty()
                    && !tag.eq_ignore_ascii_case("narrator")
                    && !tag.eq_ignore_ascii_case("system")
                {
                    messages.push(Message::Roleplay {
                        speaker: RoleplaySpeaker::Npc,
                        text: format!("{}: {}", tag, text),
                    });
                    continue;
                }
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
