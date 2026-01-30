use std::sync::mpsc::{Receiver, Sender};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::engine::prompt_builder::PromptBuilder;
use crate::engine::llm_client::{call_llm, test_connection};
use crate::engine::narrative_parser::parse_narrative;

use crate::model::event_result::{
    NarrativeApplyReport,
    EventApplication,
    EventApplyOutcome,
};
use crate::model::internal_game_state::InternalGameState;
use crate::model::game_state::LootDrop;
use crate::model::message::Message;
use crate::model::narrative_event::NarrativeEvent;
use crate::model::game_save::GameSave;
use std::fs;

pub struct Engine {
    rx: Receiver<EngineCommand>,
    tx: Sender<EngineResponse>,

    messages: Vec<Message>,
    game_state: InternalGameState,
}

#[derive(Clone, Copy, Debug)]
enum QuestOfferSource {
    World,
    Npc,
}

impl Engine {
    pub fn new(
        rx: Receiver<EngineCommand>,
        tx: Sender<EngineResponse>,
    ) -> Self {
        Self {
            rx,
            tx,
            messages: Vec::new(),
            game_state: InternalGameState::default(),
        }
    }

pub fn run(&mut self) {
    while let Ok(cmd) = self.rx.recv() {
        match cmd {

            /* =========================
               Initialize narrative (world load)
               ========================= */
            EngineCommand::InitializeNarrative { opening_message } => {
                // Reset session
                self.messages.clear();
                self.game_state = InternalGameState::default();

                // Inject narrator opening
                self.messages.push(Message::Roleplay {
                    speaker: crate::model::message::RoleplaySpeaker::Narrator,
                    text: opening_message,
                });

                // Notify UI immediately
                let _ = self.tx.send(
                    EngineResponse::FullMessageHistory(self.messages.clone())
                );
            }

            /* =========================
               Player input → Prompt → LLM
               ========================= */
            EngineCommand::SubmitPlayerInput { text, context, llm } => {
                // 1. Record player input
                self.messages.push(Message::User(text.clone()));

                // 1b. Handle explicit pickup commands without the LLM
                if is_pickup_intent(&text) {
                    if is_pickup_all_command(&text) {
                        let applications = move_all_loot_to_inventory(&mut self.game_state);
                        if applications.is_empty() {
                            self.messages.push(Message::System(
                                "No loot to add to inventory.".to_string(),
                            ));
                            let _ = self.tx.send(
                                EngineResponse::FullMessageHistory(self.messages.clone())
                            );
                            continue;
                        }

                        self.messages.push(Message::System(
                            "Added all loot to inventory.".to_string(),
                        ));

                        let report = NarrativeApplyReport { applications };
                        let snapshot = (&self.game_state).into();
                        let _ = self.tx.send(
                            EngineResponse::NarrativeApplied {
                                report,
                                snapshot,
                            }
                        );
                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                        continue;
                    }

                    let selected = select_loot_mentions(&text, &self.game_state.loot);
                    if !selected.is_empty() {
                        let (applications, moved_labels) =
                            move_selected_loot_to_inventory(&mut self.game_state, &selected);

                        let summary = if moved_labels.len() == 1 {
                            format!("Added to inventory: {}", moved_labels[0])
                        } else {
                            format!("Added to inventory: {}", moved_labels.join(", "))
                        };
                        self.messages.push(Message::System(summary));

                        let report = NarrativeApplyReport { applications };
                        let snapshot = (&self.game_state).into();
                        let _ = self.tx.send(
                            EngineResponse::NarrativeApplied {
                                report,
                                snapshot,
                            }
                        );
                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                        continue;
                    }
                }

                // 2. Build prompt
                let prompt = PromptBuilder::build(&context, &text);

                // 3. Call LM Studio
                let llm_output = match call_llm(prompt, &llm) {
                    Ok(text) => text,
                    Err(e) => {
                        self.messages.push(Message::System(format!(
                            "LLM error: {}",
                            e
                        )));
                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                        continue;
                    }
                };

                // 4. Split NARRATIVE vs EVENTS
                let (narrative, events_json) =
                    llm_output
                        .split_once("EVENTS:")
                        .unwrap_or((&llm_output, "[]"));

                // 5. Decode EVENTS JSON
                let events = match crate::model::llm_decode::decode_llm_events(events_json) {
                    Ok(events) => events,
                    Err(err) => {
                        self.messages.push(Message::System(format!(
                            "Failed to parse EVENTS: {}",
                            err
                        )));
                        Vec::new()
                    }
                };

                // 6. Handle request_context (one additional round)
                if let Some(topics) = collect_requested_topics(&events) {
                    let requested_context = build_requested_context(
                        &self.game_state,
                        &context,
                        &topics,
                    );
                    let recent_history = tail_messages(&self.messages, 12);
                    let followup_prompt = PromptBuilder::build_with_requested_context(
                        &context,
                        &text,
                        &requested_context,
                        &recent_history,
                    );
                    let llm_output = match call_llm(followup_prompt, &llm) {
                        Ok(text) => text,
                        Err(e) => {
                            self.messages.push(Message::System(format!(
                                "LLM error: {}",
                                e
                            )));
                            let _ = self.tx.send(
                                EngineResponse::FullMessageHistory(self.messages.clone())
                            );
                            continue;
                        }
                    };

                    let (narrative, events_json) =
                        llm_output
                            .split_once("EVENTS:")
                            .unwrap_or((&llm_output, "[]"));
                    let events = match crate::model::llm_decode::decode_llm_events(events_json) {
                        Ok(events) => events,
                        Err(err) => {
                            self.messages.push(Message::System(format!(
                                "Failed to parse EVENTS: {}",
                                err
                            )));
                            Vec::new()
                        }
                    };

                    if events.iter().any(|e| matches!(e, NarrativeEvent::RequestContext { .. })) {
                        self.messages.push(Message::System(
                            "Context was already provided. Please respond with narrative and events."
                                .to_string(),
                        ));
                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                        continue;
                    }

                    let new_messages = parse_narrative(narrative);
                    self.messages.extend(new_messages);

                    let mut applications = Vec::new();
                    let offer_source = quest_offer_source(narrative);
                    let player_accepts = player_accepts_quest(&text);
                    for event in events {
                        if let NarrativeEvent::StartQuest { .. } = event {
                            if let Some(reason) =
                                validate_start_quest(&event, offer_source, player_accepts, &context.world)
                            {
                                applications.push(EventApplication {
                                    event,
                                    outcome: EventApplyOutcome::Deferred { reason },
                                });
                                continue;
                            }
                        }
                        let outcome = apply_event(&mut self.game_state, event.clone());
                        applications.push(EventApplication { event, outcome });
                    }

                    if !applications.is_empty() {
                        let report = NarrativeApplyReport { applications };
                        let snapshot = (&self.game_state).into();
                        let _ = self.tx.send(
                            EngineResponse::NarrativeApplied { report, snapshot }
                        );
                    }

                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                    continue;
                }

                // 7. Parse narrative into structured messages
                let new_messages = parse_narrative(narrative);
                self.messages.extend(new_messages);

                // 8. Apply events
                let mut applications = Vec::new();
                let offer_source = quest_offer_source(narrative);
                let player_accepts = player_accepts_quest(&text);

                for event in events {
                    if let NarrativeEvent::StartQuest { .. } = event {
                        if let Some(reason) =
                            validate_start_quest(&event, offer_source, player_accepts, &context.world)
                        {
                            applications.push(EventApplication {
                                event,
                                outcome: EventApplyOutcome::Deferred { reason },
                            });
                            continue;
                        }
                    }
                    let outcome = apply_event(&mut self.game_state, event.clone());
                    applications.push(EventApplication {
                        event,
                        outcome,
                    });
                }

                // 9. Send state mutation report
                if !applications.is_empty() {
                    let report = NarrativeApplyReport { applications };
                    let snapshot = (&self.game_state).into();

                    let _ = self.tx.send(
                        EngineResponse::NarrativeApplied {
                            report,
                            snapshot,
                        }
                    );
                }

                // 10. Update UI with full history
                let _ = self.tx.send(
                    EngineResponse::FullMessageHistory(self.messages.clone())
                );
            }

            /* =========================
               Connect to LM Studio
               ========================= */
            EngineCommand::ConnectToLlm { llm } => {
                match test_connection(&llm) {
                    Ok(msg) => {
                        let _ = self.tx.send(
                            EngineResponse::LlmConnectionResult {
                                success: true,
                                message: msg,
                            }
                        );
                    }
                    Err(e) => {
                        let _ = self.tx.send(
                            EngineResponse::LlmConnectionResult {
                                success: false,
                                message: format!("Connection failed: {}", e),
                            }
                        );
                    }
                }
            }

            /* =========================
               UI: Add NPC to party
               ========================= */
            EngineCommand::AddNpcToParty { id, name, role, details } => {
                let event = crate::model::narrative_event::NarrativeEvent::NpcJoinParty {
                    id,
                    name: Some(name),
                    role: Some(role),
                    details: Some(details),
                };

                let outcome = apply_event(&mut self.game_state, event.clone());
                let report = NarrativeApplyReport {
                    applications: vec![EventApplication { event, outcome }],
                };
                let snapshot = (&self.game_state).into();

                let _ = self.tx.send(
                    EngineResponse::NarrativeApplied { report, snapshot }
                );
            }

            /* =========================
               Save / Load Game
               ========================= */
            EngineCommand::SaveGame {
                path,
                world,
                player,
                party,
                speaker_colors,
            } => {
                let save = GameSave {
                    version: 2,
                    world,
                    player,
                    party,
                    messages: self.messages.clone(),
                    internal_state: self.game_state.clone(),
                    speaker_colors,
                };
                let result = serde_json::to_string_pretty(&save)
                    .map_err(|e| e.to_string())
                    .and_then(|json| fs::write(&path, json).map_err(|e| e.to_string()));

                match result {
                    Ok(_) => {
                        self.messages.push(Message::System("Game saved.".to_string()));
                    }
                    Err(err) => {
                        self.messages.push(Message::System(format!(
                            "Failed to save game: {}",
                            err
                        )));
                    }
                }

                let _ = self.tx.send(
                    EngineResponse::FullMessageHistory(self.messages.clone())
                );
            }

            EngineCommand::LoadGame { path } => {
                let result = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|data| serde_json::from_str::<GameSave>(&data).map_err(|e| e.to_string()));

                match result {
                    Ok(save) => {
                        self.messages = save.messages.clone();
                        self.game_state = save.internal_state.clone();
                        let snapshot = (&self.game_state).into();

                        let _ = self.tx.send(
                            EngineResponse::GameLoaded { save, snapshot }
                        );

                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                    }
                    Err(err) => {
                        self.messages.push(Message::System(format!(
                            "Failed to load game: {}",
                            err
                        )));
                        let _ = self.tx.send(
                            EngineResponse::FullMessageHistory(self.messages.clone())
                        );
                    }
                }
            }

        }
    }
}
}

