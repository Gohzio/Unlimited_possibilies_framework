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
-<json>\n\n\
Do not add explanations, markdown, or extra sections.\n\n\
Event Types (JSON array of objects with a \"type\" field):\n\
- combat { description }\n\
- dialogue { speaker, text }\n\
- travel { from, to }\n\
- rest { description }\n\
- grant_power { id, name, description }\n\
- modify_stat { stat_id, delta }\n\
- start_quest { id, title, description, rewards?, sub_quests?, declinable? }\n\
- update_quest { id, title?, description?, status?, rewards?, sub_quests? }\n\
- set_flag { flag }\n\
- add_party_member { id, name, role }\n\
- npc_spawn { id, name, role, details? }\n\
- npc_join_party { id, name?, role?, details? }\n\
- npc_leave_party { id }\n\
- relationship_change { subject_id, target_id, delta }\n\
- add_item { item_id, quantity }\n\
- drop { item, quantity?, description? }\n\
- spawn_loot { item, quantity?, description? }\n\
- currency_change { currency, delta }\n\
- request_context { topics }\n\n"
        );
        prompt.push_str(
            "Event Notes:\n\
- sub_quests is an array of objects like { id, description, completed? }\n\
- start_quest should include rewards (can be empty) and may include declinable for world quests\n\
- update_quest may send partial updates for sub_quests (id required)\n\n"
        );
        prompt.push_str(
            "Request Context:\n\
- If you need more data, emit request_context { topics: [\"topic1\", \"topic2\"] }\n\
- Do NOT add narrative when requesting context\n\n"
        );
        prompt.push_str(
            "Optional Tabs (unlock via set_flag):\n\
- unlock:slaves\n\
- unlock:property\n\
- unlock:bonded_servants (aliases: bonded_servants, hirð)\n\
- unlock:concubines\n\
- unlock:harem_members\n\
- unlock:prisoners\n\
- unlock:npcs_on_mission\n\n"
        );

        prompt.push_str("Quest Rules:\n");
        if context.world.world_quests_enabled {
            prompt.push_str("- World quests are ENABLED.\n");
            prompt.push_str(
                "- When the world offers a quest, include the exact line: \"*ding* the world is offering you a quest.\"\n",
            );
            if context.world.world_quests_mandatory {
                prompt.push_str(
                    "- If the world quest is mandatory, set declinable: false and you may emit start_quest immediately.\n",
                );
            } else {
                prompt.push_str(
                    "- Do NOT use declinable: false unless mandatory world quests are enabled.\n",
                );
            }
            prompt.push_str(
                "- For declinable world quests, emit start_quest ONLY after the player explicitly accepts.\n",
            );
        } else {
            prompt.push_str("- World quests are DISABLED.\n");
        }
        if context.world.npc_quests_enabled {
            prompt.push_str("- NPC quests are ENABLED.\n");
            prompt.push_str(
                "- NPCs must explicitly say: \"I hereby offer you a quest.\" when offering.\n",
            );
            prompt.push_str(
                "- Emit start_quest ONLY after the player explicitly accepts.\n",
            );
            prompt.push_str(
                "- start_quest must include a title and rewards (can be an empty array).\n",
            );
        } else {
            prompt.push_str("- NPC quests are DISABLED.\n");
        }
        prompt.push('\n');

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

        prompt.push_str("Loot Rules:\n");
        prompt.push_str(&loot_rules_text(&context.world));
        prompt.push('\n');

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

        if !context.player.weapons.is_empty() {
            prompt.push_str("Weapons:\n");
            for item in &context.player.weapons {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }

        if !context.player.armor.is_empty() {
            prompt.push_str("Armour:\n");
            for item in &context.player.armor {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }

        if !context.player.clothing.is_empty() {
            prompt.push_str("Clothing:\n");
            for item in &context.player.clothing {
                prompt.push_str(&format!("- {}\n", item));
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
                if !member.clothing.is_empty() {
                    prompt.push_str("  Clothing:\n");
                    for item in &member.clothing {
                        prompt.push_str(&format!("  - {}\n", item));
                    }
                }
            }
        }
        prompt.push('\n');

        /* =========================
           QUESTS
           ========================= */

        if let Some(snapshot) = &context.snapshot {
            prompt.push_str("QUESTS:\n");
            if snapshot.quests.is_empty() {
                prompt.push_str("None\n\n");
            } else {
                for quest in &snapshot.quests {
                    prompt.push_str(&format!(
                        "- [{}] {}\n",
                        quest_status_label(&quest.status),
                        quest.title
                    ));
                    if !quest.description.trim().is_empty() {
                        prompt.push_str(&format!("  Description: {}\n", quest.description.trim()));
                    }
                    if !quest.rewards.is_empty() {
                        prompt.push_str("  Rewards:\n");
                        for reward in &quest.rewards {
                            prompt.push_str(&format!("  - {}\n", reward));
                        }
                    }
                    if !quest.sub_quests.is_empty() {
                        prompt.push_str("  Sub-quests:\n");
                        for step in &quest.sub_quests {
                            let status = if step.completed { "done" } else { "open" };
                            prompt.push_str(&format!("  - [{}] {}\n", status, step.description));
                        }
                    }
                }
                prompt.push('\n');
            }
        }

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
- EVENTS must be valid JSON (a JSON array only).\n\
- Do NOT use bullet lists or \"type { key: value }\" shorthand.\n\
- All keys and string values must use double quotes.\n\
- Example:\n\
  [ { \"type\": \"drop\", \"item\": \"Common Squirrel Fur\", \"quantity\": 1, \"description\": \"The soft fur of a common forest squirrel.\" } ]\n\
- If no events occur, output: []\n"
        );

        prompt
    }
}

