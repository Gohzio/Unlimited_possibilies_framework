use std::sync::mpsc::{Receiver, Sender};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::engine::prompt_builder::PromptBuilder;
use crate::engine::llm_client::{call_lm_studio, test_connection};
use crate::engine::narrative_parser::parse_narrative;

use crate::model::event_result::{
    NarrativeApplyReport,
    EventApplication,
};
use crate::model::internal_game_state::InternalGameState;
use crate::model::message::Message;

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
                   Player input → Prompt → LLM
                   ========================= */
                EngineCommand::SubmitPlayerInput { text, context } => {
                    // 1. Record player input
                    self.messages.push(Message::User(text.clone()));

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

                    // 4. Split narrative vs events
                    let (narrative, _events_json) =
                        llm_output.split_once("EVENTS:")
                            .unwrap_or((&llm_output, ""));

                    // 5. Parse narrative into structured messages
                    let new_messages = parse_narrative(narrative);
                    self.messages.extend(new_messages);

                    // 6. Send updated messages to UI
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
                   Load LLM (legacy / test path)
                   ========================= */
                EngineCommand::LoadLlm(path) => {
                    self.messages.push(Message::System(format!(
                        "Loaded LLM at: {}",
                        path.display()
                    )));

                    let fake_llm_json = r#"
                    [
                      {
                        "type": "grant_power",
                        "id": "fireball",
                        "name": "Fireball",
                        "description": "Throws a ball of fire"
                      }
                    ]
                    "#;

                    let events = crate::model::llm_decode::decode_llm_events(fake_llm_json)
                        .expect("LLM JSON should decode");

                    let mut applications = Vec::new();

                    for event in events {
                        let outcome = apply_event(&mut self.game_state, event.clone());
                        applications.push(EventApplication {
                            event,
                            outcome,
                        });
                    }

                    let report = NarrativeApplyReport { applications };
                    let snapshot = (&self.game_state).into();

                    let _ = self.tx.send(
                        EngineResponse::NarrativeApplied {
                            report,
                            snapshot,
                        }
                    );
                }
            }
        }
    }
}
