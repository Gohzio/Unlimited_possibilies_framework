use std::sync::mpsc::{Receiver, Sender};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};

use crate::model::event_result::NarrativeApplyReport;
use crate::model::internal_game_state::InternalGameState;
use crate::model::message::{Message, RoleplaySpeaker};
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
                EngineCommand::UserInput(text) => {
                    self.messages.push(Message::User(text.clone()));
                    self.messages.push(Message::Roleplay {
                        speaker: RoleplaySpeaker::Narrator,
                        text: format!("Echoing back: {}", text),
                    });

                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                }

                EngineCommand::LoadLlm(path) => {
                    self.messages.push(Message::System(format!(
                        "Loaded LLM at: {}",
                        path.display()
                    )));

                    let events = vec![
                        NarrativeEvent::GrantPower {
                            id: "debug_power".into(),
                            name: "Debug Strength".into(),
                            description: "Granted during testing.".into(),
                        }
                    ];

                    let mut results = Vec::new();
                    for event in events {
                        results.push(apply_event(&mut self.game_state, event));
                    }

                    let report = NarrativeApplyReport { results };

                    let _ = self.tx.send(
                        EngineResponse::NarrativeApplied { report }
                    );
                }
            }
        }
    }
}
