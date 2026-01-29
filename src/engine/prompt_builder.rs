use crate::model::game_context::GameContext;
use crate::model::message::{Message, RoleplaySpeaker};

/// Builds the full prompt sent to the LLM.
/// This struct is intentionally dumb: it only formats text.
/// No parsing, no networking, no engine logic.
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(context: &GameContext, player_input: &str) -> String {
        let mut prompt = String::new();

        /* =========================
           SYSTEM PROMPT
           ========================= */

        prompt.push_str(
            "You are the narrator and all non-player characters in a roleplaying game.\n\n\
Rules:\n\
- when loot appears in the world, you MUST use a DROP event to represent it.\n\
- Do not use add_item unless the player explicitly picks up an item.\n\
- You must never control or describe actions taken by the player beyond what the player explicitly states.\n\
- You must never change game state directly.\n\
- All game state changes must be expressed ONLY through structured EVENTS.\n\
- If no state change is required, output an empty events array.\n\n\
Narrative Rules:\n\
- Write immersive narration and dialogue.\n\
- Use explicit speaker tags for every narrative block.\n\
- Never invent party members.\n\
- Never speak as the player character.\n\n\
Output Format:\n\
You MUST respond in exactly two sections:\n\n\
NARRATIVE:\n\
<text>\n\n\
EVENTS:\n\
<json>\n\n\
Do not add explanations, markdown, or extra sections.\n\n\
Event Types (JSON array of objects with a \"type\" field):\n\
- combat { description }\n\
- dialogue { speaker, text }\n\
- travel { from, to }\n\
- rest { description }\n\
- grant_power { id, name, description }\n\
- modify_stat { stat_id, delta }\n\
- start_quest { id, title, description }\n\
- set_flag { flag }\n\
- add_party_member { id, name, role }\n\
- npc_spawn { id, name, role, details? }\n\
- npc_join_party { id, name?, role?, details? }\n\
- npc_leave_party { id }\n\
- relationship_change { subject_id, target_id, delta }\n\
- add_item { item_id, quantity }\n\
- drop { item, quantity?, description? }\n\
- spawn_loot { item, quantity?, description? }\n\
- currency_change { currency, delta }\n\n"
        );

        /* =========================
           WORLD
           ========================= */

        prompt.push_str("WORLD DEFINITION\n");
        prompt.push_str(&format!("Title: {}\n", context.world.title));
        prompt.push_str(&format!("Author: {}\n\n", context.world.author));

        prompt.push_str("Description:\n");
        prompt.push_str(&context.world.description);
        prompt.push_str("\n\n");

        if !context.world.themes.is_empty() {
            prompt.push_str("Themes:\n");
            for theme in &context.world.themes {
                prompt.push_str(&format!("- {}\n", theme));
            }
            prompt.push('\n');
        }

        if !context.world.tone.is_empty() {
            prompt.push_str("Tone:\n");
            for tone in &context.world.tone {
                prompt.push_str(&format!("- {}\n", tone));
            }
            prompt.push('\n');
        }

        prompt.push_str("Narration Rules:\n");
        prompt.push_str(&context.world.narrator_role);
        prompt.push_str("\n\n");

        if !context.world.style_guidelines.is_empty() {
            prompt.push_str("Style Guidelines:\n");
            for rule in &context.world.style_guidelines {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }

        if !context.world.must_not.is_empty() {
            prompt.push_str("Must NOT:\n");
            for rule in &context.world.must_not {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }

        if !context.world.must_always.is_empty() {
            prompt.push_str("Must ALWAYS:\n");
            for rule in &context.world.must_always {
                prompt.push_str(&format!("- {}\n", rule));
            }
            prompt.push('\n');
        }

        /* =========================
           PLAYER
           ========================= */

        prompt.push_str("PLAYER CHARACTER:\n");
        prompt.push_str(&format!("Name: {}\n", context.player.name));
        prompt.push_str(&format!("Class: {}\n", context.player.class));
        prompt.push_str("Background:\n");
        prompt.push_str(&context.player.background);
        prompt.push_str("\n\n");

        if !context.player.stats.is_empty() {
            prompt.push_str("Stats:\n");
            for (k, v) in &context.player.stats {
                prompt.push_str(&format!("- {}: {}\n", k, v));
            }
            prompt.push('\n');
        }

        if !context.player.powers.is_empty() {
            prompt.push_str("Powers:\n");
            for p in &context.player.powers {
                prompt.push_str(&format!("- {}\n", p));
            }
            prompt.push('\n');
        }

        /* =========================
           PARTY
           ========================= */

        prompt.push_str("PARTY MEMBERS:\n");
        if context.party.is_empty() {
            prompt.push_str("None\n");
        } else {
            for member in &context.party {
                prompt.push_str(&format!(
                    "- [PARTY: {}] Role: {}\n  Details: {}\n",
                    member.name,
                    member.role,
                    member.details
                ));
            }
        }
        prompt.push('\n');

        /* =========================
           NARRATIVE HISTORY
           ========================= */

        prompt.push_str("NARRATIVE HISTORY:\n");

        for msg in &context.history {
            if let Message::Roleplay { speaker, text } = msg {
                match speaker {
                    RoleplaySpeaker::Narrator => {
                        prompt.push_str(&format!("[NARRATOR] {}\n", text));
                    }
                    RoleplaySpeaker::Npc => {
                        if let Some((name, body)) = split_speaker_text(text) {
                            prompt.push_str(&format!("[NPC: {}] {}\n", name, body));
                        } else {
                            prompt.push_str(&format!("[NPC] {}\n", text));
                        }
                    }
                    RoleplaySpeaker::PartyMember => {
                        if let Some((name, body)) = split_speaker_text(text) {
                            prompt.push_str(&format!("[PARTY: {}] {}\n", name, body));
                        } else {
                            prompt.push_str(&format!("[PARTY] {}\n", text));
                        }
                    }
                }
            }
        }

        prompt.push('\n');

        /* =========================
           CURRENT SITUATION
           ========================= */

        prompt.push_str("CURRENT SITUATION:\n");
        if context.snapshot.is_some() {
            prompt.push_str("The world state is tracked internally by the engine.\n");
        } else {
            prompt.push_str("The adventure has just begun.\n");
        }
        prompt.push_str("\n\n");

        /* =========================
           PLAYER INPUT
           ========================= */

        prompt.push_str("PLAYER ACTION:\n");
        prompt.push_str(player_input);
        prompt.push_str("\n\n");

        /* =========================
           REMINDER
           ========================= */

        prompt.push_str(
            "REMINDER:\n\
- Use speaker tags like [NARRATOR], [PARTY: Name], [NPC: Name]\n\
- Do NOT describe player actions beyond the input.\n\
- EVENTS must be valid JSON.\n\
- If no events occur, output: []\n"
        );

        prompt
    }
}

fn split_speaker_text(text: &str) -> Option<(&str, &str)> {
    let (name, body) = text.split_once(':')?;
    let name = name.trim();
    let body = body.trim();
    if name.is_empty() || body.is_empty() {
        return None;
    }
    Some((name, body))
}
