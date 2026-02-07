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
