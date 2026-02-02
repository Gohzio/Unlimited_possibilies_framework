use std::collections::{HashMap, HashSet};

use crate::model::game_state::{
    GameStateSnapshot,
    PlayerState,
    Stat,
    Power,
    PartyMember,
    Quest,
    ItemStack,
    LootDrop,
    CurrencyBalance,
    Npc,
    Relationship,
    EquippedItem,
    FactionRep,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InternalGameState {
    pub version: u32,

    pub player: PlayerState,

    /// Authoritative internal stats store
    /// Key = stat id (e.g. "strength", "souls")
    pub stats: HashMap<String, i32>,

    pub powers: HashMap<String, Power>,
    pub party: HashMap<String, PartyMember>,
    pub quests: HashMap<String, Quest>,
    pub inventory: HashMap<String, ItemStack>,
    pub loot: Vec<LootDrop>,
    pub currencies: HashMap<String, i32>,
    pub npcs: HashMap<String, Npc>,
    pub relationships: HashMap<String, Relationship>,
    pub equipment: HashMap<String, EquippedItem>,
    pub factions: HashMap<String, FactionRep>,

    pub flags: HashSet<String>,
    #[serde(default)]
    pub action_counts: HashMap<String, u32>,
    #[serde(default)]
    pub power_usage_counts: HashMap<String, u32>,
    #[serde(default)]
    pub power_evolution_tiers: HashMap<String, u32>,
    #[serde(default)]
    pub set_bonus_tiers: HashMap<String, u32>,
}

impl From<&InternalGameState> for GameStateSnapshot {
    fn from(state: &InternalGameState) -> Self {
        GameStateSnapshot {
            version: state.version,
            player: state.player.clone(),
            stats: state.stats
                .iter()
                .map(|(id, value)| Stat {
                    id: id.clone(),
                    value: *value,
                })
                .collect(),
            powers: state.powers.values().cloned().collect(),
            equipment: state.equipment.values().cloned().collect(),
            party: state.party.values().cloned().collect(),
            quests: state.quests.values().cloned().collect(),
            inventory: state.inventory.values().cloned().collect(),
            loot: state.loot.clone(),
            currencies: state.currencies
                .iter()
                .map(|(currency, amount)| CurrencyBalance {
                    currency: currency.clone(),
                    amount: *amount,
                })
                .collect(),
            npcs: state.npcs.values().cloned().collect(),
            relationships: state.relationships.values().cloned().collect(),
            factions: state.factions.values().cloned().collect(),
            flags: state.flags.iter().cloned().collect(),
        }
    }
}

impl Default for InternalGameState {
    fn default() -> Self {
        let mut stats = HashMap::new();
        stats.insert("strength".into(), 10);
        stats.insert("dexterity".into(), 10);
        stats.insert("intelligence".into(), 10);
        stats.insert("constitution".into(), 10);
        stats.insert("agility".into(), 10);
        stats.insert("luck".into(), 10);

        Self {
            version: 1,

            player: PlayerState {
                name: "Player".to_string(),
                level: 1,
                exp: 0,
                exp_to_next: 100,
                exp_multiplier: 2.0,
                hp: 100,
                max_hp: 100,
                weapons: Vec::new(),
                armor: Vec::new(),
                clothing: Vec::new(),
            },

            stats,

            powers: HashMap::new(),
            party: HashMap::new(),
            quests: HashMap::new(),
            inventory: HashMap::new(),
            loot: Vec::new(),
            currencies: HashMap::new(),
            npcs: HashMap::new(),
            relationships: HashMap::new(),
            equipment: HashMap::new(),
            factions: HashMap::new(),

            flags: HashSet::new(),
            action_counts: HashMap::new(),
            power_usage_counts: HashMap::new(),
            power_evolution_tiers: HashMap::new(),
            set_bonus_tiers: HashMap::new(),
        }
    }
}
