use crate::model::game_state::*;
use crate::model::narrative::*;
use crate::model::event_result::*;

/// Errors that can occur when applying narrative events
#[derive(Debug)]
pub enum NarrativeApplyError {
    UnknownPower(String),
    DuplicatePower(String),
    UnknownQuest(String),
    InvalidQuestTransition(String),
    UnknownPartyMember(String),
}

pub fn apply_narrative_events(
    state: &mut GameStateSnapshot,
    response: NarrativeResponse,
) -> NarrativeApplyReport {
    let mut results = Vec::new();

    for event in response.events {
        let result = match event {
            NarrativeEvent::GrantPower { power_id } => {
                match apply_grant_power(state, &power_id) {
                    Ok(_) => EventResult::Applied,
                    Err(e) => EventResult::Rejected {
                        reason: format!("{:?}", e),
                    },
                }
            }

            NarrativeEvent::AddPartyMember { member_id } => {
                match apply_add_party_member(state, &member_id) {
                    Ok(_) => EventResult::Applied,
                    Err(e) => EventResult::Rejected {
                        reason: format!("{:?}", e),
                    },
                }
            }

            NarrativeEvent::UpdateQuest {
                quest_id,
                new_status,
            } => match apply_update_quest(state, &quest_id, new_status) {
                Ok(_) => EventResult::Applied,
                Err(e) => EventResult::Rejected {
                    reason: format!("{:?}", e),
                },
            },
        };

        results.push(result);
    }

    NarrativeApplyReport { results }
}
