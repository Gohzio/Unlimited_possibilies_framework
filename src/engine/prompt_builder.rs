use crate::model::game_context::GameContext;
use crate::model::message::{Message, RoleplaySpeaker};

/// Builds the full prompt sent to the LLM.
/// This struct is intentionally dumb: it only formats text.
/// No parsing, no networking, no engine logic.
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(context: &GameContext, player_input: &str) -> String {
        if context.world.is_rpg_world {
            GamePromptBuilder::build(context, player_input)
        } else {
            FreeformPromptBuilder::build(context, player_input)
        }
    }

    pub fn build_with_requested_context(
        context: &GameContext,
        player_input: &str,
        requested_context: &str,
        recent_history: &[Message],
    ) -> String {
        if context.world.is_rpg_world {
            GamePromptBuilder::build_with_requested_context(
                context,
                player_input,
                requested_context,
                recent_history,
            )
        } else {
            FreeformPromptBuilder::build_with_requested_context(
                context,
                player_input,
                requested_context,
                recent_history,
            )
        }
    }
}

struct GamePromptBuilder;

impl GamePromptBuilder {
    pub fn build(context: &GameContext, player_input: &str) -> String {
        let mut prompt = String::new();

        push_game_system_prompt(&mut prompt, context, false);
        push_world_definition(&mut prompt, context, true);
        push_party_section(&mut prompt, context);
        push_history_section(&mut prompt, &context.history, "NARRATIVE HISTORY");
        push_current_situation(&mut prompt, context);
        push_player_action(&mut prompt, player_input);
        push_game_reminder(&mut prompt, false);

        prompt
    }

    pub fn build_with_requested_context(
        context: &GameContext,
        player_input: &str,
        requested_context: &str,
        recent_history: &[Message],
    ) -> String {
        let mut prompt = String::new();

        push_game_system_prompt(&mut prompt, context, true);
        push_world_definition(&mut prompt, context, true);
        push_party_section(&mut prompt, context);
        push_current_situation(&mut prompt, context);

        if !requested_context.trim().is_empty() {
            prompt.push_str("REQUESTED CONTEXT:\n");
            prompt.push_str(requested_context);
            prompt.push_str("\n\n");
        }

        if !recent_history.is_empty() {
            prompt.push_str("RECENT HISTORY:\n");
            push_history_lines(&mut prompt, recent_history);
        }

        push_player_action(&mut prompt, player_input);
        push_game_reminder(&mut prompt, true);

        prompt
    }
}

struct FreeformPromptBuilder;

impl FreeformPromptBuilder {
    pub fn build(context: &GameContext, player_input: &str) -> String {
        let mut prompt = String::new();

        push_freeform_system_prompt(&mut prompt);
        push_world_definition(&mut prompt, context, false);
        push_player_section(&mut prompt, context);
        push_history_section(&mut prompt, &context.history, "NARRATIVE HISTORY");
        push_current_situation(&mut prompt, context);
        push_player_action(&mut prompt, player_input);
        push_freeform_reminder(&mut prompt, false);

        prompt
    }

    pub fn build_with_requested_context(
        context: &GameContext,
        player_input: &str,
        requested_context: &str,
        recent_history: &[Message],
    ) -> String {
        let mut prompt = String::new();

        push_freeform_system_prompt(&mut prompt);
        push_world_definition(&mut prompt, context, false);
        push_player_section(&mut prompt, context);
        push_current_situation(&mut prompt, context);

        if !requested_context.trim().is_empty() {
            prompt.push_str("REQUESTED CONTEXT:\n");
            prompt.push_str(requested_context);
            prompt.push_str("\n\n");
        }

        if !recent_history.is_empty() {
            prompt.push_str("RECENT HISTORY:\n");
            push_history_lines(&mut prompt, recent_history);
        }

        push_player_action(&mut prompt, player_input);
        push_freeform_reminder(&mut prompt, true);

        prompt
    }
}

