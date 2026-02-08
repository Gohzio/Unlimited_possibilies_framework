use std::sync::mpsc::{Receiver, Sender, TryRecvError, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::thread;

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::engine::prompt_builder::PromptBuilder;
use crate::engine::llm_client::{abort_generation, call_llm, test_connection};
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
            pending_generation: None,
        }
    }

    fn send_ui_error(&self, message: String) {
        let _ = self.tx.send(EngineResponse::UiError { message });
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

                let outcome = apply_event(&mut self.game_state, event.clone());
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
                let outcome = apply_event(&mut self.game_state, event.clone());
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
                    let outcome = apply_event(&mut self.game_state, event.clone());
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

        // 4. Split NARRATIVE vs EVENTS
        let (narrative, events_json) =
            llm_output
                .split_once("EVENTS:")
                .unwrap_or((&llm_output, "[]"));
        let split_done = Instant::now();

        // 5. Decode EVENTS JSON
        let events = match crate::model::llm_decode::decode_llm_events(events_json) {
            Ok(events) => events,
            Err(err) => {
                self.messages.push(Message::System(format!(
                    "Failed to parse EVENTS: {}",
                    err
                )));
                self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                Vec::new()
            }
        };
        let parse_done = Instant::now();

        // 6. Handle request_context (one additional round)
        if let Some(topics) = collect_requested_topics(&events) {
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

            let (narrative, events_json) =
                llm_output
                    .split_once("EVENTS:")
                    .unwrap_or((&llm_output, "[]"));
            let followup_split_done = Instant::now();
            let events = match crate::model::llm_decode::decode_llm_events(events_json) {
                Ok(events) => events,
                Err(err) => {
                    self.messages.push(Message::System(format!(
                        "Failed to parse EVENTS: {}",
                        err
                    )));
                    self.send_ui_error(format!("Failed to parse EVENTS: {}", err));
                    Vec::new()
                }
            };
            let followup_parse_done = Instant::now();

            let start_level = self.game_state.player.level;
            if events.iter().any(|e| matches!(e, NarrativeEvent::RequestContext { .. })) {
                self.messages.push(Message::System(
                    "Context was already provided. Please respond with narrative and events."
                        .to_string(),
                ));
                self.send_new_messages_since(messages_start);
                return;
            }

            let new_messages = parse_narrative(narrative);
            self.messages.extend(new_messages);
            let narrative_done = Instant::now();

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
                    let outcome = apply_event(&mut self.game_state, sanitized.clone());
                    applications.push(EventApplication {
                        event: sanitized,
                        outcome,
                    });
                    continue;
                }
                let outcome = apply_event(&mut self.game_state, event.clone());
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

            if !applications.is_empty() {
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
        let narrative_done = Instant::now();

        // 8. Apply events
        let mut applications = Vec::new();
        let offer_source = quest_offer_source(narrative);
        let player_accepts = player_accepts_quest(&text);
        let start_level = self.game_state.player.level;

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
                let outcome = apply_event(&mut self.game_state, sanitized.clone());
                applications.push(EventApplication {
                    event: sanitized,
                    outcome,
                });
                continue;
            }
            let outcome = apply_event(&mut self.game_state, event.clone());
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
        if !applications.is_empty() {
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
                push_section(&mut out, "POWERS", &format_list(&context.player.powers));
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
}

fn sync_stats_from_context(state: &mut InternalGameState, context: &crate::model::game_context::GameContext) {
    for (k, v) in &context.player.stats {
        state.stats.entry(k.to_string()).or_insert(*v);
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
    let threshold = context.world.repetition_threshold.max(1);

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

        if being_hit >= threshold {
            deltas.push(("constitution", 2));
        }
        if mining >= threshold {
            deltas.push(("strength", 1));
        }
        if woodcutting >= threshold {
            deltas.push(("strength", 1));
        }
        if jumping >= threshold {
            deltas.push(("agility", 1));
        }
        if stealth >= threshold {
            deltas.push(("agility", 1));
        }
        if crafting >= threshold {
            deltas.push(("intelligence", 1));
        }
        if fishing >= threshold {
            deltas.push(("luck", 1));
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
