# Unlimited Possibilities Framework (UPF)

Local-first, stateful, LLM‑driven RPG engine built in Rust + egui.

UPF is for people who want more than a chat UI. It is a narrative engine with explicit game state, events, and rules, so your story doesn’t collapse when context drifts. You get RPG structure (party, quests, inventory, NPCs, factions) without giving up freeform storytelling.

## Why you’d want to use this

- **Stateful RPG, not just chat.** The engine owns game state; the LLM proposes, the engine applies. This keeps continuity and prevents “LLM‑only memory loss.”
- **Structured events.** All game changes are expressed as JSON events, making the system deterministic and debuggable.
- **Local‑first.** Runs with local models (LM Studio), no required cloud.
- **Inspectable saves.** Everything is serialized; you can load, save, and audit game state.
- **UI for control.** Edit player/world data, manage party/NPCs, tune settings, and lock fields you don’t want the LLM to overwrite.

## How it differs from SillyTavern

SillyTavern is a flexible chat front‑end for LLMs. UPF is an engine with rules, state, and a structured event protocol.

Key differences:
- **Source of truth:** SillyTavern is chat-first; UPF is state-first. In UPF, the game engine owns the authoritative state and the LLM cannot directly mutate it.
- **Determinism:** UPF uses explicit events (JSON) for all changes. This makes outcomes reproducible and easier to debug.
- **RPG systems:** UPF includes built‑in RPG concepts (quests, inventory, factions, party, NPC tracking, equipment).
- **Guardrails:** UPF enforces rules (e.g., loot handling, party updates), which reduces narrative drift.
- **Developer focus:** UPF is designed for extendable engine logic, not just prompt/character management.

If you want a chat UI with lots of front‑end tooling, SillyTavern is great. If you want a playable, stateful RPG engine that keeps its world consistent, UPF is the better fit.

## 🔒 Privacy & Data

- Prompts are assembled locally and sent to the configured LLM endpoint.
- By default this is a local LM Studio server, but if you change the base URL your prompts may be sent off‑device.
- Prompts can include world data, player info, and recent chat history depending on your settings.

## ✅ Recommended Backend (LM Studio)

LM Studio is the recommended backend for UPF. It is stable, fast, and supports structured output, which improves event reliability.

If you want the best results:
- Use LM Studio with **OpenAI‑compatible** mode.
- Enable **structured EVENTS** in the app options.
- Paste the JSON schema below into LM Studio’s **Structured Output** section.

Where to add the schema in LM Studio:
1) Open LM Studio and select your model.
2) Go to the **Developer / OpenAI‑compatible API** page.
3) In **Structured Output**, choose **JSON Schema**.
4) Paste the schema below into the schema box.
5) Save/apply, then enable **Use structured EVENTS** in UPF options.

<details>
<summary>JSON Schema for EVENTS (copy/paste)</summary>