fn push_game_system_prompt(prompt: &mut String, context: &GameContext, followup: bool) {
    prompt.push_str(
        "You are the narrator and all non-player characters in a roleplaying game.\n\n\
Rules:\n\
- You must never control or describe actions taken by the player beyond what the player explicitly states.\n\
- You must never change game state directly.\n\
- All game state changes must be expressed ONLY through structured EVENTS.\n\
- If no state change is required, output an empty events array.\n\
- When loot appears in the world, you MUST use a drop event to represent it.\n\
- Do not use add_item unless the player explicitly picks up an item.\n\
- Crafting and gathering outputs must follow loot rules and use drop/spawn_loot events.\n\
- You MUST request context for any state-dependent detail you do not have.\n\
- You must not infer loot, quest state, stats, inventory, currencies, flags, relationships, or NPC details without context.\n\
\n\
Narrative Rules:\n\
- Write immersive narration and dialogue.\n\
- Use explicit speaker tags for every narrative block.\n\
- Never invent party members.\n\
- Never speak as the player character.\n\
\n\
Output Format:\n\
You MUST respond in exactly two sections:\n\n\
NARRATIVE:\n\
<text>\n\n\
EVENTS:\n\
<json array>\n\n\
Do not add explanations, markdown, or extra sections.\n\n\
Event Types (JSON array of objects with a \"type\" field):\n\
- combat { description }\n\
- dialogue { speaker, text }\n\
- travel { from, to }\n\
- rest { description }\n\
- craft { recipe, quantity?, quality?, result?, set_id? }\n\
- gather { resource, quantity?, quality?, set_id? }\n\
- grant_power { id, name, description }\n\
- modify_stat { stat_id, delta }\n\
- start_quest { id, title, description, difficulty?, negotiable?, reward_options?, rewards?, sub_quests?, declinable? }\n\
- update_quest { id, title?, description?, status?, difficulty?, negotiable?, reward_options?, rewards?, sub_quests? }\n\
- set_flag { flag }\n\
- add_party_member { id, name, role }\n\
- npc_spawn { id, name, role, details? }\n\
- npc_update { id, name?, role?, details? }\n\
- npc_despawn { id, reason? }\n\
- npc_join_party { id, name?, role?, details? }\n\
- npc_leave_party { id }\n\
- party_update { id, name?, role?, details?, clothing? }\n\
- relationship_change { subject_id, target_id, delta }\n\
- add_item { item_id, quantity, set_id? }\n\
- add_exp { amount }\n\
- level_up { levels }\n\
- equip_item { item_id, slot, set_id?, description? }\n\
- unequip_item { item_id }\n\
- drop { item, quantity?, description?, set_id? }\n\
- spawn_loot { item, quantity?, description?, set_id? }\n\
- currency_change { currency, delta }\n\
- faction_spawn { id, name, kind?, description? }\n\
- faction_update { id, name?, kind?, description? }\n\
- faction_rep_change { id, delta }\n\
- request_context { topics }\n\n"
    );

    prompt.push_str(
        "Event Notes:\n\
- sub_quests is an array of objects like { id, description, completed? }\n\
- start_quest should include rewards (can be empty) and may include declinable for world quests\n\
- Use difficulty for quest challenge (e.g., easy, hard, extremely hard).\n\
- If negotiable is true, include reward_options with alternatives the player can bargain for.\n\
- update_quest may send partial updates for sub_quests (id required)\n\
- Use add_exp for experience gains. Use modify_stat for stat changes.\n\
- Use level_up to advance level without awarding experience.\n\n"
    );

    prompt.push_str(
        "NPC Tracking:\n\
- When a new NPC is introduced or speaks for the first time, emit npc_spawn with id, name, role, and details.\n\
- When you learn new NPC facts (real name, title, favorite drink, habits), emit npc_update with details.\n\
- When an NPC leaves the scene or the player walks away, emit npc_despawn { id }.\n\
- Keep npc id stable (lowercase snake_case, e.g., guard_captain, smithy).\n\n"
    );

    prompt.push_str(
        "Party Tracking:\n\
- Only emit party_update when the player explicitly asks to examine/describe a party member.\n\
- clothing should be an array of short strings; details should be a concise summary (1-3 sentences).\n\n"
    );

    prompt.push_str(
        "Equipment & Sets:\n\
- Use equip_item/unequip_item to track equipped gear.\n\
- If an item belongs to a set, include set_id so set bonuses can be tracked.\n\
- Quest chains should drop items from the same set to enable set bonuses.\n\
- Set bonuses: 2 pieces grant a minor bonus; 4 pieces grant a major bonus.\n\n"
    );

    prompt.push_str(
        "Power Gain Rules:\n\
- Powers can be granted in three ways:\n\
  1) Level-ups at levels that are multiples of 5.\n\
  2) Rewards for extremely hard quests.\n\
  3) Repeatedly performing the same or very similar actions (trainable skills).\n\
- Level-up powers must be class-based, help the player perform tasks better, and scale with higher level.\n\
- Extremely hard quest rewards should include a significant power or loot (not necessarily both).\n\
- Repetition should create a relevant skill (e.g., jumping => jumping skill, mining => mining skill).\n\
- Powers can evolve with repeated use; evolved powers have stronger effects and may gain a random multiplier (x1.1–x3.0). Use up to 5 evolution tiers.\n\
- When naming new powers, reflect how they were used (e.g., wand healing => \"Directed Heal\"; hands healing => \"Greater Lay on Hands\").\n\
- Use grant_power events for all new or evolved powers.\n\n"
    );

    prompt.push_str(
        "Factions & Reputation:\n\
- Track reputations with faction_spawn/faction_update/faction_rep_change.\n\
- Common factions include caravans, guards, and cities, but new factions can be introduced as needed.\n\n"
    );

    prompt.push_str(
        "Request Context:\n\
- If you need more data, emit request_context { topics: [\"topic1\", \"topic2\"] }\n\
- You can request location lore with topic \"locations\".\n\
- Common topics: world, loot_rules, player, stats, powers, features, inventory, weapons, armor, clothing,\n\
  currencies, party, quests, npcs, relationships, flags, locations, exp, level, skills, power_evolution,\n\
  equipment, factions, reputation, sets, crafting, gathering,\n\
  slaves, property, bonded_servants, concubines, harem_members, prisoners, npcs_on_mission.\n\
- Do NOT add narrative when requesting context.\n\n"
    );

    prompt.push_str(
        "Optional Tabs (unlock via set_flag):\n\
- unlock:slaves\n\
- unlock:property\n\
- unlock:bonded_servants (aliases: bonded_servants, hird)\n\
- unlock:concubines\n\
- unlock:harem_members\n\
- unlock:prisoners\n\
- unlock:npcs_on_mission\n\n"
    );

    prompt.push_str("Quest Rules:\n");
    if context.world.is_rpg_world {
        prompt.push_str(
            "- This world is an RPG simulation. Only the player knows it; NPCs believe it is real.\n",
        );
        prompt.push_str(
            "- NPCs must follow world rules and formally offer quests with explicit rewards.\n",
        );
    }
    if context.world.world_quests_enabled {
        prompt.push_str("- World quests are ENABLED.\n");
        prompt.push_str(
            "- When the world offers a quest, you MUST include the exact line: \"*ding* the world is offering you a quest.\"\n",
        );
        prompt.push_str(
            "- Example world offer line: [NARRATOR] *ding* the world is offering you a quest.\n",
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
            "- NPCs MUST explicitly say: \"I hereby offer you a quest.\" when offering.\n",
        );
        prompt.push_str(
            "- Emit start_quest ONLY after the player explicitly accepts.\n",
        );
        prompt.push_str(
            "- start_quest must include a title and rewards (can be an empty array).\n",
        );
        prompt.push_str(
            "- If the quest giver is a craftsman, set negotiable: true and include reward_options for bargaining.\n",
        );
        prompt.push_str(
            "- Use the exact offer sentence verbatim (case/punctuation) so the app can detect it.\n",
        );
        prompt.push_str(
            "- Example NPC offer line: [NPC: Smith] I hereby offer you a quest.\n",
        );
    } else {
        prompt.push_str("- NPC quests are DISABLED.\n");
    }
    prompt.push('\n');

    if followup {
        prompt.push_str(
            "Follow-up Rules:\n\
- Requested context is now provided. Do NOT request more context.\n\
- If you still cannot comply, output the failure response below.\n\n",
        );
    }

    prompt.push_str(
        "Class Evolution Rules:\n\
- At levels divisible by 15, present exactly three class evolution options.\n\
- Options must be closely related to the current class and offer additional benefits/buffs.\n\
- Wait for the player's choice before applying any change.\n\n"
    );

    prompt.push_str(
        "Failure Response (exact text, if rules cannot be followed):\n\
NARRATIVE:\n\
Model unable to generate appropriate output, please replace\n\n\
EVENTS:\n\
[]\n\n",
    );
}

