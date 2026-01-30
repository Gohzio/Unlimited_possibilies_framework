use crate::model::{
    internal_game_state::InternalGameState,
    narrative_event::NarrativeEvent,
};
use crate::model::event_result::EventApplyOutcome;
/// Apply a NarrativeEvent to the InternalGameState, returning the outcome

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

        NarrativeEvent::Combat { .. }
        | NarrativeEvent::Dialogue { .. }
        | NarrativeEvent::Travel { .. }
        | NarrativeEvent::Rest { .. } => {
            // Narrative-only events: recorded by the LLM but do not mutate state.
            EventApplyOutcome::Applied
        }

        NarrativeEvent::AddPartyMember { id, name, role } => {
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
                    clothing: Vec::new(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::NpcSpawn { id, name, role, details } => {
            if state.npcs.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("NPC '{}' already exists", id),
                };
            }

            state.npcs.insert(
                id.clone(),
                crate::model::game_state::Npc {
                    id,
                    name,
                    role,
                    notes: details.unwrap_or_default(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::NpcJoinParty { id, name, role, details: _ } => {
            if state.party.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("Party member '{}' already exists", id),
                };
            }

            let (name, role) = if let Some(npc) = state.npcs.remove(&id) {
                (npc.name, npc.role)
            } else {
                let Some(name) = name else {
                    return EventApplyOutcome::Rejected {
                        reason: format!("NPC '{}' not found and no name provided", id),
                    };
                };
                let Some(role) = role else {
                    return EventApplyOutcome::Rejected {
                        reason: format!("NPC '{}' not found and no role provided", id),
                    };
                };
                (name, role)
            };

            state.party.insert(
                id.clone(),
                crate::model::game_state::PartyMember {
                    id,
                    name,
                    role,
                    hp: 100,
                    clothing: Vec::new(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::NpcLeaveParty { id } => {
            let Some(member) = state.party.remove(&id) else {
                return EventApplyOutcome::Rejected {
                    reason: format!("Party member '{}' not found", id),
                };
            };

            state.npcs.insert(
                id.clone(),
                crate::model::game_state::Npc {
                    id,
                    name: member.name,
                    role: member.role,
                    notes: String::new(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::RelationshipChange { subject_id, target_id, delta } => {
            let key = format!("{}::{}", subject_id, target_id);
            let entry = state.relationships.entry(key.clone()).or_insert(
                crate::model::game_state::Relationship {
                    subject_id,
                    target_id,
                    value: 0,
                },
            );
            entry.value += delta;
            EventApplyOutcome::Applied
        }

        NarrativeEvent::ModifyStat { stat_id, delta } => {
            match state.stats.get_mut(&stat_id) {
                Some(value) => {
                    *value += delta;
                    EventApplyOutcome::Applied
                }
                None => EventApplyOutcome::Deferred {
                    reason: format!("Unknown stat '{}'", stat_id),
                },
            }
        }

        NarrativeEvent::StartQuest { id, title, description: _ } => {
            if state.quests.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("Quest '{}' already exists", id),
                };
            }
            state.quests.insert(
                id.clone(),
                crate::model::game_state::Quest {
                    id,
                    title,
                    status: crate::model::game_state::QuestStatus::Active,
                },
            );
            EventApplyOutcome::Applied
        }

        NarrativeEvent::SetFlag { flag } => {
            state.flags.insert(flag);
            EventApplyOutcome::Applied
        }

        NarrativeEvent::AddItem { item_id, quantity } => {
            let entry = state.inventory.entry(item_id.clone()).or_insert(
                crate::model::game_state::ItemStack {
                    id: item_id.clone(),
                    quantity: 0,
                    description: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(quantity);
            EventApplyOutcome::Applied
        }

        NarrativeEvent::Drop { item, quantity, description } => {
            let qty = quantity.unwrap_or(1).max(1) as u32;
            state.loot.push(crate::model::game_state::LootDrop {
                item,
                quantity: qty,
                description,
            });
            EventApplyOutcome::Applied
        }

        NarrativeEvent::SpawnLoot { item, quantity, description } => {
            let qty = quantity.unwrap_or(1).max(1) as u32;
            state.loot.push(crate::model::game_state::LootDrop {
                item,
                quantity: qty,
                description,
            });
            EventApplyOutcome::Applied
        }

        NarrativeEvent::CurrencyChange { currency, delta } => {
            let entry = state.currencies.entry(currency).or_insert(0);
            *entry += delta;
            EventApplyOutcome::Applied
        }

        NarrativeEvent::RequestRetcon { reason } => EventApplyOutcome::Deferred {
            reason: format!("Retcon requested: {}", reason),
        },

        NarrativeEvent::Unknown { event_type, .. } => EventApplyOutcome::Deferred {
            reason: format!("Unknown event type '{}'", event_type),
        },
    }
}