fn is_pickup_all_command(text: &str) -> bool {
    let t = text.to_lowercase();
    let phrases = [
        "add all items to inventory",
        "add all to inventory",
        "take all",
        "take everything",
        "loot all",
        "pick up all",
        "pickup all",
        "collect all",
        "grab all",
    ];
    phrases.iter().any(|p| t.contains(p))
}

fn collect_requested_topics(events: &[NarrativeEvent]) -> Option<Vec<String>> {
    let mut topics = Vec::new();
    for event in events {
        if let NarrativeEvent::RequestContext { topics: requested } = event {
            for topic in requested {
                let t = topic.trim().to_lowercase();
                if !t.is_empty() && !topics.contains(&t) {
                    topics.push(t);
                }
            }
        }
    }
    if topics.is_empty() {
        None
    } else {
        Some(topics)
    }
}

fn tail_messages(messages: &[Message], max: usize) -> Vec<Message> {
    if messages.len() <= max {
        messages.to_vec()
    } else {
        messages[messages.len().saturating_sub(max)..].to_vec()
    }
}

fn build_requested_context(
    state: &InternalGameState,
    context: &crate::model::game_context::GameContext,
    topics: &[String],
) -> String {
    let mut out = String::new();

    for topic in topics {
        match topic.as_str() {
            "world" | "world_rules" | "world_definition" => {
                push_section(&mut out, "WORLD", &format_world(context));
            }
            "loot_rules" | "loot" => {
                push_section(&mut out, "LOOT RULES", &format_loot_rules(context));
            }
            "player" | "character" => {
                push_section(&mut out, "PLAYER", &format_player(context));
            }
            "stats" => {
                push_section(&mut out, "STATS", &format_stats(context));
            }
            "powers" => {
                push_section(&mut out, "POWERS", &format_list(&context.player.powers));
            }
            "features" => {
                push_section(&mut out, "FEATURES", &format_list(&context.player.features));
            }
            "inventory" => {
                push_section(&mut out, "INVENTORY", &format_inventory(state));
            }
            "weapons" => {
                push_section(&mut out, "WEAPONS", &format_list(&state.player.weapons));
            }
            "armor" | "armour" => {
                push_section(&mut out, "ARMOUR", &format_list(&state.player.armor));
            }
            "clothing" => {
                push_section(&mut out, "CLOTHING", &format_list(&state.player.clothing));
            }
            "currencies" | "currency" | "gold" => {
                push_section(&mut out, "CURRENCIES", &format_currencies(state));
            }
            "party" => {
                push_section(&mut out, "PARTY", &format_party(state));
            }
            "quests" => {
                push_section(&mut out, "QUESTS", &format_quests(state));
            }
            "npcs" => {
                push_section(&mut out, "NPCS", &format_npcs(state));
            }
            "relationships" => {
                push_section(&mut out, "RELATIONSHIPS", &format_relationships(state));
            }
            "flags" => {
                push_section(&mut out, "FLAGS", &format_flags(state));
            }
            "slaves" | "property" | "bonded_servants" | "concubines" | "harem_members"
            | "prisoners" | "npcs_on_mission" => {
                push_section(
                    &mut out,
                    "OPTIONAL TAB",
                    "No structured data tracked for this tab yet.",
                );
            }
            _ => {
                push_section(
                    &mut out,
                    "UNKNOWN TOPIC",
                    &format!("No provider for topic '{}'.", topic),
                );
            }
        }
    }

    out
}

