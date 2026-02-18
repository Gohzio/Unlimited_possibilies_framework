use std::sync::mpsc::{Receiver, Sender, TryRecvError, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::thread;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::engine::prompt_builder::PromptBuilder;
use crate::engine::llm_client::{abort_generation, call_llm, call_llm_events_structured, test_connection};
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
use rand::Rng;
use std::fs;

pub struct Engine {
    rx: Receiver<EngineCommand>,
    tx: Sender<EngineResponse>,

    messages: Vec<Message>,
    game_state: InternalGameState,
    timing_enabled: bool,
    debug_messages_enabled: bool,
    npc_recency_limit: usize,
    turn_index: u64,
    last_quest_offer_source: Option<QuestOfferSource>,
    last_quest_offer_turn: Option<u64>,
    pending_generation: Option<PendingGeneration>,
}

const SAVE_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug)]
enum QuestOfferSource {
    World,
    Npc,
}

struct PendingGeneration {
    messages_start: usize,
    text: String,
    context: crate::model::game_context::GameContext,
    llm: crate::engine::llm_client::LlmConfig,
    total_start: Instant,
    response_rx: Receiver<anyhow::Result<String>>,
    canceled: bool,
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
            timing_enabled: true,
            debug_messages_enabled: true,
            npc_recency_limit: 10,
            turn_index: 0,
            last_quest_offer_source: None,
            last_quest_offer_turn: None,
            pending_generation: None,
        }
    }

    fn send_ui_error(&self, message: String) {
        let _ = self.tx.send(EngineResponse::UiError { message });
    }

    fn push_debug_message(&mut self, message: String) {
        if self.debug_messages_enabled {
            self.messages.push(Message::System(message));
        }
    }

    fn trim_messages_after_last_user(&mut self) -> Option<String> {
        let mut idx = self.messages.len();
        while idx > 0 {
            if matches!(self.messages[idx - 1], Message::User(_)) {
                break;
            }
            idx -= 1;
        }

        if idx == 0 {
            return None;
        }

        let last_user = match &self.messages[idx - 1] {
            Message::User(text) => text.clone(),
            _ => return None,
        };

        self.messages.truncate(idx);
        Some(last_user)
    }

    fn update_npc_proximity_from_recent_messages(&mut self, limit: usize) -> bool {
        use std::collections::HashSet;

        if self.game_state.npcs.is_empty() {
            return false;
        }

        let mut active_names: HashSet<String> = HashSet::new();
        for msg in self.messages.iter().rev().take(limit) {
            if let Message::Roleplay {
                speaker: crate::model::message::RoleplaySpeaker::Npc,
                text,
            } = msg
            {
                let name = text.splitn(2, ':').next().unwrap_or("").trim();
                if !name.is_empty() {
                    active_names.insert(name.to_lowercase());
                }
            }
        }

        let mut changed = false;
        for npc in self.game_state.npcs.values_mut() {
            let name_key = npc.name.to_lowercase();
            let should_be_nearby = active_names.contains(&name_key);
            if npc.nearby != should_be_nearby {
                npc.nearby = should_be_nearby;
                changed = true;
            }
        }

        changed
    }

    fn extract_event_types_from_value(value: &serde_json::Value) -> Option<HashSet<String>> {
        let array = value.as_array()?;
        let mut out = HashSet::new();
        for item in array {
            if let Some(t) = item.get("type").and_then(|v| v.as_str()) {
                out.insert(t.to_string());
            }
        }
        Some(out)
    }

    fn extract_event_types(json: &str) -> Option<HashSet<String>> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        Self::extract_event_types_from_value(&value)
    }

    fn should_accept_structured_events(
        raw_types: Option<HashSet<String>>,
        structured_types: Option<HashSet<String>>,
    ) -> bool {
        let Some(structured) = structured_types else {
            return false;
        };
        match raw_types {
            Some(raw) if !raw.is_empty() => structured.is_subset(&raw),
            _ => false,
        }
    }

    fn split_llm_output(llm_output: &str) -> (&str, &str) {
        if let Some((narrative, events)) = llm_output.split_once("EVENTS:") {
            return (narrative, events);
        }
        let trimmed = llm_output.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            if let Ok(events) = crate::model::llm_decode::decode_llm_events(trimmed) {
                if !events.is_empty() || trimmed == "[]" {
                    return ("", llm_output);
                }
            }
        }
        (llm_output, "[]")
    }

    fn apply_event_and_campaign(&mut self, event: NarrativeEvent) -> EventApplyOutcome {
        let outcome = apply_event(&mut self.game_state, event.clone());
        if matches!(outcome, EventApplyOutcome::Applied) && is_campaign_runtime_event(&event) {
            if let Err(err) = update_active_campaign_runtime_state(&event) {
                self.push_debug_message(format!(
                    "Campaign runtime update failed for {:?}: {}",
                    event, err
                ));
            }
        }
        outcome
    }