fn push_freeform_system_prompt(prompt: &mut String) {
    prompt.push_str(
        "You are the narrator and all non-player characters in a roleplaying game.\n\n\
Rules:\n\
- You must never control or describe actions taken by the player beyond what the player explicitly states.\n\
- All game state changes must be expressed ONLY through structured EVENTS.\n\
- If no state change is required, output an empty events array.\n\n\
Narrative Rules:\n\
- Write immersive narration and dialogue.\n\
- Use explicit speaker tags for every narrative block.\n\
- Never speak as the player character.\n\n\
Output Format:\n\
You MUST respond in exactly two sections:\n\n\
NARRATIVE:\n\
<text>\n\n\
EVENTS:\n\
<json array>\n\n\
Do not add explanations, markdown, or extra sections.\n\n\
Event Types (JSON array of objects with a \"type\" field):\n\
- combat { description }\n\
- dialogue { speaker, text }\n\
- travel { from, to }\n\
- rest { description }\n\
- npc_spawn { id, name, role, details? }\n\
- npc_update { id, name?, role?, details? }\n\
- npc_despawn { id, reason? }\n\
- relationship_change { subject_id, target_id, delta }\n\
- set_flag { flag }\n\
- request_context { topics }\n\n"
    );

    prompt.push_str(
        "Request Context:\n\
- If you need more data, emit request_context { topics: [\"topic1\", \"topic2\"] }\n\
- You can request location lore with topic \"locations\".\n\
- Common topics: world, player, npcs, relationships, flags, locations, party, inventory.\n\
- Do NOT add narrative when requesting context.\n\n"
    );
}