fn push_section(out: &mut String, title: &str, body: &str) {
    out.push_str(title);
    out.push_str(":\n");
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
}

fn format_world(context: &crate::model::game_context::GameContext) -> String {
    let mut s = String::new();
    let w = &context.world;
    s.push_str(&format!("Title: {}\n", w.title));
    s.push_str(&format!("Author: {}\n", w.author));
    s.push_str("Description:\n");
    s.push_str(&w.description);
    s.push('\n');
    if !w.themes.is_empty() {
        s.push_str("Themes:\n");
        for t in &w.themes {
            s.push_str(&format!("- {}\n", t));
        }
    }
    if !w.tone.is_empty() {
        s.push_str("Tone:\n");
        for t in &w.tone {
            s.push_str(&format!("- {}\n", t));
        }
    }
    if !w.narrator_role.is_empty() {
        s.push_str("Narration Rules:\n");
        s.push_str(&w.narrator_role);
        s.push('\n');
    }
    if !w.style_guidelines.is_empty() {
        s.push_str("Style Guidelines:\n");
        for r in &w.style_guidelines {
            s.push_str(&format!("- {}\n", r));
        }
    }
    if !w.must_not.is_empty() {
        s.push_str("Must NOT:\n");
        for r in &w.must_not {
            s.push_str(&format!("- {}\n", r));
        }
    }
    if !w.must_always.is_empty() {
        s.push_str("Must ALWAYS:\n");
        for r in &w.must_always {
            s.push_str(&format!("- {}\n", r));
        }
    }
    s.push_str("Loot Rules:\n");
    s.push_str(&format_loot_rules(context));
    s
}

