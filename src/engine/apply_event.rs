use crate::model::{
    internal_game_state::InternalGameState,
    narrative_event::NarrativeEvent,
};
use crate::model::event_result::EventApplyOutcome;

fn apply_exp_gain(state: &mut InternalGameState, amount: i32, multiplier: f32) {
    let mut exp = (state.player.exp + amount).max(0);
    let mut next = state.player.exp_to_next.max(1);
    let mult = multiplier.max(1.0);

    while exp >= next {
        exp -= next;
        state.player.level = state.player.level.saturating_add(1);
        next = ((next as f32) * mult).round() as i32;
        if next < 1 {
            next = 1;
        }
    }

    state.player.exp = exp;
    state.player.exp_to_next = next;
}

fn apply_level_ups(state: &mut InternalGameState, levels: u32, multiplier: f32, reset_exp: bool) {
    let mut next = state.player.exp_to_next.max(1);
    let mult = multiplier.max(1.0);

    for _ in 0..levels {
        state.player.level = state.player.level.saturating_add(1);
        next = ((next as f32) * mult).round() as i32;
        if next < 1 {
            next = 1;
        }
    }

    if reset_exp {
        state.player.exp = 0;
    }
    state.player.exp_to_next = next;
}
/// Apply a NarrativeEvent to the InternalGameState, returning the outcome

