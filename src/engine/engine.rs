use std::sync::mpsc::{Receiver, Sender};

use crate::engine::apply_event::apply_event;
use crate::engine::protocol::{EngineCommand, EngineResponse};

use crate::model::event_result::{
    NarrativeApplyReport,
    EventApplication,
};
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

    // TEMP: fake LLM JSON output
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
    println!("Decoded LLM events: {:?}", events);

    let mut applications = Vec::new();

    for event in events {
        let outcome = apply_event(&mut self.game_state, event.clone());
        applications.push(EventApplication {
            event,
            outcome,
        });
    }

    let report = NarrativeApplyReport { applications };
    let snapshot = self.game_state.snapshot();

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