fn format_loot_rules(context: &crate::model::game_context::GameContext) -> String {
    let w = &context.world;
    let mode = w.loot_rules_mode.trim();
    let mut s = if mode.eq_ignore_ascii_case("difficulty based") {
        "Difficulty based: Harder tasks yield better rewards.\n".to_string()
    } else if mode.eq_ignore_ascii_case("rarity based") {
        "Rarity based: Each drop can roll from any tier (Common, Uncommon, Rare, Legendary, Exotic, Godly).\n".to_string()
    } else if !w.loot_rules_custom.trim().is_empty() {
        format!("Custom: {}\n", w.loot_rules_custom.trim())
    } else {
        "Custom: (not specified)\n".to_string()
    };
    s.push_str("Applies to activity rewards (Mining, Fishing, Woodcutting, Farming, Crafting).\n");
    s
}

fn format_player(context: &crate::model::game_context::GameContext) -> String {
    let p = &context.player;
    format!(
        "Name: {}\nClass: {}\nBackground:\n{}\n",
        p.name, p.class, p.background
    )
}

fn format_stats(context: &crate::model::game_context::GameContext) -> String {
    if context.player.stats.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for (k, v) in &context.player.stats {
        s.push_str(&format!("- {}: {}\n", k, v));
    }
    s
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for item in items {
        s.push_str(&format!("- {}\n", item));
    }
    s
}

