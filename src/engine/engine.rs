use std::sync::mpsc::{Receiver, Sender};
use crate::model::message::RoleplaySpeaker; 
use crate::engine::protocol::{EngineCommand, EngineResponse};
use crate::model::message::Message;

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

    // ✅ SINGLE, TOP-LEVEL impl method
    pub fn run(&mut self) {
        // Block until commands arrive
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

                    let _ = self.tx.send(
                        EngineResponse::FullMessageHistory(self.messages.clone())
                    );
                }


                }
            }
        }
    }