<div style="max-height: 280px; overflow: auto; border: 1px solid #444; padding: 8px; margin-top: 8px;">

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "NarrativeEvents",
  "type": "array",
  "items": {
    "oneOf": [
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name", "description"],
        "properties": {
          "type": { "const": "grant_power" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "description"],
        "properties": {
          "type": { "const": "combat" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "speaker", "text"],
        "properties": {
          "type": { "const": "dialogue" },
          "speaker": { "type": "string" },
          "text": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "from", "to"],
        "properties": {
          "type": { "const": "travel" },
          "from": { "type": "string" },
          "to": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "description"],
        "properties": {
          "type": { "const": "rest" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "recipe"],
        "properties": {
          "type": { "const": "craft" },
          "recipe": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "quality": { "type": "string" },
          "result": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "resource"],
        "properties": {
          "type": { "const": "gather" },
          "resource": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "quality": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name", "role"],
        "properties": {
          "type": { "const": "add_party_member" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "party_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" },
          "clothing_add": { "type": "array", "items": { "type": "string" } },
          "clothing_remove": { "type": "array", "items": { "type": "string" } },
          "weapons_add": { "type": "array", "items": { "type": "string" } },
          "weapons_remove": { "type": "array", "items": { "type": "string" } },
          "armor_add": { "type": "array", "items": { "type": "string" } },
          "armor_remove": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "section", "id", "name"],
        "properties": {
          "type": { "const": "section_card_upsert" },
          "section": { "type": "string" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "status": { "type": "string" },
          "details": { "type": "string" },
          "notes": { "type": "string" },
          "tags": { "type": "array", "items": { "type": "string" } },
          "items": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "section", "id"],
        "properties": {
          "type": { "const": "section_card_remove" },
          "section": { "type": "string" },
          "id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "player_card_update" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "status": { "type": "string" },
          "details": { "type": "string" },
          "notes": { "type": "string" },
          "tags": { "type": "array", "items": { "type": "string" } },
          "items": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "minutes"],
        "properties": {
          "type": { "const": "time_passed" },
          "minutes": { "type": "integer", "minimum": 1 },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "name", "role"],
        "properties": {
          "type": { "const": "npc_spawn" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "npc_join_party" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" },
          "clothing": { "type": "array", "items": { "type": "string" } },
          "weapons": { "type": "array", "items": { "type": "string" } },
          "armor": { "type": "array", "items": { "type": "string" } }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type"],
        "properties": {
          "type": { "const": "npc_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "role": { "type": "string" },
          "details": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "npc_despawn" },
          "id": { "type": "string" },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "npc_leave_party" },
          "id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "subject_id", "target_id", "delta"],
        "properties": {
          "type": { "const": "relationship_change" },
          "subject_id": { "type": "string" },
          "target_id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "stat_id", "delta"],
        "properties": {
          "type": { "const": "modify_stat" },
          "stat_id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "amount"],
        "properties": {
          "type": { "const": "add_exp" },
          "amount": { "type": "integer", "minimum": 1 }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "levels"],
        "properties": {
          "type": { "const": "level_up" },
          "levels": { "type": "integer", "minimum": 1 }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id", "slot"],
        "properties": {
          "type": { "const": "equip_item" },
          "item_id": { "type": "string" },
          "slot": { "type": "string" },
          "set_id": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id"],
        "properties": {
          "type": { "const": "unequip_item" },
          "item_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "title", "description"],
        "properties": {
          "type": { "const": "start_quest" },
          "id": { "type": "string" },
          "title": { "type": "string" },
          "description": { "type": "string" },
          "difficulty": { "type": "string" },
          "negotiable": { "type": "boolean" },
          "reward_options": { "type": "array", "items": { "type": "string" } },
          "rewards": { "type": "array", "items": { "type": "string" } },
          "sub_quests": {
            "type": "array",
            "items": {
              "type": "object",
              "additionalProperties": false,
              "required": ["id", "description"],
              "properties": {
                "id": { "type": "string" },
                "description": { "type": "string" },
                "completed": { "type": "boolean" }
              }
            }
          },
          "declinable": { "type": "boolean" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "update_quest" },
          "id": { "type": "string" },
          "title": { "type": "string" },
          "description": { "type": "string" },
          "status": { "type": "string", "enum": ["active", "completed", "failed"] },
          "difficulty": { "type": "string" },
          "negotiable": { "type": "boolean" },
          "reward_options": { "type": "array", "items": { "type": "string" } },
          "rewards": { "type": "array", "items": { "type": "string" } },
          "sub_quests": {
            "type": "array",
            "items": {
              "type": "object",
              "additionalProperties": false,
              "required": ["id"],
              "properties": {
                "id": { "type": "string" },
                "description": { "type": "string" },
                "completed": { "type": "boolean" }
              }
            }
          }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "topics"],
        "properties": {
          "type": { "const": "request_context" },
          "topics": {
            "oneOf": [
              { "type": "string" },
              { "type": "array", "items": { "type": "string" } }
            ]
          }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "flag"],
        "properties": {
          "type": { "const": "set_flag" },
          "flag": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "reason"],
        "properties": {
          "type": { "const": "request_retcon" },
          "reason": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item_id", "quantity"],
        "properties": {
          "type": { "const": "add_item" },
          "item_id": { "type": "string" },
          "quantity": { "type": "integer", "minimum": 1 },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item"],
        "properties": {
          "type": { "const": "drop" },
          "item": { "type": "string" },
          "quantity": { "type": "integer" },
          "description": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "item"],
        "properties": {
          "type": { "const": "spawn_loot" },
          "item": { "type": "string" },
          "quantity": { "type": "integer" },
          "description": { "type": "string" },
          "set_id": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "currency", "delta"],
        "properties": {
          "type": { "const": "currency_change" },
          "currency": { "type": "string" },
          "delta": { "type": "integer" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "name"],
        "properties": {
          "type": { "const": "faction_spawn" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "kind": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id"],
        "properties": {
          "type": { "const": "faction_update" },
          "id": { "type": "string" },
          "name": { "type": "string" },
          "kind": { "type": "string" },
          "description": { "type": "string" }
        }
      },
      {
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "id", "delta"],
        "properties": {
          "type": { "const": "faction_rep_change" },
          "id": { "type": "string" },
          "delta": { "type": "integer" }
        }
      }
    ]
  }
}
```

</div>
</details>

## Build Instructions (Windows, Linux, macOS)

### Prerequisites

- Rust toolchain (stable) with Cargo
- Git

Verify:

```bash
rustc --version
cargo --version
git --version
```

### Linux

1) Install system deps for egui/eframe (X11/Wayland and OpenGL). If you already have a working Rust+OpenGL toolchain on your desktop, you can skip this. Otherwise:

Examples:
- Debian/Ubuntu:
  ```bash
  sudo apt install libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev pkg-config
  ```
- Fedora:
  ```bash
  sudo dnf install libX11-devel libxkbcommon-devel wayland-devel mesa-libGL-devel pkgconf
  ```

2) Clone and build:

```bash
git clone <REPO_URL>
cd Unlimited_possibilies_framework
cargo build --release
```

3) Run:

```bash
./target/release/Unlimited_possibilities_framework
```

### macOS

1) Install Rust. If prompted, install Xcode Command Line Tools.

2) Clone and build:

```bash
git clone <REPO_URL>
cd Unlimited_possibilies_framework
cargo build --release
```

3) Run:

```bash
./target/release/Unlimited_possibilities_framework
```

### Windows

1) Install Rust with the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`).
2) Install the Visual Studio Build Tools (C++ build tools).

3) Clone and build (PowerShell):

```powershell
git clone <REPO_URL>
cd Unlimited_possibilies_framework
cargo build --release
```

4) Run:

```powershell
.\target\release\Unlimited_possibilities_framework.exe
```

## 🧱 Architectural Principles

- **Narrative first, mechanics second**
- **LLM may suggest — engine decides**
- **Never crash on creativity**
- **Everything serializable**
- **Local‑first, inspectable state**

---

## 🛠️ Tech Stack

- **Rust**
- **eframe / egui**
- **Serde (JSON-first design)**
- **LM Studio (local LLMs)**

---

## 🚀 Long‑Term Vision

This engine should eventually be able to:

- Run full tabletop‑style campaigns
- Act as a solo GM
- Support multiple worlds & genres
- Become a toolkit for narrative experimentation

---

> “If the LLM surprises the engine, the engine should learn — not panic.”