pub fn apply_event(
    state: &mut InternalGameState,
    event: NarrativeEvent,
) -> EventApplyOutcome {
    match event {
        NarrativeEvent::GrantPower { id, name, description } => {
            if let Some(existing) = state.powers.get_mut(&id) {
                existing.name = name;
                existing.description = description;
                return EventApplyOutcome::Applied;
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
        NarrativeEvent::Craft {
            recipe,
            quantity,
            quality,
            result,
            set_id,
        } => {
            let item = result.unwrap_or_else(|| recipe.clone());
            let qty = quantity.unwrap_or(1).max(1);
            let desc = quality.map(|q| format!("Crafted quality: {}", q));
            state.loot.push(crate::model::game_state::LootDrop {
                item,
                quantity: qty,
                description: desc,
                set_id,
            });
            EventApplyOutcome::Applied
        }
        NarrativeEvent::Gather {
            resource,
            quantity,
            quality,
            set_id,
        } => {
            let qty = quantity.unwrap_or(1).max(1);
            let desc = quality.map(|q| format!("Gathered quality: {}", q));
            state.loot.push(crate::model::game_state::LootDrop {
                item: resource,
                quantity: qty,
                description: desc,
                set_id,
            });
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
                    details: String::new(),
                    hp: 100,
                    clothing: Vec::new(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::PartyUpdate {
            id,
            name,
            role,
            details,
            clothing,
        } => {
            let Some(member) = state.party.get_mut(&id) else {
                return EventApplyOutcome::Deferred {
                    reason: format!("Party member '{}' not found", id),
                };
            };

            if let Some(name) = name {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    member.name = trimmed.to_string();
                }
            }
            if let Some(role) = role {
                let trimmed = role.trim();
                if !trimmed.is_empty() {
                    member.role = trimmed.to_string();
                }
            }
            if let Some(details) = details {
                let trimmed = details.trim();
                if !trimmed.is_empty() {
                    if member.details.trim().is_empty() {
                        member.details = trimmed.to_string();
                    } else if !member.details.contains(trimmed) {
                        member.details = format!("{}\n{}", member.details.trim_end(), trimmed);
                    }
                }
            }
            if let Some(clothing) = clothing {
                if !clothing.is_empty() {
                    member.clothing = clothing;
                }
            }

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
                    nearby: true,
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
                    details: String::new(),
                    hp: 100,
                    clothing: Vec::new(),
                },
            );

            EventApplyOutcome::Applied
        }

        NarrativeEvent::NpcUpdate {
            id,
            name,
            role,
            details,
        } => {
            let entry = state.npcs.entry(id.clone()).or_insert(
                crate::model::game_state::Npc {
                    id,
                    name: name.clone().unwrap_or_else(|| "Unknown".to_string()),
                    role: role.clone().unwrap_or_else(|| "Unknown".to_string()),
                    notes: String::new(),
                    nearby: true,
                },
            );
            if let Some(name) = name {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    entry.name = trimmed.to_string();
                }
            }
            if let Some(role) = role {
                let trimmed = role.trim();
                if !trimmed.is_empty() {
                    entry.role = trimmed.to_string();
                }
            }
            if let Some(details) = details {
                let trimmed = details.trim();
                if !trimmed.is_empty() && !entry.notes.contains(trimmed) {
                    if !entry.notes.is_empty() {
                        entry.notes.push_str(" | ");
                    }
                    entry.notes.push_str(trimmed);
                }
            }
            entry.nearby = true;
            EventApplyOutcome::Applied
        }

        NarrativeEvent::NpcDespawn { id, reason: _ } => {
            if let Some(npc) = state.npcs.get_mut(&id) {
                npc.nearby = false;
                return EventApplyOutcome::Applied;
            }
            EventApplyOutcome::Deferred {
                reason: format!("NPC '{}' not found", id),
            }
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
                    nearby: true,
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
        NarrativeEvent::EquipItem {
            item_id,
            slot,
            set_id,
            description,
        } => {
            let key = item_id.clone();
            let slot_norm = slot.trim().to_lowercase();
            state.equipment.insert(
                key.clone(),
                crate::model::game_state::EquippedItem {
                    item_id: key.clone(),
                    slot: slot_norm.clone(),
                    set_id,
                    description,
                },
            );
            if let Some(item) = state.inventory.get_mut(&key) {
                if item.quantity > 1 {
                    item.quantity -= 1;
                } else {
                    state.inventory.remove(&key);
                }
            }
            match slot_norm.as_str() {
                "weapon" | "weapons" => {
                    if !state.player.weapons.iter().any(|w| w.eq_ignore_ascii_case(&key)) {
                        state.player.weapons.push(key);
                    }
                }
                "armor" | "armour" => {
                    if !state.player.armor.iter().any(|a| a.eq_ignore_ascii_case(&key)) {
                        state.player.armor.push(key);
                    }
                }
                "clothing" => {
                    if !state.player.clothing.iter().any(|c| c.eq_ignore_ascii_case(&key)) {
                        state.player.clothing.push(key);
                    }
                }
                _ => {}
            }
            EventApplyOutcome::Applied
        }
        NarrativeEvent::UnequipItem { item_id } => {
            let key = item_id.clone();
            state.equipment.remove(&key);
            state.player.weapons.retain(|w| !w.eq_ignore_ascii_case(&key));
            state.player.armor.retain(|a| !a.eq_ignore_ascii_case(&key));
            state.player.clothing.retain(|c| !c.eq_ignore_ascii_case(&key));
            let entry = state.inventory.entry(key).or_insert(
                crate::model::game_state::ItemStack {
                    id: item_id,
                    quantity: 0,
                    description: None,
                    set_id: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(1);
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
        NarrativeEvent::AddExp { amount } => {
            let mult = state.player.exp_multiplier.max(1.0);
            apply_exp_gain(state, amount, mult);
            EventApplyOutcome::Applied
        }
        NarrativeEvent::LevelUp { levels } => {
            let mult = state.player.exp_multiplier.max(1.0);
            apply_level_ups(state, levels, mult, false);
            EventApplyOutcome::Applied
        }

        NarrativeEvent::StartQuest {
            id,
            title,
            description,
            difficulty,
            negotiable,
            reward_options,
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
                    difficulty,
                    negotiable: negotiable.unwrap_or(false),
                    reward_options: reward_options.unwrap_or_default(),
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
            difficulty,
            negotiable,
            reward_options,
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
            if let Some(diff) = difficulty {
                let trimmed = diff.trim();
                if !trimmed.is_empty() {
                    quest.difficulty = Some(trimmed.to_string());
                }
            }
            if let Some(neg) = negotiable {
                quest.negotiable = neg;
            }
            if let Some(options) = reward_options {
                quest.reward_options = options;
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

        NarrativeEvent::AddItem { item_id, quantity, set_id } => {
            let set_id_clone = set_id.clone();
            let entry = state.inventory.entry(item_id.clone()).or_insert(
                crate::model::game_state::ItemStack {
                    id: item_id.clone(),
                    quantity: 0,
                    description: None,
                    set_id: set_id_clone,
                },
            );
            entry.quantity = entry.quantity.saturating_add(quantity);
            if entry.set_id.is_none() {
                entry.set_id = set_id;
            }
            EventApplyOutcome::Applied
        }

        NarrativeEvent::Drop { item, quantity, description, set_id } => {
            let qty = quantity.unwrap_or(1).max(1) as u32;
            state.loot.push(crate::model::game_state::LootDrop {
                item,
                quantity: qty,
                description,
                set_id,
            });
            EventApplyOutcome::Applied
        }

        NarrativeEvent::SpawnLoot { item, quantity, description, set_id } => {
            let qty = quantity.unwrap_or(1).max(1) as u32;
            state.loot.push(crate::model::game_state::LootDrop {
                item,
                quantity: qty,
                description,
                set_id,
            });
            EventApplyOutcome::Applied
        }

        NarrativeEvent::CurrencyChange { currency, delta } => {
            let entry = state.currencies.entry(currency).or_insert(0);
            *entry += delta;
            EventApplyOutcome::Applied
        }
        NarrativeEvent::FactionSpawn {
            id,
            name,
            kind,
            description,
        } => {
            if state.factions.contains_key(&id) {
                return EventApplyOutcome::Rejected {
                    reason: format!("Faction '{}' already exists", id),
                };
            }
            state.factions.insert(
                id.clone(),
                crate::model::game_state::FactionRep {
                    id,
                    name,
                    kind,
                    description,
                    reputation: 0,
                },
            );
            EventApplyOutcome::Applied
        }
        NarrativeEvent::FactionUpdate {
            id,
            name,
            kind,
            description,
        } => {
            let Some(faction) = state.factions.get_mut(&id) else {
                return EventApplyOutcome::Deferred {
                    reason: format!("Faction '{}' not found", id),
                };
            };
            if let Some(name) = name {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    faction.name = trimmed.to_string();
                }
            }
            if let Some(kind) = kind {
                let trimmed = kind.trim();
                if !trimmed.is_empty() {
                    faction.kind = Some(trimmed.to_string());
                }
            }
            if let Some(description) = description {
                let trimmed = description.trim();
                if !trimmed.is_empty() {
                    faction.description = Some(trimmed.to_string());
                }
            }
            EventApplyOutcome::Applied
        }
        NarrativeEvent::FactionRepChange { id, delta } => {
            let entry = state.factions.entry(id.clone()).or_insert(
                crate::model::game_state::FactionRep {
                    id,
                    name: "Unknown Faction".to_string(),
                    kind: None,
                    description: None,
                    reputation: 0,
                },
            );
            entry.reputation += delta;
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

        let (item_raw, quantity) = split_quantity_suffix(reward);
        let (item, set_id) = extract_set_id(&item_raw);
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
                    set_id: None,
                },
            );
            entry.quantity = entry.quantity.saturating_add(quantity.max(1));
            if entry.set_id.is_none() {
                entry.set_id = set_id;
            }
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

fn extract_set_id(raw: &str) -> (String, Option<String>) {
    let mut item = raw.to_string();
    let mut set_id: Option<String> = None;

    if let Some((before, rest)) = raw.split_once("(set:") {
        if let Some((set, after)) = rest.split_once(')') {
            set_id = Some(set.trim().to_string());
            item = format!("{}{}", before, after).trim().to_string();
        }
    } else if let Some((before, rest)) = raw.split_once("[set:") {
        if let Some((set, after)) = rest.split_once(']') {
            set_id = Some(set.trim().to_string());
            item = format!("{}{}", before, after).trim().to_string();
        }
    }

    (item, set_id)
}

fn upsert_equipment(
    state: &mut InternalGameState,
    item_id: &str,
    slot: &str,
    set_id: Option<String>,
    description: Option<String>,
) {
    state.equipment.insert(
        item_id.to_string(),
        crate::model::game_state::EquippedItem {
            item_id: item_id.to_string(),
            slot: slot.to_string(),
            set_id,
            description,
        },
    );
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