impl PromptBuilder {
    pub fn build_with_requested_context(
        context: &GameContext,
        player_input: &str,
        requested_context: &str,
        recent_history: &[Message],
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(
            "You are the narrator and all non-player characters in a roleplaying game.\n\n\
Rules:\n\
- You must never control or describe actions taken by the player beyond what the player explicitly states.\n\
- All game state changes must be expressed ONLY through structured EVENTS.\n\
- If no state change is required, output an empty events array.\n\n\
Output Format:\n\
You MUST respond in exactly two sections:\n\n\
NARRATIVE:\n\
<text>\n\n\
EVENTS:\n\
<json array>\n\n\
Event Types (JSON array of objects with a \"type\" field):\n\
- combat { description }\n\
- dialogue { speaker, text }\n\
- travel { from, to }\n\
- rest { description }\n\
- grant_power { id, name, description }\n\
- modify_stat { stat_id, delta }\n\
- start_quest { id, title, description, rewards?, sub_quests?, declinable? }\n\
- update_quest { id, title?, description?, status?, rewards?, sub_quests? }\n\
- set_flag { flag }\n\
- add_party_member { id, name, role }\n\
- npc_spawn { id, name, role, details? }\n\
- npc_join_party { id, name?, role?, details? }\n\
- npc_leave_party { id }\n\
- relationship_change { subject_id, target_id, delta }\n\
- add_item { item_id, quantity }\n\
- drop { item, quantity?, description? }\n\
- spawn_loot { item, quantity?, description? }\n\
- currency_change { currency, delta }\n\
- request_context { topics }\n\n"
        );

        prompt.push_str("Quest Rules:\n");
        if context.world.world_quests_enabled {
            prompt.push_str("- World quests are ENABLED.\n");
            prompt.push_str(
                "- When the world offers a quest, include the exact line: \"*ding* the world is offering you a quest.\"\n",
            );
            if context.world.world_quests_mandatory {
                prompt.push_str(
                    "- If the world quest is mandatory, set declinable: false and you may emit start_quest immediately.\n",
                );
            } else {
                prompt.push_str(
                    "- Do NOT use declinable: false unless mandatory world quests are enabled.\n",
                );
            }
            prompt.push_str(
                "- For declinable world quests, emit start_quest ONLY after the player explicitly accepts.\n",
            );
        } else {
            prompt.push_str("- World quests are DISABLED.\n");
        }
        if context.world.npc_quests_enabled {
            prompt.push_str("- NPC quests are ENABLED.\n");
            prompt.push_str(
                "- NPCs must explicitly say: \"I hereby offer you a quest.\" when offering.\n",
            );
            prompt.push_str(
                "- Emit start_quest ONLY after the player explicitly accepts.\n",
            );
            prompt.push_str(
                "- start_quest must include a title and rewards (can be an empty array).\n",
            );
        } else {
            prompt.push_str("- NPC quests are DISABLED.\n");
        }
        prompt.push('\n');

        prompt.push_str("WORLD TITLE:\n");
        prompt.push_str(&format!("{}\n\n", context.world.title));

        if !requested_context.trim().is_empty() {
            prompt.push_str("REQUESTED CONTEXT:\n");
            prompt.push_str(requested_context);
            prompt.push_str("\n\n");
        }

        if !recent_history.is_empty() {
            prompt.push_str("RECENT HISTORY:\n");
            for msg in recent_history {
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
        }

        prompt.push_str("PLAYER ACTION:\n");
        prompt.push_str(player_input);
        prompt.push_str("\n\n");

        prompt.push_str(
            "REMINDER:\n\
- Use speaker tags like [NARRATOR], [PARTY: Name], [NPC: Name]\n\
- Do NOT describe player actions beyond the input.\n\
- EVENTS must be valid JSON (a JSON array only).\n\
- All keys and string values must use double quotes.\n\
- If you still need more context, emit request_context with topics.\n\
"
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

fn loot_rules_text(world: &crate::ui::app::WorldDefinition) -> String {
    let mode = world.loot_rules_mode.trim();
    let mut base = if mode.eq_ignore_ascii_case("difficulty based") {
        "Difficulty based: Harder tasks yield better rewards.".to_string()
    } else if mode.eq_ignore_ascii_case("rarity based") {
        "Rarity based: Each drop can roll from any tier (Common, Uncommon, Rare, Legendary, Exotic, Godly).".to_string()
    } else if !world.loot_rules_custom.trim().is_empty() {
        format!("Custom: {}", world.loot_rules_custom.trim())
    } else {
        "Custom: (not specified)".to_string()
    };
    base.push_str(" Applies to activity rewards (Mining, Fishing, Woodcutting, Farming, Crafting).");
    base
}

fn quest_status_label(status: &crate::model::game_state::QuestStatus) -> &'static str {
    match status {
        crate::model::game_state::QuestStatus::Active => "active",
        crate::model::game_state::QuestStatus::Completed => "completed",
        crate::model::game_state::QuestStatus::Failed => "failed",
    }
}