fn format_inventory(state: &InternalGameState) -> String {
    if state.inventory.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for item in state.inventory.values() {
        if item.quantity <= 1 {
            s.push_str(&format!("- {}\n", item.id));
        } else {
            s.push_str(&format!("- {} x{}\n", item.id, item.quantity));
        }
    }
    s
}

fn format_currencies(state: &InternalGameState) -> String {
    if state.currencies.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for (currency, amount) in &state.currencies {
        s.push_str(&format!("- {}: {}\n", currency, amount));
    }
    s
}

fn format_party(state: &InternalGameState) -> String {
    if state.party.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for member in state.party.values() {
        s.push_str(&format!("- {} ({})\n", member.name, member.role));
    }
    s
}

fn format_quests(state: &InternalGameState) -> String {
    if state.quests.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for quest in state.quests.values() {
        s.push_str(&format!(
            "- [{}] {}\n",
            quest_status_label(&quest.status),
            quest.title
        ));
        if !quest.description.trim().is_empty() {
            s.push_str(&format!("  Description: {}\n", quest.description));
        }
        if !quest.rewards.is_empty() {
            s.push_str("  Rewards:\n");
            for r in &quest.rewards {
                s.push_str(&format!("  - {}\n", r));
            }
        }
        if !quest.sub_quests.is_empty() {
            s.push_str("  Sub-quests:\n");
            for step in &quest.sub_quests {
                let status = if step.completed { "done" } else { "open" };
                s.push_str(&format!("  - [{}] {}\n", status, step.description));
            }
        }
    }
    s
}

fn quest_status_label(status: &crate::model::game_state::QuestStatus) -> &'static str {
    match status {
        crate::model::game_state::QuestStatus::Active => "active",
        crate::model::game_state::QuestStatus::Completed => "completed",
        crate::model::game_state::QuestStatus::Failed => "failed",
    }
}

fn format_npcs(state: &InternalGameState) -> String {
    if state.npcs.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for npc in state.npcs.values() {
        s.push_str(&format!("- {} ({})\n", npc.name, npc.role));
    }
    s
}

fn format_relationships(state: &InternalGameState) -> String {
    if state.relationships.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for rel in state.relationships.values() {
        s.push_str(&format!(
            "- {} -> {}: {}\n",
            rel.subject_id, rel.target_id, rel.value
        ));
    }
    s
}

fn format_flags(state: &InternalGameState) -> String {
    if state.flags.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for flag in &state.flags {
        s.push_str(&format!("- {}\n", flag));
    }
    s
}

