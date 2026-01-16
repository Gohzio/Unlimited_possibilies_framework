use std::sync::mpsc::{Receiver, Sender};

use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::event_result::{EventResult, NarrativeApplyReport};
use crate::model::message::{Message, RoleplaySpeaker};

pub struct Engine {
    rx: Receiver<EngineCommand>,
    tx: Sender<EngineResponse>,
    messages: Vec<Message>,
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
        }
    }

    /// Main engine loop (runs on background thread)
    pub fn run(&mut self) {
        // Block until commands arrive
        while let Ok(cmd) = self.rx.recv() {
            match cmd {
                EngineCommand::UserInput(text) => {
                    // Record user input
                    self.messages.push(Message::User(text.clone()));

                    // TEMP: fake narrator response
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

                    // TEMP: fake narrative application report
                    let report = NarrativeApplyReport {
                        results: vec![
                            EventResult::Applied,
                        ],
                    };

                    // Send logic-layer result first
                    let _ = self.tx.send(
                        EngineResponse::NarrativeApplied { report }
                    );

                    // Then send updated UI state
                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                }
            }
        }
    }
}