pub fn run(&mut self) {
    loop {
        let mut cmd_opt: Option<EngineCommand> = None;
        if self.pending_generation.is_some() {
            match self.rx.try_recv() {
                Ok(cmd) => cmd_opt = Some(cmd),
                Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => {}
            }
        }

        if cmd_opt.is_none() {
            if let Some(pending) = &mut self.pending_generation {
                match pending.response_rx.try_recv() {
                    Ok(result) => {
                        let pending = self.pending_generation.take().expect("pending generation");
                        self.handle_llm_result(pending, result);
                        continue;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        let pending = self.pending_generation.take().expect("pending generation");
                        self.handle_llm_result(
                            pending,
                            Err(anyhow::anyhow!("LLM generation thread disconnected")),
                        );
                        continue;
                    }
                }
            }
        }

        let cmd = if let Some(cmd) = cmd_opt {
            Some(cmd)
        } else if self.pending_generation.is_some() {
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match self.rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => break,
            }
        };

        let Some(cmd) = cmd else {
            continue;
        };

        match cmd {

            /* =========================
               Initialize narrative (world load)
               ========================= */
            EngineCommand::InitializeNarrative { opening_message } => {
                // Reset session
                self.messages.clear();
                self.game_state = InternalGameState::default();
                self.turn_index = 0;
                self.last_quest_offer_source = None;
                self.last_quest_offer_turn = None;

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
                if self.pending_generation.is_some() {
                    self.send_ui_error("Generation already in progress.".to_string());
                    continue;
                }
                self.turn_index = self.turn_index.saturating_add(1);
                let total_start = Instant::now();
                let messages_start = self.messages.len();
                self.game_state.player.exp_multiplier = context.world.exp_multiplier.max(1.0);
                sync_stats_from_context(&mut self.game_state, &context);
                update_action_counts(&mut self.game_state, &text);
                update_power_usage(&mut self.game_state, &text);
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
                            self.send_new_messages_since(messages_start);
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
                        self.send_new_messages_since(messages_start);
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
                        self.send_new_messages_since(messages_start);
                        continue;
                    }
                }

                // 2. Build prompt
                let prompt = PromptBuilder::build(&context, &text);

                // 3. Call LM Studio asynchronously
                let (resp_tx, resp_rx) = std::sync::mpsc::channel();
                let llm_clone = llm.clone();
                thread::spawn(move || {
                    let result = call_llm(prompt, &llm_clone);
                    let _ = resp_tx.send(result);
                });

                self.pending_generation = Some(PendingGeneration {
                    messages_start,
                    text,
                    context,
                    llm,
                    total_start,
                    response_rx: resp_rx,
                    canceled: false,
                });
            }

            EngineCommand::RegenerateLastResponse { text, context, llm } => {
                if self.pending_generation.is_some() {
                    self.send_ui_error("Generation already in progress.".to_string());
                    continue;
                }

                let Some(last_user) = self.trim_messages_after_last_user() else {
                    self.send_ui_error("No user message to regenerate.".to_string());
                    continue;
                };

                if last_user != text {
                    self.push_debug_message(
                        "Warning: last user message changed before regeneration.".to_string(),
                    );
                }

                let total_start = Instant::now();
                let messages_start = self.messages.len();
                self.game_state.player.exp_multiplier = context.world.exp_multiplier.max(1.0);
                sync_stats_from_context(&mut self.game_state, &context);

                let prompt = PromptBuilder::build(&context, &text);

                let (resp_tx, resp_rx) = std::sync::mpsc::channel();
                let llm_clone = llm.clone();
                thread::spawn(move || {
                    let result = call_llm(prompt, &llm_clone);
                    let _ = resp_tx.send(result);
                });

                self.pending_generation = Some(PendingGeneration {
                    messages_start,
                    text,
                    context,
                    llm,
                    total_start,
                    response_rx: resp_rx,
                    canceled: false,
                });
            }

            /* =========================
               UI: Stop generation
               ========================= */
            EngineCommand::StopGeneration => {
                if let Some(mut pending) = self.pending_generation.take() {
                    let llm = pending.llm.clone();
                    if !pending.canceled {
                        pending.canceled = true;
                        self.messages.push(Message::System("Generation stopped.".to_string()));
                        self.send_new_messages_since(pending.messages_start);
                    }
                    thread::spawn(move || {
                        let _ = abort_generation(&llm);
                    });
                }
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
                    id: Some(id),
                    name: Some(name),
                    role: Some(role),
                    details: Some(details),
                    clothing: None,
                    weapons: None,
                    armor: None,
                };

                let outcome = self.apply_event_and_campaign(event.clone());
                let report = NarrativeApplyReport {
                    applications: vec![EventApplication { event, outcome }],
                };
                let snapshot = (&self.game_state).into();

                let _ = self.tx.send(
                    EngineResponse::NarrativeApplied { report, snapshot }
                );
            }

            /* =========================
               UI: Create NPC
               ========================= */
            EngineCommand::CreateNpc { name, role, details } => {
                let details = if details.trim().is_empty() {
                    None
                } else {
                    Some(details)
                };
                let event = crate::model::narrative_event::NarrativeEvent::NpcSpawn {
                    id: None,
                    name,
                    role,
                    details,
                };

                let outcome = self.apply_event_and_campaign(event.clone());
                let report = NarrativeApplyReport {
                    applications: vec![EventApplication { event, outcome }],
                };
                let snapshot = (&self.game_state).into();

                let _ = self.tx.send(
                    EngineResponse::NarrativeApplied { report, snapshot }
                );
            }

            EngineCommand::AddPartyMember {
                name,
                role,
                details,
                weapons,
                armor,
                clothing,
            } => {
                let id = generate_unique_party_id(&self.game_state, &name);
                let event = crate::model::narrative_event::NarrativeEvent::AddPartyMember {
                    id: id.clone(),
                    name: name.clone(),
                    role: role.clone(),
                };
                let outcome = self.apply_event_and_campaign(event.clone());
                if let Some(member) = self.game_state.party.get_mut(&id) {
                    if !details.trim().is_empty() {
                        member.details = details.trim().to_string();
                    }
                    member.weapons = weapons;
                    member.armor = armor;
                    member.clothing = clothing;
                }
                let report = NarrativeApplyReport {
                    applications: vec![EventApplication { event, outcome }],
                };
                let snapshot = (&self.game_state).into();
                let _ = self.tx.send(EngineResponse::NarrativeApplied { report, snapshot });
            }

            EngineCommand::SetPartyMember {
                id,
                name,
                role,
                details,
                weapons,
                armor,
                clothing,
            } => {
                if let Some(member) = self.game_state.party.get(&id) {
                    let (weapons_add, weapons_remove) = diff_lists(&member.weapons, &weapons);
                    let (armor_add, armor_remove) = diff_lists(&member.armor, &armor);
                    let (clothing_add, clothing_remove) = diff_lists(&member.clothing, &clothing);

                    let event = crate::model::narrative_event::NarrativeEvent::PartyUpdate {
                        id: id.clone(),
                        name: Some(name),
                        role: Some(role),
                        details: Some(details),
                        clothing_add: Some(clothing_add),
                        clothing_remove: Some(clothing_remove),
                        weapons_add: Some(weapons_add),
                        weapons_remove: Some(weapons_remove),
                        armor_add: Some(armor_add),
                        armor_remove: Some(armor_remove),
                    };
                    let outcome = self.apply_event_and_campaign(event.clone());
                    let report = NarrativeApplyReport {
                        applications: vec![EventApplication { event, outcome }],
                    };
                    let snapshot = (&self.game_state).into();
                    let _ = self.tx.send(EngineResponse::NarrativeApplied { report, snapshot });
                }
            }

            EngineCommand::RemovePartyMember { id } => {
                if self.game_state.party.remove(&id).is_some() {
                    let report = NarrativeApplyReport { applications: Vec::new() };
                    let snapshot = (&self.game_state).into();
                    let _ = self.tx.send(EngineResponse::NarrativeApplied { report, snapshot });
                }
            }

            EngineCommand::SetPartyMemberLocks {
                id,
                lock_name,
                lock_role,
                lock_details,
                lock_weapons,
                lock_armor,
                lock_clothing,
            } => {
                if let Some(member) = self.game_state.party.get_mut(&id) {
                    member.lock_name = lock_name;
                    member.lock_role = lock_role;
                    member.lock_details = lock_details;
                    member.lock_weapons = lock_weapons;
                    member.lock_armor = lock_armor;
                    member.lock_clothing = lock_clothing;
                }
            }

            EngineCommand::SetTimingEnabled { enabled } => {
                self.timing_enabled = enabled;
            }

            EngineCommand::SetDebugMessagesEnabled { enabled } => {
                self.debug_messages_enabled = enabled;
            }

            EngineCommand::SetNpcRecencyLimit { limit } => {
                self.npc_recency_limit = limit.max(1);
            }

            EngineCommand::GenerateCampaign { config, llm } => {
                if self.pending_generation.is_some() {
                    self.send_ui_error("Cannot generate campaign while response generation is in progress.".to_string());
                    continue;
                }

                let prompt = build_campaign_generation_prompt(&config);
                match call_llm(prompt, &llm) {
                    Ok(output) => {
                        match parse_campaign_blueprint(&output)
                            .and_then(|blueprint| validate_campaign_blueprint(&blueprint, &config).map(|_| blueprint))
                        {
                            Ok(blueprint) => {
                                match save_campaign_package(&blueprint, &config) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        self.send_ui_error(format!(
                                            "Campaign generation validated but package save failed: {}",
                                            err
                                        ));
                                    }
                                }
                            }
                            Err(err) => {
                                self.send_ui_error(format!(
                                    "Campaign generation failed validation: {}",
                                    err
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        self.send_ui_error(format!("Campaign generation failed: {}", err));
                    }
                }
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
                save_chat_log,
                character_image_rgba,
                character_image_size,
            } => {
                let messages_start = self.messages.len();
                let save = GameSave {
                    version: SAVE_VERSION,
                    world,
                    player,
                    party,
                    messages: self.messages.clone(),
                    internal_state: self.game_state.clone(),
                    speaker_colors,
                    character_image_rgba,
                    character_image_size,
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

                if save_chat_log {
                    let log_path = path.with_extension("log.txt");
                    if let Err(err) = fs::write(&log_path, self.format_chat_log()) {
                        self.messages.push(Message::System(format!(
                            "Failed to save chat log: {}",
                            err
                        )));
                    }
                }

                self.send_new_messages_since(messages_start);
            }

            EngineCommand::LoadGame { path } => {
                let result = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|data| serde_json::from_str::<GameSave>(&data).map_err(|e| e.to_string()));

                match result {
                    Ok(mut save) => {
                        migrate_save(&mut save);
                        self.messages = save.messages.clone();
                        self.game_state = save.internal_state.clone();
                        let snapshot = (&self.game_state).into();

                        let _ = self.tx.send(
                            EngineResponse::GameLoaded { save, snapshot }
                        );

                    }
                    Err(err) => {
                        let messages_start = self.messages.len();
                        self.messages.push(Message::System(format!(
                            "Failed to load game: {}",
                            err
                        )));
                        self.send_new_messages_since(messages_start);
                    }
                }
            }

        }
    }
    }

    fn emit_timing(
        &mut self,
        tag: &str,
        total_start: Instant,
        split_done: Instant,
        parse_done: Instant,
        narrative_done: Instant,
        apply_done: Instant,
        snapshot_done: Instant,
        followup: Option<(Instant, Instant, Instant)>,
    ) {
        if !self.timing_enabled {
            return;
        }

        let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;
        let split_ms = split_done.duration_since(total_start).as_secs_f64() * 1000.0;
        let parse_ms = parse_done.duration_since(split_done).as_secs_f64() * 1000.0;
        let narrative_ms = narrative_done.duration_since(parse_done).as_secs_f64() * 1000.0;
        let apply_ms = apply_done.duration_since(narrative_done).as_secs_f64() * 1000.0;
        let snapshot_ms = snapshot_done.duration_since(apply_done).as_secs_f64() * 1000.0;

        let mut msg = format!(
            "[timing:{}] total={:.2}ms split={:.2}ms parse={:.2}ms narrative={:.2}ms apply={:.2}ms snapshot={:.2}ms",
            tag, total_ms, split_ms, parse_ms, narrative_ms, apply_ms, snapshot_ms
        );

        if let Some((followup_start, followup_split_done, followup_parse_done)) = followup {
            let followup_total = followup_start.elapsed().as_secs_f64() * 1000.0;
            let followup_split =
                followup_split_done.duration_since(followup_start).as_secs_f64() * 1000.0;
            let followup_parse = followup_parse_done
                .duration_since(followup_split_done)
                .as_secs_f64()
                * 1000.0;
            msg.push_str(&format!(
                " followup_total={:.2}ms followup_split={:.2}ms followup_parse={:.2}ms",
                followup_total, followup_split, followup_parse
            ));
        }

        self.messages.push(Message::System(msg));
    }

    fn handle_llm_result(
        &mut self,
        pending: PendingGeneration,
        result: anyhow::Result<String>,
    ) {
        if pending.canceled {
            return;
        }

        let PendingGeneration {
            messages_start,
            text,
            context,
            llm,
            total_start,
            ..
        } = pending;

        let llm_output = match result {
            Ok(text) => text,
            Err(e) => {
                self.messages.push(Message::System(format!(
                    "LLM error: {}",
                    e
                )));
                self.send_ui_error(format!("LLM error: {}", e));
                self.send_new_messages_since(messages_start);
                return;
            }
        };

        let current_turn = self.turn_index;

        // 4. Split NARRATIVE vs EVENTS
        let (narrative, events_json) = Self::split_llm_output(&llm_output);
        let split_done = Instant::now();

        let use_structured_events =
            llm.use_structured_events && matches!(llm.api_mode, crate::engine::llm_client::LlmApiMode::OpenAiChat);

        // 5. Decode EVENTS JSON (raw) for request_context detection
        let raw_events = if use_structured_events {
            crate::model::llm_decode::decode_llm_events(events_json).unwrap_or_default()
        } else {
            match crate::model::llm_decode::decode_llm_events(events_json) {
                Ok(events) => events,
                Err(err) => {
                    self.push_debug_message(format!("Failed to parse EVENTS: {}", err));
                    self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                    Vec::new()
                }
            }
        };
        let parse_done = Instant::now();

        let structured_events_json = if use_structured_events {
            let raw_types = Self::extract_event_types(events_json);
            if events_json.trim().is_empty() || events_json.trim() == "[]" {
                None
            } else {
                match call_llm_events_structured(narrative, events_json, &llm) {
                    Ok(json) => {
                        let structured_types = Self::extract_event_types(&json);
                        if Self::should_accept_structured_events(raw_types, structured_types) {
                            Some(json)
                        } else {
                            let warning = "Structured EVENTS added new event types; using raw EVENTS instead.";
                            self.push_debug_message(warning.to_string());
                            self.send_ui_error(warning.to_string());
                            None
                        }
                    }
                    Err(err) => {
                        let warning =
                            format!("Structured EVENTS failed, using raw EVENTS: {}", err);
                        self.push_debug_message(warning.clone());
                        self.send_ui_error(warning);
                        None
                    }
                }
            }
        } else {
            None
        };

        let events_json = structured_events_json.as_deref().unwrap_or(events_json);

        let events = match crate::model::llm_decode::decode_llm_events(events_json) {
            Ok(events) => events,
            Err(err) => {
                self.push_debug_message(format!("Failed to parse EVENTS: {}", err));
                self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                Vec::new()
            }
        };

        // 6. Handle request_context (one additional round)
        if let Some(topics) = collect_requested_topics(&raw_events) {
            let followup_start = Instant::now();
            let requested_context = build_requested_context(
                &self.game_state,
                &context,
                &topics,
            );
            let recent_history = tail_messages(&self.messages, 5);
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
                    self.send_ui_error(format!("LLM error: {}", e));
                    self.send_new_messages_since(messages_start);
                    return;
                }
            };

            let (narrative, events_json) = Self::split_llm_output(&llm_output);
            let followup_split_done = Instant::now();
            let raw_events = if use_structured_events {
                crate::model::llm_decode::decode_llm_events(events_json).unwrap_or_default()
            } else {
                match crate::model::llm_decode::decode_llm_events(events_json) {
                    Ok(events) => events,
                    Err(err) => {
                        self.push_debug_message(format!("Failed to parse EVENTS: {}", err));
                        self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                        Vec::new()
                    }
                }
            };
            let followup_parse_done = Instant::now();

            let structured_events_json = if use_structured_events {
                let raw_types = Self::extract_event_types(events_json);
                if events_json.trim().is_empty() || events_json.trim() == "[]" {
                    None
                } else {
                    match call_llm_events_structured(narrative, events_json, &llm) {
                        Ok(json) => {
                            let structured_types = Self::extract_event_types(&json);
                            if Self::should_accept_structured_events(raw_types, structured_types) {
                                Some(json)
                            } else {
                                let warning = "Structured EVENTS added new event types; using raw EVENTS instead.";
                                self.push_debug_message(warning.to_string());
                                self.send_ui_error(warning.to_string());
                                None
                            }
                        }
                        Err(err) => {
                            let warning =
                                format!("Structured EVENTS failed, using raw EVENTS: {}", err);
                            self.push_debug_message(warning.clone());
                            self.send_ui_error(warning);
                            None
                        }
                    }
                }
            } else {
                None
            };

            let events_json = structured_events_json.as_deref().unwrap_or(events_json);
            let events = match crate::model::llm_decode::decode_llm_events(events_json) {
                Ok(events) => events,
                Err(err) => {
                    self.push_debug_message(format!("Failed to parse EVENTS: {}", err));
                    self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                    Vec::new()
                }
            };

            let start_level = self.game_state.player.level;
            let had_redundant_context = raw_events
                .iter()
                .any(|e| matches!(e, NarrativeEvent::RequestContext { .. }));
            if had_redundant_context {
                let warning = "LLM requested context again; showing narrative only. Consider regenerating or switching models.";
                self.messages
                    .push(Message::System(warning.to_string()));
                self.send_ui_error(warning.to_string());
            }
            let events: Vec<_> = events
                .into_iter()
                .filter(|e| !matches!(e, NarrativeEvent::RequestContext { .. }))
                .collect();

            let new_messages = parse_narrative(narrative);
            self.messages.extend(new_messages);
            let proximity_changed =
                self.update_npc_proximity_from_recent_messages(self.npc_recency_limit);
            let narrative_done = Instant::now();

            let mut applications = Vec::new();
            let offer_source = quest_offer_source(narrative);
            if let Some(source) = offer_source {
                self.last_quest_offer_source = Some(source);
                self.last_quest_offer_turn = Some(current_turn);
            }
            let player_accepts = player_accepts_quest(&text);
            let mut effective_offer_source = offer_source;
            if effective_offer_source.is_none() && player_accepts {
                if let (Some(source), Some(turn)) =
                    (self.last_quest_offer_source, self.last_quest_offer_turn)
                {
                    if turn + 1 == current_turn {
                        effective_offer_source = Some(source);
                    }
                }
            }
            for event in events {
                if let NarrativeEvent::StartQuest { .. } = event {
                    if let Some(reason) =
                        validate_start_quest(
                            &event,
                            effective_offer_source,
                            player_accepts,
                            &context.world,
                        )
                    {
                        applications.push(EventApplication {
                            event,
                            outcome: EventApplyOutcome::Deferred { reason },
                        });
                        continue;
                    }
                }
                if let NarrativeEvent::PartyUpdate { .. } = event {
                    if !player_requested_party_details(&text) {
                        applications.push(EventApplication {
                            event,
                            outcome: EventApplyOutcome::Deferred {
                                reason: "Party update ignored: player did not request details.".to_string(),
                            },
                        });
                        continue;
                    }
                    let sanitized = sanitize_party_update(&event);
                    let outcome = self.apply_event_and_campaign(sanitized.clone());
                    applications.push(EventApplication {
                        event: sanitized,
                        outcome,
                    });
                    continue;
                }
                let outcome = self.apply_event_and_campaign(event.clone());
                applications.push(EventApplication { event, outcome });
            }

            maybe_grant_repetition_power(
                &mut self.game_state,
                &text,
                &context.world,
                &mut applications,
            );
            maybe_evolve_powers(&mut self.game_state, &context.world, &mut applications);
            apply_set_bonuses(&mut self.game_state, &mut applications);
            apply_level_stat_growth(
                &mut self.game_state,
                &context,
                start_level,
                &mut applications,
            );
            let apply_done = Instant::now();

            if !applications.is_empty() || proximity_changed {
                let report = NarrativeApplyReport { applications };
                let snapshot = (&self.game_state).into();
                let _ = self.tx.send(
                    EngineResponse::NarrativeApplied { report, snapshot }
                );
                let snapshot_done = Instant::now();
                self.emit_timing(
                    "followup",
                    total_start,
                    split_done,
                    parse_done,
                    narrative_done,
                    apply_done,
                    snapshot_done,
                    Some((followup_start, followup_split_done, followup_parse_done)),
                );
            } else {
                self.emit_timing(
                    "followup",
                    total_start,
                    split_done,
                    parse_done,
                    narrative_done,
                    apply_done,
                    Instant::now(),
                    Some((followup_start, followup_split_done, followup_parse_done)),
                );
            }

            self.send_new_messages_since(messages_start);
            return;
        }

        // 7. Parse narrative into structured messages
            let new_messages = parse_narrative(narrative);
            self.messages.extend(new_messages);
            let proximity_changed =
                self.update_npc_proximity_from_recent_messages(self.npc_recency_limit);
            let narrative_done = Instant::now();

        // 8. Apply events
        let mut applications = Vec::new();
        let offer_source = quest_offer_source(narrative);
        if let Some(source) = offer_source {
            self.last_quest_offer_source = Some(source);
            self.last_quest_offer_turn = Some(current_turn);
        }
        let player_accepts = player_accepts_quest(&text);
        let mut effective_offer_source = offer_source;
        if effective_offer_source.is_none() && player_accepts {
            if let (Some(source), Some(turn)) =
                (self.last_quest_offer_source, self.last_quest_offer_turn)
            {
                if turn + 1 == current_turn {
                    effective_offer_source = Some(source);
                }
            }
        }
        let start_level = self.game_state.player.level;

        for event in events {
            if let NarrativeEvent::StartQuest { .. } = event {
                if let Some(reason) =
                    validate_start_quest(
                        &event,
                        effective_offer_source,
                        player_accepts,
                        &context.world,
                    )
                {
                    applications.push(EventApplication {
                        event,
                        outcome: EventApplyOutcome::Deferred { reason },
                    });
                    continue;
                }
            }
            if let NarrativeEvent::PartyUpdate { .. } = event {
                if !player_requested_party_details(&text) {
                    applications.push(EventApplication {
                        event,
                        outcome: EventApplyOutcome::Deferred {
                            reason: "Party update ignored: player did not request details.".to_string(),
                        },
                    });
                    continue;
                }
                let sanitized = sanitize_party_update(&event);
                let outcome = self.apply_event_and_campaign(sanitized.clone());
                applications.push(EventApplication {
                    event: sanitized,
                    outcome,
                });
                continue;
            }
            let outcome = self.apply_event_and_campaign(event.clone());
            applications.push(EventApplication {
                event,
                outcome,
            });
        }

        maybe_grant_repetition_power(
            &mut self.game_state,
            &text,
            &context.world,
            &mut applications,
        );
        maybe_evolve_powers(&mut self.game_state, &context.world, &mut applications);
        apply_set_bonuses(&mut self.game_state, &mut applications);
        apply_level_stat_growth(
            &mut self.game_state,
            &context,
            start_level,
            &mut applications,
        );
        let apply_done = Instant::now();

        // 9. Send state mutation report
        if !applications.is_empty() || proximity_changed {
            let report = NarrativeApplyReport { applications };
            let snapshot = (&self.game_state).into();

            let _ = self.tx.send(
                EngineResponse::NarrativeApplied {
                    report,
                    snapshot,
                }
            );
            let snapshot_done = Instant::now();
            self.emit_timing(
                "primary",
                total_start,
                split_done,
                parse_done,
                narrative_done,
                apply_done,
                snapshot_done,
                None,
            );
        } else {
            self.emit_timing(
                "primary",
                total_start,
                split_done,
                parse_done,
                narrative_done,
                apply_done,
                Instant::now(),
                None,
            );
        }

        // 10. Update UI with full history
        self.send_new_messages_since(messages_start);
    }

    fn send_new_messages_since(&self, start_len: usize) {
        if self.messages.len() <= start_len {
            return;
        }
        let _ = self.tx.send(EngineResponse::AppendMessages(
            self.messages[start_len..].to_vec(),
        ));
    }

    fn format_chat_log(&self) -> String {
        let mut out = String::new();
        for msg in &self.messages {
            match msg {
                Message::User(text) => {
                    out.push_str("You: ");
                    out.push_str(text);
                }
                Message::Roleplay { speaker, text } => {
                    let label = match speaker {
                        crate::model::message::RoleplaySpeaker::Narrator => "Narrator",
                        crate::model::message::RoleplaySpeaker::Npc => "NPC",
                        crate::model::message::RoleplaySpeaker::PartyMember => "Party",
                    };
                    out.push_str(label);
                    out.push_str(": ");
                    out.push_str(text);
                }
                Message::System(text) => {
                    out.push_str("System: ");
                    out.push_str(text);
                }
            }
            out.push('\n');
        }
        out
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
                push_section(&mut out, "PLAYER", &format_player_state(state, context));
            }
            "stats" => {
                push_section(&mut out, "STATS", &format_state_stats(state));
            }
            "exp" | "experience" | "level" => {
                push_section(&mut out, "EXP", &format_exp(state));
            }
            "powers" => {
                push_section(&mut out, "POWERS", &format_powers(state, context));
            }
            "features" => {
                push_section(&mut out, "FEATURES", &format_list(&context.player.features));
            }
            "inventory" => {
                push_section(&mut out, "INVENTORY", &format_inventory(state));
            }
            "equipment" | "equipped" => {
                push_section(&mut out, "EQUIPMENT", &format_equipment(state));
                push_section(&mut out, "SET BONUSES", &format_set_bonuses(state));
            }
            "sets" | "set_bonuses" => {
                push_section(&mut out, "SET BONUSES", &format_set_bonuses(state));
            }
            "crafting" | "gathering" => {
                push_section(&mut out, "CRAFTING", &format_crafting_rules(context));
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
            "factions" | "reputation" | "rep" => {
                push_section(&mut out, "FACTIONS", &format_factions(state));
            }
            "npcs" => {
                push_section(&mut out, "NPCS", &format_npcs(state));
            }
            "locations" | "location" => {
                push_section(&mut out, "LOCATIONS", &load_locations_context());
            }
            "campaign" | "campaign_context" => {
                push_section(&mut out, "CAMPAIGN", &load_campaign_context("campaign"));
            }
            "campaign_manifest" | "campaign_meta" => {
                push_section(&mut out, "CAMPAIGN MANIFEST", &load_campaign_context("campaign_manifest"));
            }
            "campaign_state" => {
                push_section(&mut out, "CAMPAIGN STATE", &load_campaign_context("campaign_state"));
            }
            "campaign_timeline" => {
                push_section(&mut out, "CAMPAIGN TIMELINE", &load_campaign_context("campaign_timeline"));
            }
            "campaign_factions" => {
                push_section(&mut out, "CAMPAIGN FACTIONS", &load_campaign_context("campaign_factions"));
            }
            "campaign_npcs" => {
                push_section(&mut out, "CAMPAIGN NPCS", &load_campaign_context("campaign_npcs"));
            }
            "campaign_quests" => {
                push_section(&mut out, "CAMPAIGN QUESTS", &load_campaign_context("campaign_quests"));
            }
            "campaign_bosses" => {
                push_section(&mut out, "CAMPAIGN BOSSES", &load_campaign_context("campaign_bosses"));
            }
            "campaign_threats" | "campaign_roaming_threats" => {
                push_section(&mut out, "CAMPAIGN THREATS", &load_campaign_context("campaign_threats"));
            }
            "campaign_index" => {
                push_section(&mut out, "CAMPAIGN INDEX", &load_campaign_context("campaign_index"));
            }
            "relationships" => {
                push_section(&mut out, "RELATIONSHIPS", &format_relationships(state));
            }
            "skills" | "skill_rules" | "repetition" => {
                push_section(&mut out, "SKILL PROGRESSION", &format_skill_rules(context));
            }
            "power_evolution" | "power_evolution_rules" => {
                push_section(&mut out, "POWER EVOLUTION", &format_power_evolution_rules(context));
            }
            "flags" => {
                push_section(&mut out, "FLAGS", &format_flags(state));
            }
            "slaves" | "property" | "bonded_servants" | "concubines" | "harem_members"
            | "prisoners" | "npcs_on_mission" => {
                push_section(
                    &mut out,
                    "OPTIONAL TAB",
                    &format_section_cards(state, topic),
                );
            }
            "player_card" => {
                push_section(&mut out, "PLAYER CARD", &format_player_card(state));
            }
            "time" | "clock" | "world_time" => {
                push_section(&mut out, "TIME", &format_time(state));
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

fn load_locations_context() -> String {
    let path = std::path::Path::new("data/locations.json");
    match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(err) => format!(
            "No locations file available at {} ({})",
            path.display(),
            err
        ),
    }
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
    s.push_str("Experience Rules:\n");
    s.push_str(&format_exp_rules(context));
    s.push_str("Skill Progression:\n");
    s.push_str(&format_skill_rules(context));
    s.push_str("Power Evolution:\n");
    s.push_str(&format_power_evolution_rules(context));
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

fn format_exp_rules(context: &crate::model::game_context::GameContext) -> String {
    let mult = context.world.exp_multiplier.max(1.0);
    format!(
        "Base EXP to reach level 2 is 100.\nEach next level multiplies by x{}.\n",
        trim_multiplier(mult)
    )
}

fn format_skill_rules(context: &crate::model::game_context::GameContext) -> String {
    let base = context.world.repetition_threshold.max(1);
    let step = context.world.repetition_tier_step.max(1);
    let mut s = format!(
        "Base threshold: {} repeats.\nEach tier increases by +{} repeats.\n",
        base, step
    );
    let names = normalized_tier_names(&context.world.skill_tier_names);
    s.push_str(&format!(
        "Tiers: {}, {}, {}, {}, {}.\n",
        names[0], names[1], names[2], names[3], names[4]
    ));
    if !context.world.skill_thresholds.is_empty() {
        s.push_str("Overrides:\n");
        for entry in &context.world.skill_thresholds {
            let skill = entry.skill.trim();
            if skill.is_empty() {
                continue;
            }
            let tier_names = normalized_tier_names(&entry.tier_names);
            s.push_str(&format!(
                "- {}: base {}, step {}, tiers: {}, {}, {}, {}, {}\n",
                skill,
                entry.base.max(1),
                entry.step.max(1),
                tier_names[0],
                tier_names[1],
                tier_names[2],
                tier_names[3],
                tier_names[4]
            ));
        }
    }
    s
}

fn format_crafting_rules(context: &crate::model::game_context::GameContext) -> String {
    let loot = format_loot_rules(context);
    format!(
        "Crafting and gathering must follow loot rules.\n{}",
        loot
    )
}

fn format_power_evolution_rules(context: &crate::model::game_context::GameContext) -> String {
    let base = context.world.power_evolution_base.max(1);
    let step = context.world.power_evolution_step.max(1);
    let min_mult = context.world.power_evolution_multiplier_min.max(1.0);
    let max_mult = context
        .world
        .power_evolution_multiplier_max
        .max(min_mult);
    format!(
        "Base uses: {}. Tier step: {}. Multiplier range: x{}–x{}.\n",
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

fn format_player_state(
    state: &InternalGameState,
    context: &crate::model::game_context::GameContext,
) -> String {
    let p = &context.player;
    let s = &state.player;
    format!(
        "Name: {}\nClass: {}\nLevel: {}\nEXP: {}/{}\nHP: {}/{}\nBackground:\n{}\n",
        p.name,
        p.class,
        s.level,
        s.exp,
        s.exp_to_next,
        s.hp,
        s.max_hp,
        p.background
    )
}

fn format_exp(state: &InternalGameState) -> String {
    let s = &state.player;
    format!(
        "EXP: {}/{}\nLevel: {}\nEXP to next level: {}\n",
        s.exp, s.exp_to_next, s.level, s.exp_to_next
    )
}

fn format_state_stats(state: &InternalGameState) -> String {
    if state.stats.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for (k, v) in &state.stats {
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

fn format_powers(
    state: &InternalGameState,
    context: &crate::model::game_context::GameContext,
) -> String {
    if !state.powers.is_empty() {
        let mut powers: Vec<_> = state.powers.values().collect();
        powers.sort_by(|a, b| a.name.cmp(&b.name));
        let mut s = String::new();
        for power in powers {
            if power.description.trim().is_empty() {
                s.push_str(&format!("- {}\n", power.name));
            } else {
                s.push_str(&format!("- {}: {}\n", power.name, power.description));
            }
        }
        return s;
    }

    format_power_entries(&context.player.powers)
}

fn format_power_entries(powers: &[crate::ui::app::PowerEntry]) -> String {
    if powers.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for power in powers {
        let name = power.name.trim();
        if name.is_empty() {
            continue;
        }
        let desc = power.description.trim();
        if desc.is_empty() {
            s.push_str(&format!("- {}\n", name));
        } else {
            s.push_str(&format!("- {}: {}\n", name, desc));
        }
    }
    s
}

fn format_inventory(state: &InternalGameState) -> String {
    if state.inventory.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for item in state.inventory.values() {
        let label = if item.quantity <= 1 {
            format!("- {}", item.id)
        } else {
            format!("- {} x{}", item.id, item.quantity)
        };
        if let Some(set_id) = &item.set_id {
            s.push_str(&format!("{} (set: {})\n", label, set_id));
        } else {
            s.push_str(&format!("{}\n", label));
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

fn format_equipment(state: &InternalGameState) -> String {
    if state.equipment.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for item in state.equipment.values() {
        let set_label = item
            .set_id
            .as_ref()
            .map(|v| format!(" (set: {})", v))
            .unwrap_or_default();
        s.push_str(&format!(
            "- {} [{}]{}\n",
            item.item_id, item.slot, set_label
        ));
        if let Some(desc) = &item.description {
            let trimmed = desc.trim();
            if !trimmed.is_empty() {
                s.push_str(&format!("  {}\n", trimmed));
            }
        }
    }
    s
}

fn format_set_bonuses(state: &InternalGameState) -> String {
    if state.equipment.is_empty() {
        return "None\n".to_string();
    }
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for item in state.equipment.values() {
        let Some(set_id) = &item.set_id else { continue };
        let entry = counts.entry(set_id.clone()).or_insert(0);
        *entry = entry.saturating_add(1);
    }
    if counts.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for (set_id, count) in counts {
        let tier = if count >= 4 { 2 } else if count >= 2 { 1 } else { 0 };
        let tier_label = match tier {
            2 => "major",
            1 => "minor",
            _ => "none",
        };
        s.push_str(&format!(
            "- {}: {} pieces ({} bonus)\n",
            set_id, count, tier_label
        ));
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
        if !member.details.trim().is_empty() {
            s.push_str(&format!("  Details: {}\n", member.details.trim()));
        }
        if !member.clothing.is_empty() {
            s.push_str("  Clothing:\n");
            for item in &member.clothing {
                s.push_str(&format!("  - {}\n", item));
            }
        }
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
        if let Some(diff) = &quest.difficulty {
            if !diff.trim().is_empty() {
                s.push_str(&format!("  Difficulty: {}\n", diff.trim()));
            }
        }
        if quest.negotiable {
            s.push_str("  Negotiable rewards: yes\n");
        }
        if !quest.reward_options.is_empty() {
            s.push_str("  Reward options:\n");
            for opt in &quest.reward_options {
                s.push_str(&format!("  - {}\n", opt));
            }
        }
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
        let status = if npc.nearby { "nearby" } else { "away" };
        s.push_str(&format!("- {} ({}) [{}]\n", npc.name, npc.role, status));
    }
    s
}

fn format_section_cards(state: &InternalGameState, section: &str) -> String {
    let Some(cards) = state.sections.get(section) else {
        return "None\n".to_string();
    };
    if cards.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for card in cards {
        s.push_str(&format!("- {} ({})\n", card.name, card.role));
        if !card.status.trim().is_empty() {
            s.push_str(&format!("  Status: {}\n", card.status.trim()));
        }
        if !card.details.trim().is_empty() {
            s.push_str(&format!("  Details: {}\n", card.details.trim()));
        }
        if !card.notes.trim().is_empty() {
            s.push_str(&format!("  Notes: {}\n", card.notes.trim()));
        }
        if !card.tags.is_empty() {
            s.push_str("  Tags:\n");
            for tag in &card.tags {
                s.push_str(&format!("  - {}\n", tag));
            }
        }
        if !card.items.is_empty() {
            s.push_str("  Items:\n");
            for item in &card.items {
                s.push_str(&format!("  - {}\n", item));
            }
        }
    }
    s
}

fn format_player_card(state: &InternalGameState) -> String {
    let Some(card) = state.player_card.as_ref() else {
        return "None\n".to_string();
    };
    let mut s = String::new();
    s.push_str(&format!("- {} ({})\n", card.name, card.role));
    if !card.status.trim().is_empty() {
        s.push_str(&format!("  Status: {}\n", card.status.trim()));
    }
    if !card.details.trim().is_empty() {
        s.push_str(&format!("  Details: {}\n", card.details.trim()));
    }
    if !card.notes.trim().is_empty() {
        s.push_str(&format!("  Notes: {}\n", card.notes.trim()));
    }
    if !card.tags.is_empty() {
        s.push_str("  Tags:\n");
        for tag in &card.tags {
            s.push_str(&format!("  - {}\n", tag));
        }
    }
    if !card.items.is_empty() {
        s.push_str("  Items:\n");
        for item in &card.items {
            s.push_str(&format!("  - {}\n", item));
        }
    }
    s
}

fn format_time(state: &InternalGameState) -> String {
    let total_minutes = state.world_time_minutes;
    let days = total_minutes / (24 * 60);
    let hours = (total_minutes / 60) % 24;
    let minutes = total_minutes % 60;
    format!(
        "Elapsed time: {} days, {:02}:{:02}\n",
        days, hours, minutes
    )
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

fn format_factions(state: &InternalGameState) -> String {
    if state.factions.is_empty() {
        return "None\n".to_string();
    }
    let mut s = String::new();
    for faction in state.factions.values() {
        let kind = faction.kind.clone().unwrap_or_else(|| "unknown".to_string());
        s.push_str(&format!(
            "- {} ({}) rep: {}\n",
            faction.name, kind, faction.reputation
        ));
        if let Some(desc) = &faction.description {
            let trimmed = desc.trim();
            if !trimmed.is_empty() {
                s.push_str(&format!("  {}\n", trimmed));
            }
        }
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
                    set_id: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(drop.quantity);
            if entry.description.is_none() {
                entry.description = drop.description.clone();
            }
            if entry.set_id.is_none() {
                entry.set_id = drop.set_id.clone();
            }

            moved_labels.push(format!("{} x{}", drop.item, drop.quantity));
            applications.push(EventApplication {
                event: NarrativeEvent::AddItem {
                    item_id: drop.item,
                    quantity: drop.quantity,
                    set_id: drop.set_id,
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
    let n = normalize_phrase(narrative);
    if n.contains("the world is offering you a quest") {
        return Some(QuestOfferSource::World);
    }
    if n.contains("i hereby offer you a quest") {
        if n.contains("[npc") {
            return Some(QuestOfferSource::Npc);
        }
        if !looks_like_hostile_offer(&n) {
            return Some(QuestOfferSource::Npc);
        }
        return Some(QuestOfferSource::Npc);
    }
    None
}

fn looks_like_hostile_offer(normalized: &str) -> bool {
    let hostile = [
        "attacks",
        "attack",
        "lunges",
        "swings",
        "strikes",
        "slashes",
        "bites",
        "mauls",
        "charges",
        "roars",
        "hostile",
        "bloodthirsty",
        "feral",
        "enemy",
        "ambush",
    ];
    hostile.iter().any(|k| normalized.contains(k))
}

fn build_campaign_generation_prompt(
    config: &crate::engine::protocol::CampaignGenerationConfig,
) -> String {
    let scope = [
        ("timeline", config.include_timeline),
        ("npcs", config.include_npcs),
        ("factions", config.include_factions),
        ("quest_lines", config.include_quest_lines),
        ("world_bosses", config.include_world_bosses),
        ("roaming_threats", config.include_roaming_threats),
    ]
    .iter()
    .filter_map(|(k, v)| if *v { Some(*k) } else { None })
    .collect::<Vec<_>>()
    .join(", ");

    let mut prompt = String::new();
    prompt.push_str(
        "You are a campaign architect. Generate a coherent RPG campaign blueprint as strict JSON.\n\
Return JSON only. Do not include markdown.\n\n",
    );
    prompt.push_str("Required JSON shape:\n");
    prompt.push_str(
        "{\n\
  \"summary\": string,\n\
  \"timeline\": [ { \"chapter\": number, \"title\": string, \"beats\": [string] } ],\n\
  \"factions\": [ { \"id\": string, \"name\": string, \"goal\": string, \"methods\": [string], \"allies\": [string], \"rivals\": [string] } ],\n\
  \"npcs\": [ { \"id\": string, \"name\": string, \"faction_id\": string, \"role\": string, \"motivation\": string, \"secrets\": [string] } ],\n\
  \"quest_lines\": [ { \"id\": string, \"title\": string, \"chapters\": [number], \"steps\": [string], \"rewards\": [string], \"depends_on\": [string] } ],\n\
  \"world_bosses\": [ { \"id\": string, \"name\": string, \"chapter\": number, \"faction_id\": string, \"arena\": string, \"mechanics\": [string], \"drop_table\": [string] } ],\n\
  \"roaming_threats\": [ { \"id\": string, \"name\": string, \"regions\": [string], \"behavior\": string, \"danger\": string } ],\n\
  \"consistency_notes\": [string]\n\
}\n\n",
    );

    prompt.push_str("Generation constraints:\n");
    prompt.push_str(&format!("- scope: {}\n", scope));
    prompt.push_str(&format!("- chapters: {}\n", config.chapters));
    prompt.push_str(&format!("- factions: {}\n", config.faction_count));
    prompt.push_str(&format!("- world_bosses: {}\n", config.world_boss_count));
    prompt.push_str(&format!("- npc_density: {}\n", config.npc_density));
    prompt.push_str(&format!("- passes: {}\n", config.passes));
    prompt.push_str(&format!(
        "- run_consistency_pass: {}\n",
        config.run_consistency_pass
    ));
    prompt.push_str(&format!("- core_tone: {}\n", config.core_tone));
    prompt.push_str(&format!("- narrative_style: {}\n", config.narrative_style));
    prompt.push_str(&format!("- intensity: {}\n", config.intensity));

    if !config.theme_tags.is_empty() {
        prompt.push_str(&format!("- theme_tags: {}\n", config.theme_tags.join(", ")));
    }
    if !config.exclude_tags.is_empty() {
        prompt.push_str(&format!(
            "- taboo_exclusions: {}\n",
            config.exclude_tags.join(", ")
        ));
    }
    if !config.include_tags.is_empty() {
        prompt.push_str(&format!(
            "- explicitly_allowed_dark_content: {}\n",
            config.include_tags.join(", ")
        ));
    }
    if !config.custom_dark_tags.is_empty() {
        prompt.push_str(&format!(
            "- custom_dark_choices: {}\n",
            config.custom_dark_tags.join(", ")
        ));
    }

    prompt.push_str(
        "\nRules:\n\
- Keep internal consistency across timeline, factions, npcs, and quest dependencies.\n\
- Ensure every faction has distinct strategy and pressure on the world.\n\
- Ensure quest_lines and bosses tie into chapter progression.\n\
- Never include content listed in taboo_exclusions.\n\
- You may include explicitly_allowed_dark_content and custom_dark_choices.\n\
- Return JSON only.\n",
    );

    prompt
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignBlueprint {
    summary: String,
    timeline: Vec<CampaignTimelineEntry>,
    factions: Vec<CampaignFaction>,
    npcs: Vec<CampaignNpc>,
    quest_lines: Vec<CampaignQuestLine>,
    world_bosses: Vec<CampaignWorldBoss>,
    roaming_threats: Vec<CampaignRoamingThreat>,
    consistency_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignTimelineEntry {
    chapter: u32,
    title: String,
    beats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignFaction {
    id: String,
    name: String,
    goal: String,
    methods: Vec<String>,
    allies: Vec<String>,
    rivals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignNpc {
    id: String,
    name: String,
    faction_id: String,
    role: String,
    motivation: String,
    secrets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignQuestLine {
    id: String,
    title: String,
    chapters: Vec<u32>,
    steps: Vec<String>,
    rewards: Vec<String>,
    depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignWorldBoss {
    id: String,
    name: String,
    chapter: u32,
    faction_id: String,
    arena: String,
    mechanics: Vec<String>,
    drop_table: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignRoamingThreat {
    id: String,
    name: String,
    regions: Vec<String>,
    behavior: String,
    danger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignManifest {
    campaign_id: String,
    summary: String,
    chapters: u32,
    generated_unix: u64,
    scope: Vec<String>,
    core_tone: String,
    narrative_style: String,
    intensity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignIndex {
    manifest: String,
    state: String,
    timeline: Vec<String>,
    factions: Vec<String>,
    npcs: Vec<String>,
    quest_lines: Vec<String>,
    world_bosses: Vec<String>,
    roaming_threats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignRuntimeState {
    current_chapter: u32,
    #[serde(default)]
    current_phase: String,
    revealed_factions: Vec<String>,
    revealed_npcs: Vec<String>,
    revealed_quests: Vec<String>,
    #[serde(default)]
    revealed_world_bosses: Vec<String>,
    defeated_world_bosses: Vec<String>,
    seen_roaming_threats: Vec<String>,
    #[serde(default)]
    campaign_flags: Vec<String>,
    #[serde(default)]
    active_quest_lines: Vec<String>,
    #[serde(default)]
    completed_quest_lines: Vec<String>,
    #[serde(default)]
    failed_quest_lines: Vec<String>,
    #[serde(default)]
    world_boss_states: std::collections::HashMap<String, String>,
}

#[derive(Debug)]
struct ActiveCampaignBundle {
    root: std::path::PathBuf,
    manifest: CampaignManifest,
    state: CampaignRuntimeState,
    index: CampaignIndex,
}

fn parse_campaign_blueprint(raw: &str) -> Result<CampaignBlueprint, String> {
    let normalized = normalize_campaign_json(raw);
    if normalized.trim().is_empty() {
        return Err("Empty campaign response.".to_string());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(&normalized).or_else(|primary_err| {
        if let Some(extracted) = extract_json_object(&normalized) {
            serde_json::from_str::<serde_json::Value>(&extracted).map_err(|_| {
                format!("Invalid campaign JSON shape: {}", primary_err)
            })
        } else {
            Err(format!("Invalid campaign JSON: {}", primary_err))
        }
    })?;

    normalize_campaign_blueprint_value(parsed)
}

fn normalize_campaign_blueprint_value(
    value: serde_json::Value,
) -> Result<CampaignBlueprint, String> {
    let root = find_blueprint_root(&value);
    let consistency_notes = get_first_nonempty_string_vec(
        root,
        &["consistency_notes", "consistency", "notes", "validation_notes"],
    );

    let timeline: Vec<CampaignTimelineEntry> = get_array(root, "timeline")
        .into_iter()
        .map(|entry| CampaignTimelineEntry {
            chapter: get_u32(entry, "chapter"),
            title: get_string(entry, "title"),
            beats: get_string_vec(entry, "beats"),
        })
        .collect();

    let factions = get_array(root, "factions")
        .into_iter()
        .map(|f| CampaignFaction {
            id: get_string(f, "id"),
            name: get_string(f, "name"),
            goal: get_string(f, "goal"),
            methods: get_string_vec(f, "methods"),
            allies: get_string_vec(f, "allies"),
            rivals: get_string_vec(f, "rivals"),
        })
        .collect();

    let npcs = get_array(root, "npcs")
        .into_iter()
        .map(|n| CampaignNpc {
            id: get_string(n, "id"),
            name: get_string(n, "name"),
            faction_id: get_string(n, "faction_id"),
            role: get_string(n, "role"),
            motivation: get_string(n, "motivation"),
            secrets: get_string_vec(n, "secrets"),
        })
        .collect();

    let quest_lines = get_array(root, "quest_lines")
        .into_iter()
        .map(|q| CampaignQuestLine {
            id: get_string(q, "id"),
            title: get_string(q, "title"),
            chapters: get_u32_vec(q, "chapters"),
            steps: get_string_vec(q, "steps"),
            rewards: get_string_vec(q, "rewards"),
            depends_on: get_string_vec(q, "depends_on"),
        })
        .collect();

    let world_bosses = get_array(root, "world_bosses")
        .into_iter()
        .map(|b| CampaignWorldBoss {
            id: get_string(b, "id"),
            name: get_string(b, "name"),
            chapter: get_u32(b, "chapter"),
            faction_id: get_string(b, "faction_id"),
            arena: get_string(b, "arena"),
            mechanics: get_string_vec(b, "mechanics"),
            drop_table: get_string_vec(b, "drop_table"),
        })
        .collect();

    let roaming_threats = get_array(root, "roaming_threats")
        .into_iter()
        .map(|t| CampaignRoamingThreat {
            id: get_string(t, "id"),
            name: get_string(t, "name"),
            regions: get_string_vec(t, "regions"),
            behavior: get_string(t, "behavior"),
            danger: get_string(t, "danger"),
        })
        .collect();

    let summary = build_campaign_summary(root, &timeline, &consistency_notes);

    Ok(CampaignBlueprint {
        summary,
        timeline,
        factions,
        npcs,
        quest_lines,
        world_bosses,
        roaming_threats,
        consistency_notes,
    })
}

fn find_blueprint_root(value: &serde_json::Value) -> &serde_json::Value {
    for key in ["blueprint", "campaign", "data"] {
        if let Some(candidate) = value.get(key) {
            if candidate.is_object() {
                return candidate;
            }
        }
    }
    value
}

fn get_array<'a>(
    obj: &'a serde_json::Value,
    key: &str,
) -> Vec<&'a serde_json::Value> {
    obj.get(key)
        .and_then(|v| v.as_array())
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn get_first_nonempty_string(obj: &serde_json::Value, keys: &[&str]) -> String {
    for key in keys {
        let value = get_string(obj, key);
        if !value.trim().is_empty() {
            return value;
        }
    }
    String::new()
}

fn get_first_nonempty_string_vec(obj: &serde_json::Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        let value = get_string_vec(obj, key);
        if !value.is_empty() {
            return value;
        }
    }
    Vec::new()
}

fn build_campaign_summary(
    root: &serde_json::Value,
    timeline: &[CampaignTimelineEntry],
    consistency_notes: &[String],
) -> String {
    let direct = get_first_nonempty_string(
        root,
        &[
            "summary",
            "overview",
            "synopsis",
            "campaign_summary",
            "description",
            "premise",
            "hook",
        ],
    );
    if !direct.trim().is_empty() {
        return direct;
    }

    if let Some(first) = timeline.first() {
        if !first.title.trim().is_empty() {
            return format!("Campaign opening: {}", first.title.trim());
        }
    }

    if let Some(note) = consistency_notes.first() {
        if !note.trim().is_empty() {
            return note.trim().to_string();
        }
    }

    "Generated campaign".to_string()
}

fn get_string(obj: &serde_json::Value, key: &str) -> String {
    obj.get(key).map(value_to_string).unwrap_or_default()
}

fn get_string_vec(obj: &serde_json::Value, key: &str) -> Vec<String> {
    match obj.get(key) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(value_to_string)
            .filter(|v| !v.trim().is_empty())
            .collect(),
        Some(v) => {
            let one = value_to_string(v);
            if one.trim().is_empty() {
                Vec::new()
            } else {
                vec![one]
            }
        }
        None => Vec::new(),
    }
}

fn get_u32(obj: &serde_json::Value, key: &str) -> u32 {
    obj.get(key).map(value_to_u32).unwrap_or(0)
}

fn get_u32_vec(obj: &serde_json::Value, key: &str) -> Vec<u32> {
    match obj.get(key) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(value_to_u32)
            .filter(|v| *v > 0)
            .collect(),
        Some(v) => {
            let one = value_to_u32(v);
            if one > 0 {
                vec![one]
            } else {
                Vec::new()
            }
        }
        None => Vec::new(),
    }
}

fn value_to_u32(value: &serde_json::Value) -> u32 {
    match value {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
        serde_json::Value::String(s) => s.trim().parse::<u32>().unwrap_or(0),
        serde_json::Value::Object(map) => {
            for key in ["chapter", "value", "number", "index"] {
                if let Some(v) = map.get(key) {
                    let parsed = value_to_u32(v);
                    if parsed > 0 {
                        return parsed;
                    }
                }
            }
            0
        }
        _ => 0,
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        serde_json::Value::Object(map) => {
            for key in [
                "text",
                "value",
                "name",
                "title",
                "summary",
                "description",
                "label",
                "id",
            ] {
                if let Some(v) = map.get(key) {
                    let candidate = value_to_string(v);
                    if !candidate.trim().is_empty() {
                        return candidate;
                    }
                }
            }
            serde_json::to_string(value).unwrap_or_default()
        }
        serde_json::Value::Null => String::new(),
    }
}

fn validate_campaign_blueprint(
    blueprint: &CampaignBlueprint,
    config: &crate::engine::protocol::CampaignGenerationConfig,
) -> Result<(), String> {
    if blueprint.summary.trim().is_empty() {
        return Err("summary is required.".to_string());
    }

    if config.include_timeline && blueprint.timeline.is_empty() {
        return Err("timeline is required by scope but is empty.".to_string());
    }
    if config.include_factions && blueprint.factions.is_empty() {
        return Err("factions are required by scope but are empty.".to_string());
    }
    if config.include_npcs && blueprint.npcs.is_empty() {
        return Err("npcs are required by scope but are empty.".to_string());
    }
    if config.include_quest_lines && blueprint.quest_lines.is_empty() {
        return Err("quest_lines are required by scope but are empty.".to_string());
    }
    if config.include_world_bosses && config.world_boss_count > 0 && blueprint.world_bosses.is_empty() {
        return Err("world_bosses are required by scope but are empty.".to_string());
    }
    if config.include_roaming_threats && blueprint.roaming_threats.is_empty() {
        return Err("roaming_threats are required by scope but are empty.".to_string());
    }

    let mut seen_chapters = std::collections::HashSet::new();
    for entry in &blueprint.timeline {
        if entry.chapter == 0 || entry.chapter > config.chapters.max(1) {
            return Err(format!(
                "timeline chapter {} is out of allowed range 1..={}.",
                entry.chapter,
                config.chapters.max(1)
            ));
        }
        if entry.title.trim().is_empty() {
            return Err(format!("timeline chapter {} has empty title.", entry.chapter));
        }
        if entry.beats.is_empty() {
            return Err(format!("timeline chapter {} has no beats.", entry.chapter));
        }
        seen_chapters.insert(entry.chapter);
    }

    for faction in &blueprint.factions {
        if faction.id.trim().is_empty() || faction.name.trim().is_empty() || faction.goal.trim().is_empty() {
            return Err("factions require non-empty id, name, and goal.".to_string());
        }
    }

    for npc in &blueprint.npcs {
        if npc.id.trim().is_empty() || npc.name.trim().is_empty() || npc.role.trim().is_empty() {
            return Err("npcs require non-empty id, name, and role.".to_string());
        }
    }

    for quest in &blueprint.quest_lines {
        if quest.id.trim().is_empty() || quest.title.trim().is_empty() {
            return Err("quest_lines require non-empty id and title.".to_string());
        }
        if quest.steps.is_empty() {
            return Err(format!("quest_line '{}' has no steps.", quest.id));
        }
        for chapter in &quest.chapters {
            if *chapter == 0 || *chapter > config.chapters.max(1) {
                return Err(format!(
                    "quest_line '{}' references chapter {} out of range 1..={}.",
                    quest.id,
                    chapter,
                    config.chapters.max(1)
                ));
            }
        }
    }

    for boss in &blueprint.world_bosses {
        if boss.id.trim().is_empty() || boss.name.trim().is_empty() {
            return Err("world_bosses require non-empty id and name.".to_string());
        }
        if boss.chapter == 0 || boss.chapter > config.chapters.max(1) {
            return Err(format!(
                "world_boss '{}' chapter {} out of range 1..={}.",
                boss.id,
                boss.chapter,
                config.chapters.max(1)
            ));
        }
    }

    for threat in &blueprint.roaming_threats {
        if threat.id.trim().is_empty() || threat.name.trim().is_empty() {
            return Err("roaming_threats require non-empty id and name.".to_string());
        }
        if threat.regions.is_empty() {
            return Err(format!("roaming_threat '{}' has no regions.", threat.id));
        }
    }

    Ok(())
}

fn normalize_campaign_json(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.starts_with("```") {
        if let Some(first_newline) = s.find('\n') {
            s = s[(first_newline + 1)..].to_string();
        }
        if let Some(end_fence) = s.rfind("```") {
            s = s[..end_fence].to_string();
        }
    }
    s.trim().to_string()
}

fn extract_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(s[start..=end].to_string())
}

fn campaigns_root_dir() -> std::path::PathBuf {
    std::path::Path::new("data").join("campaigns")
}

fn save_campaign_package(
    blueprint: &CampaignBlueprint,
    config: &crate::engine::protocol::CampaignGenerationConfig,
) -> Result<std::path::PathBuf, String> {
    let root = campaigns_root_dir();
    fs::create_dir_all(&root).map_err(|e| format!("cannot create campaigns directory: {}", e))?;

    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock error: {}", e))?;

    let campaign_id = format!("campaign_{}_{}", epoch.as_secs(), epoch.subsec_nanos());
    let campaign_dir = root.join(&campaign_id);
    fs::create_dir_all(&campaign_dir)
        .map_err(|e| format!("cannot create campaign directory: {}", e))?;

    let timeline_dir = campaign_dir.join("timeline");
    let factions_dir = campaign_dir.join("factions");
    let npcs_dir = campaign_dir.join("npcs");
    let quests_dir = campaign_dir.join("quests");
    let bosses_dir = campaign_dir.join("bosses");
    let threats_dir = campaign_dir.join("threats");
    for dir in [&timeline_dir, &factions_dir, &npcs_dir, &quests_dir, &bosses_dir, &threats_dir] {
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create campaign subdirectory '{}': {}", dir.display(), e))?;
    }

    let scope = [
        ("timeline", config.include_timeline),
        ("npcs", config.include_npcs),
        ("factions", config.include_factions),
        ("quest_lines", config.include_quest_lines),
        ("world_bosses", config.include_world_bosses),
        ("roaming_threats", config.include_roaming_threats),
    ]
    .iter()
    .filter_map(|(k, v)| if *v { Some((*k).to_string()) } else { None })
    .collect::<Vec<_>>();

    let manifest = CampaignManifest {
        campaign_id: campaign_id.clone(),
        summary: blueprint.summary.clone(),
        chapters: config.chapters.max(1),
        generated_unix: epoch.as_secs(),
        scope,
        core_tone: config.core_tone.clone(),
        narrative_style: config.narrative_style.clone(),
        intensity: config.intensity,
    };

    let mut timeline_files = Vec::new();
    for entry in &blueprint.timeline {
        let name = format!("chapter_{:02}.json", entry.chapter);
        let rel = format!("timeline/{}", name);
        write_json_file(&campaign_dir.join(&rel), entry)?;
        timeline_files.push(rel);
    }

    let mut faction_files = Vec::new();
    for faction in &blueprint.factions {
        let id = sanitize_file_component(&faction.id, "faction");
        let rel = format!("factions/{}.json", id);
        write_json_file(&campaign_dir.join(&rel), faction)?;
        faction_files.push(rel);
    }

    let mut npc_files = Vec::new();
    for npc in &blueprint.npcs {
        let id = sanitize_file_component(&npc.id, "npc");
        let rel = format!("npcs/{}.json", id);
        write_json_file(&campaign_dir.join(&rel), npc)?;
        npc_files.push(rel);
    }

    let mut quest_files = Vec::new();
    for quest in &blueprint.quest_lines {
        let id = sanitize_file_component(&quest.id, "quest");
        let rel = format!("quests/{}.json", id);
        write_json_file(&campaign_dir.join(&rel), quest)?;
        quest_files.push(rel);
    }

    let mut boss_files = Vec::new();
    for boss in &blueprint.world_bosses {
        let id = sanitize_file_component(&boss.id, "boss");
        let rel = format!("bosses/{}.json", id);
        write_json_file(&campaign_dir.join(&rel), boss)?;
        boss_files.push(rel);
    }

    let mut threat_files = Vec::new();
    for threat in &blueprint.roaming_threats {
        let id = sanitize_file_component(&threat.id, "threat");
        let rel = format!("threats/{}.json", id);
        write_json_file(&campaign_dir.join(&rel), threat)?;
        threat_files.push(rel);
    }

    let state = CampaignRuntimeState {
        current_chapter: 1,
        current_phase: "opening".to_string(),
        revealed_factions: Vec::new(),
        revealed_npcs: Vec::new(),
        revealed_quests: Vec::new(),
        revealed_world_bosses: Vec::new(),
        defeated_world_bosses: Vec::new(),
        seen_roaming_threats: Vec::new(),
        campaign_flags: Vec::new(),
        active_quest_lines: Vec::new(),
        completed_quest_lines: Vec::new(),
        failed_quest_lines: Vec::new(),
        world_boss_states: std::collections::HashMap::new(),
    };

    let index = CampaignIndex {
        manifest: "manifest.json".to_string(),
        state: "state.json".to_string(),
        timeline: timeline_files,
        factions: faction_files,
        npcs: npc_files,
        quest_lines: quest_files,
        world_bosses: boss_files,
        roaming_threats: threat_files,
    };

    write_json_file(&campaign_dir.join("manifest.json"), &manifest)?;
    write_json_file(&campaign_dir.join("state.json"), &state)?;
    write_json_file(&campaign_dir.join("index.json"), &index)?;
    write_json_file(&campaign_dir.join("blueprint_full.json"), blueprint)?;

    fs::write(root.join("active_campaign.txt"), &campaign_id)
        .map_err(|e| format!("cannot set active campaign: {}", e))?;

    Ok(campaign_dir)
}

fn write_json_file<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("cannot serialize json '{}': {}", path.display(), e))?;
    fs::write(path, json).map_err(|e| format!("cannot write '{}': {}", path.display(), e))
}

fn sanitize_file_component(input: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_sep = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn load_campaign_context(topic: &str) -> String {
    let bundle = match load_active_campaign_bundle() {
        Ok(bundle) => bundle,
        Err(err) => return format!("Campaign context unavailable: {}", err),
    };

    match topic {
        "campaign" | "campaign_context" => {
            let mut out = String::new();
            out.push_str("MANIFEST:\n");
            out.push_str(&to_pretty_json(&bundle.manifest));
            out.push_str("\n\nSTATE:\n");
            out.push_str(&to_pretty_json(&bundle.state));

            if let Some(path) = find_current_chapter_file(&bundle.index.timeline, bundle.state.current_chapter) {
                out.push_str("\n\nCURRENT CHAPTER:\n");
                out.push_str(&read_campaign_file_or_message(&bundle.root, &path));
            }
            out
        }
        "campaign_manifest" | "campaign_meta" => to_pretty_json(&bundle.manifest),
        "campaign_state" => to_pretty_json(&bundle.state),
        "campaign_index" => to_pretty_json(&bundle.index),
        "campaign_timeline" => {
            if let Some(path) = find_current_chapter_file(&bundle.index.timeline, bundle.state.current_chapter) {
                return read_campaign_file_or_message(&bundle.root, &path);
            }
            read_campaign_group(&bundle.root, &bundle.index.timeline, 3)
        }
        "campaign_factions" => read_campaign_group(&bundle.root, &bundle.index.factions, 20),
        "campaign_npcs" => read_campaign_group(&bundle.root, &bundle.index.npcs, 30),
        "campaign_quests" => read_campaign_group(&bundle.root, &bundle.index.quest_lines, 20),
        "campaign_bosses" => read_campaign_group(&bundle.root, &bundle.index.world_bosses, 20),
        "campaign_threats" | "campaign_roaming_threats" => {
            read_campaign_group(&bundle.root, &bundle.index.roaming_threats, 20)
        }
        _ => format!("No campaign provider for topic '{}'.", topic),
    }
}

fn load_active_campaign_bundle() -> Result<ActiveCampaignBundle, String> {
    let root = campaigns_root_dir();
    let active_path = root.join("active_campaign.txt");
    let campaign_id = fs::read_to_string(&active_path)
        .map_err(|e| format!("missing active campaign pointer '{}': {}", active_path.display(), e))?;
    let campaign_id = campaign_id.trim();
    if campaign_id.is_empty() {
        return Err("active campaign pointer is empty.".to_string());
    }

    let campaign_root = root.join(campaign_id);
    let manifest: CampaignManifest = read_json_file(&campaign_root.join("manifest.json"))?;
    let state: CampaignRuntimeState = read_json_file(&campaign_root.join("state.json"))?;
    let index: CampaignIndex = read_json_file(&campaign_root.join("index.json"))?;

    Ok(ActiveCampaignBundle {
        root: campaign_root,
        manifest,
        state,
        index,
    })
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &std::path::Path) -> Result<T, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("cannot parse '{}': {}", path.display(), e))
}

fn to_pretty_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|_| "{}".to_string())
}

fn find_current_chapter_file(paths: &[String], current_chapter: u32) -> Option<String> {
    let needle = format!("chapter_{:02}.json", current_chapter);
    if let Some(path) = paths.iter().find(|p| p.ends_with(&needle)) {
        return Some(path.clone());
    }
    let needle_plain = format!("chapter_{}.json", current_chapter);
    paths.iter().find(|p| p.ends_with(&needle_plain)).cloned()
}

fn read_campaign_group(root: &std::path::Path, files: &[String], max_files: usize) -> String {
    if files.is_empty() {
        return "None".to_string();
    }
    let mut out = String::new();
    for file in files.iter().take(max_files) {
        out.push_str(&format!("FILE: {}\n", file));
        out.push_str(&read_campaign_file_or_message(root, file));
        out.push_str("\n\n");
    }
    if files.len() > max_files {
        out.push_str(&format!(
            "... truncated {} additional files\n",
            files.len().saturating_sub(max_files)
        ));
    }
    out
}

fn read_campaign_file_or_message(root: &std::path::Path, relative: &str) -> String {
    let path = root.join(relative);
    match fs::read_to_string(&path) {
        Ok(data) => data,
        Err(err) => format!("unable to read '{}': {}", path.display(), err),
    }
}

fn is_campaign_runtime_event(event: &NarrativeEvent) -> bool {
    matches!(
        event,
        NarrativeEvent::CampaignSetChapter { .. }
            | NarrativeEvent::CampaignSetPhase { .. }
            | NarrativeEvent::CampaignReveal { .. }
            | NarrativeEvent::CampaignSetFlag { .. }
            | NarrativeEvent::QuestlineAdvance { .. }
            | NarrativeEvent::QuestlineComplete { .. }
            | NarrativeEvent::QuestlineFail { .. }
            | NarrativeEvent::WorldBossState { .. }
    )
}

fn update_active_campaign_runtime_state(event: &NarrativeEvent) -> Result<(), String> {
    let bundle = match load_active_campaign_bundle() {
        Ok(bundle) => bundle,
        Err(_) => return Ok(()),
    };
    let mut state = bundle.state;
    let mut changed = false;

    match event {
        NarrativeEvent::CampaignSetChapter { chapter } => {
            let max_chapter = bundle.manifest.chapters.max(1);
            let next = (*chapter).max(1).min(max_chapter);
            if state.current_chapter != next {
                state.current_chapter = next;
                changed = true;
            }
        }
        NarrativeEvent::CampaignSetPhase { phase } => {
            let trimmed = phase.trim();
            if !trimmed.is_empty() && state.current_phase != trimmed {
                state.current_phase = trimmed.to_string();
                changed = true;
            }
        }
        NarrativeEvent::CampaignReveal { entity_type, id } => {
            let kind = entity_type.trim().to_lowercase();
            let target = id.trim();
            if !kind.is_empty() && !target.is_empty() {
                changed |= match kind.as_str() {
                    "faction" | "factions" => push_unique(&mut state.revealed_factions, target),
                    "npc" | "npcs" | "companion" | "companions" => {
                        push_unique(&mut state.revealed_npcs, target)
                    }
                    "quest" | "quests" | "questline" | "quest_line" | "questlines" => {
                        push_unique(&mut state.revealed_quests, target)
                    }
                    "world_boss" | "worldboss" | "boss" | "bosses" => {
                        push_unique(&mut state.revealed_world_bosses, target)
                    }
                    "threat" | "threats" | "roaming_threat" | "roaming_threats" => {
                        push_unique(&mut state.seen_roaming_threats, target)
                    }
                    _ => push_unique(
                        &mut state.campaign_flags,
                        &format!("reveal:{}:{}", kind, target),
                    ),
                };
            }
        }
        NarrativeEvent::CampaignSetFlag { flag } => {
            let trimmed = flag.trim();
            if !trimmed.is_empty() {
                changed |= push_unique(&mut state.campaign_flags, trimmed);
            }
        }
        NarrativeEvent::QuestlineAdvance { id, .. } => {
            let quest_id = id.trim();
            if !quest_id.is_empty() {
                changed |= push_unique(&mut state.active_quest_lines, quest_id);
                changed |= remove_value(&mut state.completed_quest_lines, quest_id);
                changed |= remove_value(&mut state.failed_quest_lines, quest_id);
                changed |= push_unique(&mut state.revealed_quests, quest_id);
            }
        }
        NarrativeEvent::QuestlineComplete { id, .. } => {
            let quest_id = id.trim();
            if !quest_id.is_empty() {
                changed |= remove_value(&mut state.active_quest_lines, quest_id);
                changed |= remove_value(&mut state.failed_quest_lines, quest_id);
                changed |= push_unique(&mut state.completed_quest_lines, quest_id);
                changed |= push_unique(&mut state.revealed_quests, quest_id);
            }
        }
        NarrativeEvent::QuestlineFail { id, .. } => {
            let quest_id = id.trim();
            if !quest_id.is_empty() {
                changed |= remove_value(&mut state.active_quest_lines, quest_id);
                changed |= remove_value(&mut state.completed_quest_lines, quest_id);
                changed |= push_unique(&mut state.failed_quest_lines, quest_id);
                changed |= push_unique(&mut state.revealed_quests, quest_id);
            }
        }
        NarrativeEvent::WorldBossState {
            id,
            state: boss_state,
            ..
        } => {
            let boss_id = id.trim();
            let trimmed_state = boss_state.trim().to_lowercase();
            if !boss_id.is_empty() && !trimmed_state.is_empty() {
                changed |= push_unique(&mut state.revealed_world_bosses, boss_id);
                match state.world_boss_states.get(boss_id) {
                    Some(prev) if prev == &trimmed_state => {}
                    _ => {
                        state
                            .world_boss_states
                            .insert(boss_id.to_string(), trimmed_state.clone());
                        changed = true;
                    }
                }
                if trimmed_state == "defeated" {
                    changed |= push_unique(&mut state.defeated_world_bosses, boss_id);
                }
            }
        }
        _ => {}
    }

    if !changed {
        return Ok(());
    }

    write_json_file(&bundle.root.join("state.json"), &state)
}

fn push_unique(values: &mut Vec<String>, value: &str) -> bool {
    if values.iter().any(|v| v == value) {
        return false;
    }
    values.push(value.to_string());
    true
}

fn remove_value(values: &mut Vec<String>, value: &str) -> bool {
    let before = values.len();
    values.retain(|v| v != value);
    before != values.len()
}

fn player_accepts_quest(input: &str) -> bool {
    let t = normalize_phrase(input);
    let phrases = [
        "i accept",
        "i accept the quest",
        "accept quest",
        "accept the quest",
        "yes i accept",
        "i agree",
        "i will do it",
        "i will take it",
        "i accept it",
        "accept it",
        "i will do this",
        "sure",
        "yes",
        "ok",
        "okay",
    ];
    phrases.iter().any(|p| t.contains(p))
}

fn normalize_phrase(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

fn update_action_counts(state: &mut InternalGameState, input: &str) {
    let text = input.to_lowercase();
    let actions: [(&str, &[&str]); 7] = [
        ("jumping", &["jump", "jumps", "jumping", "leap", "hop"]),
        ("mining", &["mine", "mines", "mining", "pickaxe", "ore"]),
        ("fishing", &["fish", "fishing", "cast line", "reel"]),
        ("woodcutting", &["chop", "chopping", "woodcut", "lumber", "axe"]),
        ("crafting", &["craft", "crafting", "forge", "smith", "smithing"]),
        ("stealth", &["sneak", "sneaking", "stealth", "hide", "hidden"]),
        (
            "being_hit",
            &[
                "i'm hit",
                "i am hit",
                "hit me",
                "hits me",
                "struck",
                "wounded",
                "hurt",
                "took damage",
                "i take damage",
            ],
        ),
    ];

    for (action, keywords) in actions {
        if keywords.iter().any(|k| text.contains(k)) {
            let entry = state.action_counts.entry(action.to_string()).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    // Track direct mentions of any stat id so user-defined stats (e.g., "lust")
    // can participate in usage-based level growth.
    let stat_ids: Vec<String> = state.stats.keys().cloned().collect();
    for stat_id in stat_ids {
        let needle = stat_id.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        if text.contains(&needle) {
            let key = format!("stat::{}", needle);
            let entry = state.action_counts.entry(key).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
}

fn sync_stats_from_context(state: &mut InternalGameState, context: &crate::model::game_context::GameContext) {
    for (k, v) in &context.player.stats {
        state.stats.insert(k.to_string(), *v);
    }
}

fn apply_level_stat_growth(
    state: &mut InternalGameState,
    context: &crate::model::game_context::GameContext,
    start_level: u32,
    applications: &mut Vec<EventApplication>,
) {
    let gained = state.player.level.saturating_sub(start_level);
    if gained == 0 {
        return;
    }

    let class = context.player.class.to_lowercase();

    for _ in 0..gained {
        let mut deltas: Vec<(&str, i32)> = Vec::new();

        if class.contains("tank") || class.contains("guardian") || class.contains("paladin") {
            deltas.push(("constitution", 2));
            deltas.push(("strength", 1));
        } else if class.contains("warrior") || class.contains("fighter") || class.contains("barbarian") {
            deltas.push(("strength", 2));
            deltas.push(("constitution", 1));
        } else if class.contains("rogue") || class.contains("assassin") || class.contains("ranger") {
            deltas.push(("agility", 2));
            deltas.push(("luck", 1));
        } else if class.contains("mage") || class.contains("wizard") || class.contains("sorcerer") {
            deltas.push(("intelligence", 2));
            deltas.push(("luck", 1));
        } else if class.contains("cleric") || class.contains("priest") || class.contains("druid") {
            deltas.push(("intelligence", 1));
            deltas.push(("constitution", 1));
            deltas.push(("luck", 1));
        } else {
            deltas.push(("strength", 1));
            deltas.push(("constitution", 1));
        }

        let being_hit = state.action_counts.get("being_hit").copied().unwrap_or(0);
        let mining = state.action_counts.get("mining").copied().unwrap_or(0);
        let woodcutting = state.action_counts.get("woodcutting").copied().unwrap_or(0);
        let jumping = state.action_counts.get("jumping").copied().unwrap_or(0);
        let stealth = state.action_counts.get("stealth").copied().unwrap_or(0);
        let crafting = state.action_counts.get("crafting").copied().unwrap_or(0);
        let fishing = state.action_counts.get("fishing").copied().unwrap_or(0);
        let mut usage_scores: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        // Built-in action-to-stat mappings.
        *usage_scores.entry("strength".to_string()).or_insert(0) = mining.saturating_add(woodcutting);
        *usage_scores.entry("constitution".to_string()).or_insert(0) = being_hit;
        *usage_scores.entry("agility".to_string()).or_insert(0) = jumping.saturating_add(stealth);
        *usage_scores.entry("intelligence".to_string()).or_insert(0) = crafting;
        *usage_scores.entry("luck".to_string()).or_insert(0) = fishing;

        // Dynamic stat usage from direct stat mentions (stat::<id>).
        for (key, value) in &state.action_counts {
            if let Some(stat_id) = key.strip_prefix("stat::") {
                if value > &0 {
                    let entry = usage_scores.entry(stat_id.to_string()).or_insert(0);
                    *entry = entry.saturating_add(*value);
                }
            }
        }

        let mut top: Option<(&str, u32)> = None;
        let mut second: Option<(&str, u32)> = None;
        for (stat, score) in usage_scores.iter() {
            if *score == 0 {
                continue;
            }

            match top {
                None => top = Some((stat.as_str(), *score)),
                Some((_, top_score)) if *score > top_score => {
                    second = top;
                    top = Some((stat.as_str(), *score));
                }
                _ => match second {
                    None => second = Some((stat.as_str(), *score)),
                    Some((_, second_score)) if *score > second_score => {
                        second = Some((stat.as_str(), *score));
                    }
                    _ => {}
                },
            }
        }

        if let Some((top_stat, _)) = top {
            // Most-used play pattern should drive the biggest base stat growth.
            deltas.push((top_stat, 2));
            if let Some((second_stat, _)) = second {
                if second_stat != top_stat {
                    deltas.push((second_stat, 1));
                }
            }
        }

        apply_stat_deltas(state, deltas, applications);
    }
}

fn apply_stat_deltas(
    state: &mut InternalGameState,
    deltas: Vec<(&str, i32)>,
    applications: &mut Vec<EventApplication>,
) {
    for (stat_id, delta) in deltas {
        let entry = state.stats.entry(stat_id.to_string()).or_insert(10);
        *entry += delta;
        let event = NarrativeEvent::ModifyStat {
            stat_id: stat_id.to_string(),
            delta,
        };
        applications.push(EventApplication {
            event,
            outcome: EventApplyOutcome::Applied,
        });
    }
}

fn update_power_usage(state: &mut InternalGameState, input: &str) {
    if state.powers.is_empty() {
        return;
    }
    let text = input.to_lowercase();
    for power in state.powers.values() {
        let name = power.name.trim();
        if name.is_empty() {
            continue;
        }
        if text.contains(&name.to_lowercase()) {
            let entry = state.power_usage_counts.entry(power.id.clone()).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }
}

fn apply_set_bonuses(state: &mut InternalGameState, applications: &mut Vec<EventApplication>) {
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for item in state.equipment.values() {
        let Some(set_id) = &item.set_id else { continue };
        let entry = counts.entry(set_id.clone()).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let mut affected: std::collections::HashSet<String> = std::collections::HashSet::new();
    for set_id in counts.keys() {
        affected.insert(set_id.clone());
    }
    for set_id in state.set_bonus_tiers.keys() {
        affected.insert(set_id.clone());
    }

    for set_id in affected {
        let count = counts.get(&set_id).copied().unwrap_or(0);
        let desired = if count >= 4 {
            2
        } else if count >= 2 {
            1
        } else {
            0
        };
        let current = state.set_bonus_tiers.get(&set_id).copied().unwrap_or(0);
        if desired == current {
            continue;
        }

        if current > 0 {
            let deltas = set_bonus_deltas(current, true);
            apply_stat_deltas(state, deltas, applications);
        }
        if desired > 0 {
            let deltas = set_bonus_deltas(desired, false);
            apply_stat_deltas(state, deltas, applications);
        }

        if desired == 0 {
            state.set_bonus_tiers.remove(&set_id);
        } else {
            state.set_bonus_tiers.insert(set_id.clone(), desired);
        }

        let name = if desired == 2 {
            format!("{} Set Bonus (4)", set_id)
        } else if desired == 1 {
            format!("{} Set Bonus (2)", set_id)
        } else {
            format!("{} Set Bonus", set_id)
        };
        let desc = if desired == 2 {
            "Major set bonus: +2 strength, +2 constitution, +1 agility.".to_string()
        } else if desired == 1 {
            "Minor set bonus: +1 strength, +1 constitution.".to_string()
        } else {
            "Set bonus inactive.".to_string()
        };
        let event = NarrativeEvent::GrantPower {
            id: format!("set_bonus_{}", set_id.to_lowercase().replace(' ', "_")),
            name,
            description: desc,
        };
        let outcome = apply_event(state, event.clone());
        applications.push(EventApplication { event, outcome });
    }
}

fn set_bonus_deltas(tier: u32, remove: bool) -> Vec<(&'static str, i32)> {
    let mult = if remove { -1 } else { 1 };
    match tier {
        1 => vec![("strength", 1 * mult), ("constitution", 1 * mult)],
        2 => vec![("strength", 2 * mult), ("constitution", 2 * mult), ("agility", 1 * mult)],
        _ => Vec::new(),
    }
}

fn maybe_evolve_powers(
    state: &mut InternalGameState,
    world: &crate::ui::app::WorldDefinition,
    applications: &mut Vec<EventApplication>,
) {
    if state.powers.is_empty() {
        return;
    }
    let base_threshold = world.power_evolution_base.max(1);
    let step = world.power_evolution_step.max(1);
    let min_mult = world.power_evolution_multiplier_min.max(1.0);
    let max_mult = world
        .power_evolution_multiplier_max
        .max(min_mult);
    let mut rng = rand::thread_rng();

    for (id, power) in state.powers.clone() {
        let uses = state.power_usage_counts.get(&id).copied().unwrap_or(0);
        if uses < base_threshold {
            continue;
        }
        let tiers = 1 + (uses.saturating_sub(base_threshold)) / step;
        let capped_tier = tiers.min(5);
        let current = state.power_evolution_tiers.get(&id).copied().unwrap_or(0);
        if capped_tier <= current {
            continue;
        }
        let multiplier: f32 = rng.gen_range(min_mult..=max_mult);
        state.power_evolution_tiers.insert(id.clone(), capped_tier);

        let evolved_name = format!("Evolved {}", power.name);
        let evolved_desc = format!(
            "{}\nEvolution tier {}. Multiplier x{:.2}.",
            power.description, capped_tier, multiplier
        );

        let event = NarrativeEvent::GrantPower {
            id: id.clone(),
            name: evolved_name,
            description: evolved_desc,
        };
        let outcome = apply_event(state, event.clone());
        applications.push(EventApplication { event, outcome });
    }
}

fn maybe_grant_repetition_power(
    state: &mut InternalGameState,
    input: &str,
    world: &crate::ui::app::WorldDefinition,
    applications: &mut Vec<EventApplication>,
) {
    let text = input.to_lowercase();
    let candidates: [(&str, &[&str], &str, &str, &str); 6] = [
        (
            "jumping",
            &["jump", "jumps", "jumping", "leap", "hop"],
            "skill_jumping",
            "Jumping Skill",
            "Improves jumping efficiency and control from repeated practice.",
        ),
        (
            "mining",
            &["mine", "mines", "mining", "pickaxe", "ore"],
            "skill_mining",
            "Mining Skill",
            "Improves mining yield and stamina from repeated practice.",
        ),
        (
            "fishing",
            &["fish", "fishing", "cast line", "reel"],
            "skill_fishing",
            "Fishing Skill",
            "Improves fishing success and patience from repeated practice.",
        ),
        (
            "woodcutting",
            &["chop", "chopping", "woodcut", "lumber", "axe"],
            "skill_woodcutting",
            "Woodcutting Skill",
            "Improves woodcutting efficiency from repeated practice.",
        ),
        (
            "crafting",
            &["craft", "crafting", "forge", "smith", "smithing"],
            "skill_crafting",
            "Crafting Skill",
            "Improves crafting outcomes from repeated practice.",
        ),
        (
            "stealth",
            &["sneak", "sneaking", "stealth", "hide", "hidden"],
            "skill_stealth",
            "Stealth Skill",
            "Improves stealth and movement control from repeated practice.",
        ),
    ];

    let base_default = world.repetition_threshold.max(1);
    let step_default = world.repetition_tier_step.max(1);

    for (action_key, keywords, power_id, power_name, power_desc) in candidates {
        if !keywords.iter().any(|k| text.contains(k)) {
            continue;
        }
        let count = state.action_counts.get(action_key).copied().unwrap_or(0);
        let (base, step) = skill_threshold_for(world, action_key, base_default, step_default);
        let tier = repetition_tier(count, base, step);
        if tier == 0 {
            continue;
        }
        let capped_tier = tier.min(5);
        if let Some(existing) = state.powers.get(power_id) {
            let names = skill_tier_names_for(world, action_key);
            let current = current_tier_from_name(&existing.name, &names);
            if current >= capped_tier {
                continue;
            }
        }
        let tier_name = tier_name_for(world, capped_tier);
        let upgraded_name = format!("{} {}", tier_name, power_name);
        let upgraded_desc = format!("Tier {}. {}", capped_tier, power_desc);

        let event = NarrativeEvent::GrantPower {
            id: power_id.to_string(),
            name: upgraded_name,
            description: upgraded_desc,
        };
        let outcome = apply_event(state, event.clone());
        applications.push(EventApplication { event, outcome });
    }
}

fn repetition_tier(count: u32, base: u32, step: u32) -> u32 {
    if count < base {
        return 0;
    }
    let step = step.max(1);
    1 + (count - base) / step
}

fn current_tier_from_name(name: &str, tier_names: &[String; 5]) -> u32 {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let Some((prefix, _)) = trimmed.split_once(' ') else {
        return 0;
    };
    for (idx, tier) in tier_names.iter().enumerate() {
        if prefix.eq_ignore_ascii_case(tier.trim()) {
            return (idx + 1) as u32;
        }
    }
    0
}

fn tier_name_for(world: &crate::ui::app::WorldDefinition, tier: u32) -> String {
    let mut names = world.skill_tier_names.clone();
    ensure_tier_names(&mut names);
    let idx = (tier.saturating_sub(1) as usize).min(4);
    names[idx].clone()
}

fn ensure_tier_names(names: &mut Vec<String>) {
    let defaults = ["Novice", "Adept", "Expert", "Master", "Grandmaster"];
    if names.len() < 5 {
        for i in names.len()..5 {
            names.push(defaults[i].to_string());
        }
    } else if names.len() > 5 {
        names.truncate(5);
    }
    for (i, name) in names.iter_mut().enumerate() {
        if name.trim().is_empty() {
            *name = defaults[i].to_string();
        }
    }
}

fn skill_threshold_for(
    world: &crate::ui::app::WorldDefinition,
    skill: &str,
    base_default: u32,
    step_default: u32,
) -> (u32, u32) {
    for entry in &world.skill_thresholds {
        if entry.skill.trim().eq_ignore_ascii_case(skill) {
            return (entry.base.max(1), entry.step.max(1));
        }
    }
    (base_default, step_default)
}

fn skill_tier_names_for(
    world: &crate::ui::app::WorldDefinition,
    skill: &str,
) -> [String; 5] {
    for entry in &world.skill_thresholds {
        if entry.skill.trim().eq_ignore_ascii_case(skill) {
            let names = normalized_tier_names(&entry.tier_names);
            return names;
        }
    }
    normalized_tier_names(&world.skill_tier_names)
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

fn player_requested_party_details(input: &str) -> bool {
    let t = input.to_ascii_lowercase();
    let phrases = [
        "describe",
        "details",
        "look over",
        "inspect",
        "examine",
        "what is",
        "tell me about",
        "appearance",
        "clothing",
        "outfit",
        "wearing",
    ];
    phrases.iter().any(|p| t.contains(p))
}

fn sanitize_party_update(event: &NarrativeEvent) -> NarrativeEvent {
    let NarrativeEvent::PartyUpdate {
        id,
        name,
        role,
        details,
        clothing_add,
        clothing_remove,
        weapons_add,
        weapons_remove,
        armor_add,
        armor_remove,
    } = event
    else {
        return event.clone();
    };

    let mut details = details.as_ref().map(|d| d.trim().to_string());
    if let Some(d) = details.as_mut() {
        if d.len() > 320 {
            d.truncate(317);
            d.push_str("...");
        }
    }

    let mut clothing_add = clothing_add.clone();
    let mut clothing_remove = clothing_remove.clone();
    let mut weapons_add = weapons_add.clone();
    let mut weapons_remove = weapons_remove.clone();
    let mut armor_add = armor_add.clone();
    let mut armor_remove = armor_remove.clone();

    fn sanitize_items(items: &mut Option<Vec<String>>) {
        if let Some(list) = items.as_mut() {
            list.retain(|c| !c.trim().is_empty());
            if list.len() > 8 {
                list.truncate(8);
            }
        }
    }

    sanitize_items(&mut clothing_add);
    sanitize_items(&mut clothing_remove);
    sanitize_items(&mut weapons_add);
    sanitize_items(&mut weapons_remove);
    sanitize_items(&mut armor_add);
    sanitize_items(&mut armor_remove);

    NarrativeEvent::PartyUpdate {
        id: id.clone(),
        name: name.clone(),
        role: role.clone(),
        details,
        clothing_add,
        clothing_remove,
        weapons_add,
        weapons_remove,
        armor_add,
        armor_remove,
    }
}

fn migrate_save(save: &mut GameSave) {
    if save.version < SAVE_VERSION {
        save.version = SAVE_VERSION;
    }
}

fn generate_unique_party_id(state: &InternalGameState, name: &str) -> String {
    let mut base = String::new();
    let mut last_was_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            base.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            base.push('_');
            last_was_underscore = true;
        }
    }
    let trimmed = base.trim_matches('_');
    let base_id = if trimmed.is_empty() {
        "party_member".to_string()
    } else {
        format!("party_{}", trimmed)
    };
    if !state.party.contains_key(&base_id) {
        return base_id;
    }
    let mut idx = 2;
    loop {
        let candidate = format!("{}_{}", base_id, idx);
        if !state.party.contains_key(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}

fn diff_lists(old_list: &[String], new_list: &[String]) -> (Vec<String>, Vec<String>) {
    let mut add = Vec::new();
    let mut remove = Vec::new();
    for item in new_list {
        if !old_list.iter().any(|v| v.eq_ignore_ascii_case(item)) {
            add.push(item.clone());
        }
    }
    for item in old_list {
        if !new_list.iter().any(|v| v.eq_ignore_ascii_case(item)) {
            remove.push(item.clone());
        }
    }
    (add, remove)
}

#[cfg(test)]
mod tests {
    use super::sanitize_party_update;
    use crate::model::narrative_event::NarrativeEvent;

    #[test]
    fn sanitize_party_update_trims_lists_and_details() {
        let event = NarrativeEvent::PartyUpdate {
            id: "p1".to_string(),
            name: None,
            role: None,
            details: Some("a".repeat(400)),
            clothing_add: Some(vec![
                "hat".to_string(),
                "".to_string(),
                "boots".to_string(),
                "gloves".to_string(),
                "cape".to_string(),
                "belt".to_string(),
                "ring".to_string(),
                "amulet".to_string(),
                "extra".to_string(),
            ]),
            clothing_remove: None,
            weapons_add: None,
            weapons_remove: None,
            armor_add: None,
            armor_remove: None,
        };

        let sanitized = sanitize_party_update(&event);
        if let NarrativeEvent::PartyUpdate { details, clothing_add, .. } = sanitized {
            let details = details.expect("details");
            assert!(details.len() <= 320);
            let clothing_add = clothing_add.expect("clothing_add");
            assert!(clothing_add.len() <= 8);
            assert!(!clothing_add.iter().any(|v| v.trim().is_empty()));
        } else {
            panic!("expected party update");
        }
    }
}