fn is_pickup_intent(text: &str) -> bool {
    let t = text.to_lowercase();
    let verbs = [
        "add to inventory",
        "add to my inventory",
        "take",
        "take the",
        "take all",
        "loot",
        "loot the",
        "pick up",
        "pickup",
        "collect",
        "grab",
    ];
    verbs.iter().any(|v| t.contains(v))
}

fn move_all_loot_to_inventory(state: &mut InternalGameState) -> Vec<EventApplication> {
    let selected: Vec<usize> = (0..state.loot.len()).collect();
    let (applications, _) = move_selected_loot_to_inventory(state, &selected);
    applications
}

fn select_loot_mentions(text: &str, loot: &[LootDrop]) -> Vec<usize> {
    let t = text.to_lowercase();
    let mut selected = Vec::new();
    for (idx, drop) in loot.iter().enumerate() {
        let name = drop.item.to_lowercase();
        if t.contains(&name) {
            selected.push(idx);
        }
    }
    selected
}

fn move_selected_loot_to_inventory(
    state: &mut InternalGameState,
    selected: &[usize],
) -> (Vec<EventApplication>, Vec<String>) {
    if selected.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut applications = Vec::new();
    let mut moved_labels = Vec::new();
    let mut remaining = Vec::new();

    for (idx, drop) in std::mem::take(&mut state.loot).into_iter().enumerate() {
        if selected.contains(&idx) {
            let entry = state.inventory.entry(drop.item.clone()).or_insert(
                crate::model::game_state::ItemStack {
                    id: drop.item.clone(),
                    quantity: 0,
                    description: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(drop.quantity);
            if entry.description.is_none() {
                entry.description = drop.description.clone();
            }

            moved_labels.push(format!("{} x{}", drop.item, drop.quantity));
            applications.push(EventApplication {
                event: NarrativeEvent::AddItem {
                    item_id: drop.item,
                    quantity: drop.quantity,
                },
                outcome: EventApplyOutcome::Applied,
            });
        } else {
            remaining.push(drop);
        }
    }

    state.loot = remaining;
    (applications, moved_labels)
}

fn quest_offer_source(narrative: &str) -> Option<QuestOfferSource> {
    let n = narrative.to_ascii_lowercase();
    if n.contains("*ding* the world is offering you a quest.") {
        return Some(QuestOfferSource::World);
    }
    if n.contains("i hereby offer you a quest.") {
        return Some(QuestOfferSource::Npc);
    }
    None
}

fn player_accepts_quest(input: &str) -> bool {
    let t = input.to_ascii_lowercase();
    let phrases = [
        "i accept",
        "i accept the quest",
        "accept quest",
        "accept the quest",
        "yes i accept",
        "yes, i accept",
        "i agree",
        "i will do it",
    ];
    phrases.iter().any(|p| t.contains(p))
}

fn validate_start_quest(
    event: &NarrativeEvent,
    offer_source: Option<QuestOfferSource>,
    player_accepts: bool,
    world: &crate::ui::app::WorldDefinition,
) -> Option<String> {
    let NarrativeEvent::StartQuest { declinable, .. } = event else {
        return None;
    };

    let source = match offer_source {
        Some(source) => source,
        None => {
            return Some("Quest rejected: missing quest offer phrase.".to_string());
        }
    };

    match source {
        QuestOfferSource::World => {
            if !world.world_quests_enabled {
                return Some("Quest rejected: world quests are disabled.".to_string());
            }
            if declinable == &Some(false) && !world.world_quests_mandatory {
                return Some("Quest rejected: mandatory world quests are disabled.".to_string());
            }
            if declinable == &Some(false) && world.world_quests_mandatory {
                return None;
            }
            if player_accepts {
                None
            } else {
                Some("Quest pending: player has not accepted the world quest.".to_string())
            }
        }
        QuestOfferSource::Npc => {
            if !world.npc_quests_enabled {
                return Some("Quest rejected: NPC quests are disabled.".to_string());
            }
            if player_accepts {
                None
            } else {
                Some("Quest pending: player has not accepted the quest.".to_string())
            }
        }
    }
}
