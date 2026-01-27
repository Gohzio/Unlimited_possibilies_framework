# 🜂 Narrative Engine RPG 🜂  
*A local-first, LLM-powered roleplaying engine with persistent worlds*

> **Status:** Actively evolving ⚙️  
> **Mood:** Dangerous ideas, rapid iteration  
> **Core Goal:** Let an LLM act as a world, not a chatbot.

---

## ✨ What This Is

A desktop RPG engine built in **Rust + egui**, designed to:

- Treat the LLM as a **narrative actor**
- Maintain a **persistent internal world state**
- Separate **story intent** from **mechanical consequences**
- Allow deep player/world customization via **JSON**
- Stay fully **offline / local-first** (LM Studio compatible)

This is *not* a chat UI.  
It’s a **story engine**.

---

## 🧭 Roadmap / TODO List

### 🧠 Core Narrative Engine
- [ ] **Expand narrative event system**
  - [ ] `combat`
  - [ ] `dialogue`
  - [ ] `travel`
  - [ ] `rest`
  - [ ] `spawn_loot` (currently missing ❌)
  - [ ] `currency_change`
  - [ ] `npc_spawn`
  - [ ] `npc_join_party`
  - [ ] `npc_leave_party`
  - [ ] `relationship_change`
- [ ] Graceful handling of **unknown / future events**
- [ ] Separate **narrative-only** vs **state-mutating** events cleanly

---

### 🎭 Narrative Presentation
- [ ] **Speaker-based text colors**
  - Player
  - Narrator
  - NPCs
  - System
- [ ] *Italic formatting for emotions / internal thoughts*
- [ ] Better spacing & flow for long narrative passages
- [ ] Remove empty message artifacts from partial LLM outputs

---

### 👤 Player Creation & Editing
- [ ] Fix Player Creation Panel
  - [ ] Edit **stats**
  - [ ] Edit **powers**
  - [ ] Edit **features**
  - [ ] Edit **inventory**
- [ ] Remove reliance on **manual JSON editing**
- [ ] Live validation of player config
- [ ] Preview player summary before starting session

---

### 🧑‍🤝‍🧑 Party System
- [ ] Allow NPCs to be added as **party members**
- [ ] Party tab auto-updates from narrative events
- [ ] Expand Party tab UI
- [ ] Button to generate **Text → Image prompt** per party member
  - (For use in external image generation tools)
- [ ] Individual party member sheets
- [ ] Party-wide status effects

---

### 📜 World & NPC Management
- [ ] **Local NPC Tab** (Left panel)
  - [ ] Persistent NPCs not in party
  - [ ] Relationship tracking
  - [ ] Known locations & factions
- [ ] World state auto-expands as LLM introduces new concepts

---

### 🎒 Items, Loot & Economy
- [ ] Functional **loot drops**
- [ ] Currency system
  - [ ] Gold / credits / setting-based currency
  - [ ] Add / remove / spend events
- [ ] Inventory stacking & descriptions
- [ ] Item rarity & flavor text

---

### 📈 Progression
- [ ] XP bar
- [ ] Level-up system
- [ ] Level-up events emitted by LLM
- [ ] Stat growth & perk unlocks

---

### 💾 Persistence
- [ ] **Save Session** button
- [ ] Load previous sessions
- [ ] Autosave checkpoints
- [ ] Session metadata (last played, world name, player name)

---

### 🧪 LLM Integration
- [ ] LLM-driven **power creation**
- [ ] LLM-assisted item descriptions
- [ ] LLM-assisted NPC backstories
- [ ] Better prompt contracts for dual-output:
  - Narrative text
  - Structured events JSON

---

### 🖼️ Immersion Features
- [ ] Embed JSON metadata into uploaded **PNG images**
  - Player portraits
  - NPC portraits
  - World maps
- [ ] Read embedded metadata back into the engine
- [ ] Visual identity tied directly to game state

---

### 🖥️ UI / UX Polish
- [ ] Confine center text strictly to center panel
- [ ] Copy/paste support everywhere (✅ mostly there)
- [ ] Font scaling **independent of UI scaling**
- [ ] Move settings/options to **compact icon buttons**
- [ ] Reduce visual noise
- [ ] Improve scroll behavior in long sessions

---

## 🧱 Architectural Principles

- **Narrative first, mechanics second**
- **LLM may suggest — engine decides**
- **Never crash on creativity**
- **Everything serializable**
- **Local-first, inspectable state**

---

## 🛠️ Tech Stack

- **Rust**
- **eframe / egui**
- **Serde (JSON-first design)**
- **LM Studio (local LLMs)**

---

## 🚀 Long-Term Vision

This engine should eventually be able to:

- Run full tabletop-style campaigns
- Act as a solo GM
- Support multiple worlds & genres
- Become a toolkit for narrative experimentation

---

> *“If the LLM surprises the engine, the engine should learn — not panic.”*


| Section                | Player             | Engine    | LLM  |
| ---------------------- | -----------------  | --------  | ---  |
| Meta                   | ✅ edit            | ❌        | ❌   |
| Identity               | ✅ edit (pre-game) | ❌        | ❌   |
| Mechanical Foundations | ✅ start           | ✅ mutate | ❌   |
| Equipment / Inventory  | ✅ start           | ✅ mutate | ❌   |
| Narrative Directives   | ✅ edit anytime    | ❌        | ❌   |