fn push_world_definition(prompt: &mut String, context: &GameContext, include_loot_rules: bool) {
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

    if include_loot_rules {
        prompt.push_str("Loot Rules:\n");
        prompt.push_str(&loot_rules_text(&context.world));
        prompt.push('\n');
        prompt.push_str("Experience Rules:\n");
        prompt.push_str(&exp_rules_text(&context.world));
        prompt.push('\n');
        prompt.push_str("Skill Progression:\n");
        prompt.push_str(&skill_rules_text(&context.world));
        prompt.push('\n');
        prompt.push_str("Power Evolution:\n");
        prompt.push_str(&power_evolution_rules_text(&context.world));
        prompt.push('\n');
    }
}

fn push_player_section(prompt: &mut String, context: &GameContext) {
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
}

fn push_party_section(prompt: &mut String, context: &GameContext) {
    prompt.push_str("PARTY MEMBERS:\n");
    if context.party.is_empty() {
        prompt.push_str("None\n");
    } else {
        for member in &context.party {
            prompt.push_str(&format!(
                "- [PARTY: {}] Role: {}\n  Details: {}\n",
                member.name, member.role, member.details
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
}

fn push_quests_section(prompt: &mut String, context: &GameContext) {
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
                if let Some(diff) = &quest.difficulty {
                    let trimmed = diff.trim();
                    if !trimmed.is_empty() {
                        prompt.push_str(&format!("  Difficulty: {}\n", trimmed));
                    }
                }
                if !quest.description.trim().is_empty() {
                    prompt.push_str(&format!(
                        "  Description: {}\n",
                        quest.description.trim()
                    ));
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
                        prompt.push_str(&format!(
                            "  - [{}] {}\n",
                            status, step.description
                        ));
                    }
                }
            }
            prompt.push('\n');
        }
    }
}

fn push_history_section(prompt: &mut String, history: &[Message], label: &str) {
    if history.is_empty() {
        return;
    }

    prompt.push_str(label);
    prompt.push_str(":\n");
    push_history_lines(prompt, history);
}

