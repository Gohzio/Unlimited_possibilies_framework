use crate::model::{
    internal_game_state::InternalGameState,
    narrative_event::NarrativeEvent,
};
use crate::model::event_result::EventApplyOutcome;

pub fn apply_event(
    state: &mut InternalGameState,
    event: NarrativeEvent,
) -> EventApplyOutcome {
    match event {
        NarrativeEvent::GrantPower { id, name, description } => {
            if state.powers.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("Power '{}' already exists", id),
                };
            }

            state.powers.insert(
                id.clone(),
                crate::model::game_state::Power {
                    id,
                    name,
                    description,
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::AddPartyMember { id, name, role, .. } => {
            if state.party.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("Party member '{}' already exists", id),
                };
            }

            state.party.insert(
                id.clone(),
                crate::model::game_state::PartyMember {
                    id,
                    name,
                    role,
                    hp: 100,
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::AddItem { item_id, quantity } => {
            EventApplyOutcome::Deferred {
                reason: format!(
                    "AddItem '{}' (x{}) deferred: inventory not implemented",
                    item_id, quantity
                ),
            }
        }

        _ => EventApplyOutcome::Deferred {
            reason: "Event not yet implemented".into(),
        },
    }
}
