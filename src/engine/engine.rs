use std::sync::mpsc::{Receiver, Sender};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::engine::prompt_builder::PromptBuilder;
use crate::engine::llm_client::{call_lm_studio, test_connection};
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

pub struct Engine {
    rx: Receiver<EngineCommand>,
    tx: Sender<EngineResponse>,

    messages: Vec<Message>,
    game_state: InternalGameState,
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
            EngineCommand::SubmitPlayerInput { text, context } => {
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
                let llm_output = match call_lm_studio(prompt) {
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

                // 5. Parse narrative into structured messages
                let new_messages = parse_narrative(narrative);
                self.messages.extend(new_messages);

                // 6. Decode EVENTS JSON
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

                // 7. Apply events
                let mut applications = Vec::new();

                for event in events {
                    let outcome = apply_event(&mut self.game_state, event.clone());
                    applications.push(EventApplication {
                        event,
                        outcome,
                    });
                }

                // 8. Send state mutation report
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

                // 9. Update UI with full history
                let _ = self.tx.send(
                    EngineResponse::FullMessageHistory(self.messages.clone())
                );
            }

            /* =========================
               Connect to LM Studio
               ========================= */
            EngineCommand::ConnectToLlm => {
                match test_connection() {
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