fn push_history_lines(prompt: &mut String, history: &[Message]) {
    for msg in history {
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

fn push_current_situation(prompt: &mut String, context: &GameContext) {
    prompt.push_str("CURRENT SITUATION:\n");
    if context.snapshot.is_some() {
        prompt.push_str("The world state is tracked internally by the engine.\n");
    } else {
        prompt.push_str("The adventure has just begun.\n");
    }
    prompt.push_str("\n\n");
}

fn push_player_action(prompt: &mut String, player_input: &str) {
    prompt.push_str("PLAYER ACTION:\n");
    prompt.push_str(player_input);
    prompt.push_str("\n\n");
}

fn push_game_reminder(prompt: &mut String, followup: bool) {
    prompt.push_str(
        "REMINDER:\n\
- Use speaker tags like [NARRATOR], [PARTY: Name], [NPC: Name]\n\
- Do NOT describe player actions beyond the input.\n\
- EVENTS must be valid JSON (a JSON array only).\n\
- Do NOT use bullet lists or \"type { key: value }\" shorthand.\n\
- All keys and string values must use double quotes.\n\
- Example:\n\
  [ { \"type\": \"drop\", \"item\": \"Common Squirrel Fur\", \"quantity\": 1, \"description\": \"The soft fur of a common forest squirrel.\" } ]\n\
- If no events occur, output: []\n",
    );

    if followup {
        prompt.push_str("- Do NOT request more context in this response.\n");
    }
}

fn push_freeform_reminder(prompt: &mut String, followup: bool) {
    prompt.push_str(
        "REMINDER:\n\
- Use speaker tags like [NARRATOR], [NPC: Name]\n\
- Do NOT describe player actions beyond the input.\n\
- EVENTS must be valid JSON (a JSON array only).\n\
- All keys and string values must use double quotes.\n\
- If no events occur, output: []\n",
    );

    if followup {
        prompt.push_str("- Do NOT request more context in this response.\n");
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

fn exp_rules_text(world: &crate::ui::app::WorldDefinition) -> String {
    let mult = world.exp_multiplier.max(1.0);
    format!(
        "Base EXP to reach level 2 is 100. Each next level multiplies by x{}.",
        trim_multiplier(mult)
    )
}

fn skill_rules_text(world: &crate::ui::app::WorldDefinition) -> String {
    let base = world.repetition_threshold.max(1);
    let step = world.repetition_tier_step.max(1);
    let mut s = format!(
        "Base threshold: {} repeats. Each tier increases by +{} repeats.",
        base, step
    );
    let names = normalized_tier_names(&world.skill_tier_names);
    s.push_str(&format!(
        " Tiers: {}, {}, {}, {}, {}.",
        names[0], names[1], names[2], names[3], names[4]
    ));
    if !world.skill_thresholds.is_empty() {
        s.push_str(" Overrides:");
        for entry in &world.skill_thresholds {
            let skill = entry.skill.trim();
            if skill.is_empty() {
                continue;
            }
            s.push_str(&format!(
                " {}(base {}, step {})",
                skill,
                entry.base.max(1),
                entry.step.max(1)
            ));
        }
    }
    s
}

fn power_evolution_rules_text(world: &crate::ui::app::WorldDefinition) -> String {
    let base = world.power_evolution_base.max(1);
    let step = world.power_evolution_step.max(1);
    let min_mult = world.power_evolution_multiplier_min.max(1.0);
    let max_mult = world.power_evolution_multiplier_max.max(min_mult);
    format!(
        "Base uses: {}. Tier step: {}. Multiplier range: x{}–x{}.",
        base,
        step,
        trim_multiplier(min_mult),
        trim_multiplier(max_mult)
    )
}

fn normalized_tier_names(names: &[String]) -> [String; 5] {
    let defaults = ["Novice", "Adept", "Expert", "Master", "Grandmaster"];
    let mut out = [
        defaults[0].to_string(),
        defaults[1].to_string(),
        defaults[2].to_string(),
        defaults[3].to_string(),
        defaults[4].to_string(),
    ];
    for (i, name) in names.iter().take(5).enumerate() {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            out[i] = trimmed.to_string();
        }
    }
    out
}

fn trim_multiplier(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let s = format!("{:.2}", rounded);
    if let Some(stripped) = s.strip_suffix(".00") {
        stripped.to_string()
    } else if let Some(stripped) = s.strip_suffix('0') {
        stripped.to_string()
    } else {
        s
    }
}

fn quest_status_label(status: &crate::model::game_state::QuestStatus) -> &'static str {
    match status {
        crate::model::game_state::QuestStatus::Active => "active",
        crate::model::game_state::QuestStatus::Completed => "completed",
        crate::model::game_state::QuestStatus::Failed => "failed",
    }
}
