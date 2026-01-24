use crate::model::game_context::GameContext;

/// Builds the full prompt sent to the LLM.
/// This struct is intentionally dumb: it only formats text.
/// No parsing, no networking, no engine logic.
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(context: &GameContext, player_input: &str) -> String {
        let mut prompt = String::new();

        // ---------- SYSTEM PROMPT ----------
        prompt.push_str(
            "You are the narrator and all non-player characters in a roleplaying game.\n\n\
Rules:\n\
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
Do not add explanations, markdown, or extra sections.\n\n"
        );

        // ---------- WORLD ----------
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

        // ---------- PLAYER ----------
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

        // ---------- PARTY ----------
        prompt.push_str("PARTY MEMBERS:\n");
        if context.party.is_empty() {
            prompt.push_str("None\n");
        } else {
            for member in &context.party {
                prompt.push_str(&format!(
                    "- {} ({})\n  Notes: {}\n",
                    member.name,
                    member.role,
                    member.notes
                ));
            }
        }
        prompt.push('\n');

// ---------- CURRENT SITUATION ----------
prompt.push_str("CURRENT SITUATION:\n");

if context.snapshot.is_some() {
    prompt.push_str("The world state is tracked internally by the engine.\n");
} else {
    prompt.push_str("The adventure has just begun.\n");
}

prompt.push_str("\n\n");


        // ---------- PLAYER INPUT ----------
        prompt.push_str("PLAYER ACTION:\n");
        prompt.push_str(player_input);
        prompt.push_str("\n\n");

        // ---------- REMINDER ----------
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
