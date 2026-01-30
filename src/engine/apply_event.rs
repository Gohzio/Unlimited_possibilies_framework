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

        NarrativeEvent::StartQuest {
            id,
            title,
            description,
            rewards,
            sub_quests,
            declinable: _,
        } => {
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
                    description,
                    status: crate::model::game_state::QuestStatus::Active,
                    rewards: rewards.unwrap_or_default(),
                    sub_quests: sub_quests.unwrap_or_default(),
                    rewards_claimed: false,
                },
            );
            EventApplyOutcome::Applied
        }
        NarrativeEvent::UpdateQuest {
            id,
            title,
            description,
            status,
            rewards,
            sub_quests,
        } => {
            let Some(quest) = state.quests.get_mut(&id) else {
                return EventApplyOutcome::Deferred {
                    reason: format!("Quest '{}' not found", id),
                };
            };
            let mut rewards_to_apply: Option<Vec<String>> = None;

            if let Some(title) = title {
                let t = title.trim();
                if !t.is_empty() {
                    quest.title = t.to_string();
                }
            }
            if let Some(description) = description {
                quest.description = description;
            }
            if let Some(status) = status {
                quest.status = status;
            }
            if let Some(rewards) = rewards {
                quest.rewards = rewards;
            }

            if let Some(updates) = sub_quests {
                for update in updates {
                    if let Some(existing) = quest
                        .sub_quests
                        .iter_mut()
                        .find(|s| s.id == update.id)
                    {
                        if let Some(description) = update.description {
                            existing.description = description;
                        }
                        if let Some(completed) = update.completed {
                            existing.completed = completed;
                        }
                    } else {
                        let description = update
                            .description
                            .unwrap_or_else(|| "Unnamed objective".to_string());
                        let completed = update.completed.unwrap_or(false);
                        quest.sub_quests.push(crate::model::game_state::QuestStep {
                            id: update.id,
                            description,
                            completed,
                        });
                    }
                }
            }

            if quest.status == crate::model::game_state::QuestStatus::Completed
                && !quest.rewards_claimed
                && !quest.rewards.is_empty()
            {
                rewards_to_apply = Some(quest.rewards.clone());
                quest.rewards_claimed = true;
            }

            if let Some(rewards) = rewards_to_apply {
                apply_quest_rewards(state, &rewards);
            }

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

        NarrativeEvent::RequestContext { .. } => EventApplyOutcome::Deferred {
            reason: "Context requested".to_string(),
        },

        NarrativeEvent::Unknown { event_type, .. } => EventApplyOutcome::Deferred {
            reason: format!("Unknown event type '{}'", event_type),
        },
    }
}

fn apply_quest_rewards(state: &mut InternalGameState, rewards: &[String]) {
    for reward in rewards {
        let reward = reward.trim();
        if reward.is_empty() {
            continue;
        }

        if let Some((amount, currency)) = parse_currency_reward(reward) {
            let entry = state.currencies.entry(currency).or_insert(0);
            *entry += amount;
            continue;
        }

        let (item, quantity) = split_quantity_suffix(reward);
        if item.trim().is_empty() {
            continue;
        }

        if looks_like_clothing(&item) {
            if !state
                .player
                .clothing
                .iter()
                .any(|c| c.eq_ignore_ascii_case(item.trim()))
            {
                state.player.clothing.push(item.trim().to_string());
            }
        } else if looks_like_armor(&item) {
            if !state
                .player
                .armor
                .iter()
                .any(|c| c.eq_ignore_ascii_case(item.trim()))
            {
                state.player.armor.push(item.trim().to_string());
            }
        } else if looks_like_weapon(&item) {
            if !state
                .player
                .weapons
                .iter()
                .any(|c| c.eq_ignore_ascii_case(item.trim()))
            {
                state.player.weapons.push(item.trim().to_string());
            }
        } else {
            let entry = state.inventory.entry(item.trim().to_string()).or_insert(
                crate::model::game_state::ItemStack {
                    id: item.trim().to_string(),
                    quantity: 0,
                    description: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(quantity.max(1));
        }
    }
}

fn parse_currency_reward(reward: &str) -> Option<(i32, String)> {
    let mut parts = reward.split_whitespace();
    let first = parts.next()?;
    let amount: i32 = first.parse().ok()?;
    let currency = parts.collect::<Vec<_>>().join(" ");
    if currency.is_empty() {
        return None;
    }
    Some((amount, currency))
}

fn split_quantity_suffix(reward: &str) -> (String, u32) {
    let mut parts = reward.rsplitn(2, ' ');
    let last = parts.next().unwrap_or("");
    let rest = parts.next();
    if let Some(rest) = rest {
        let last = last.trim();
        let lower = last.to_lowercase();
        if let Some(num) = lower.strip_prefix('x') {
            if let Ok(qty) = num.parse::<u32>() {
                let name = rest.trim();
                return (name.to_string(), qty.max(1));
            }
        }
    }
    (reward.to_string(), 1)
}

fn looks_like_clothing(item: &str) -> bool {
    let item = item.to_lowercase();
    let keywords = [
        "clothing",
        "underwear",
        "lingerie",
        "bra",
        "panties",
        "briefs",
        "boxers",
        "bikini",
        "swimsuit",
        "swimwear",
        "socks",
        "stockings",
        "tights",
        "leggings",
        "shirt",
        "blouse",
        "top",
        "tee",
        "t-shirt",
        "sweater",
        "hoodie",
        "coat",
        "jacket",
        "pants",
        "jeans",
        "trousers",
        "shorts",
        "skirt",
        "dress",
        "gown",
        "robe",
        "cloak",
        "tunic",
        "vest",
        "armor",
        "armour",
        "helm",
        "helmet",
        "boots",
        "gloves",
        "cap",
        "hat",
        "belt",
        "scarf",
        "gauntlet",
        "greaves",
        "pauldron",
        "cuirass",
    ];
    keywords.iter().any(|k| item.contains(k))
}

fn looks_like_armor(item: &str) -> bool {
    let item = item.to_lowercase();
    let keywords = [
        "armor",
        "armour",
        "helm",
        "helmet",
        "breastplate",
        "cuirass",
        "gauntlet",
        "greaves",
        "pauldron",
        "shield",
        "mail",
        "chainmail",
        "plate",
        "leather armor",
        "leather armour",
    ];
    keywords.iter().any(|k| item.contains(k))
}

fn looks_like_weapon(item: &str) -> bool {
    let item = item.to_lowercase();
    let keywords = [
        "sword",
        "axe",
        "bow",
        "dagger",
        "mace",
        "spear",
        "staff",
        "wand",
        "hammer",
        "halberd",
        "crossbow",
        "rifle",
        "pistol",
        "gun",
        "blade",
    ];
    keywords.iter().any(|k| item.contains(k))
}
