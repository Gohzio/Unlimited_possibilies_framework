# Narrative Engine – Roadmap & Architecture

This project is a deterministic narrative RPG engine where **the engine owns truth**  
and an external LLM acts purely as a constrained narrator.

The LLM is *not authoritative*.  
All state changes are validated, applied, or rejected by the engine.

---

## ✅ Locked Architectural Decisions

- The engine **does not load models**
- LLM inference runs **externally**
- Communication uses an **OpenAI-compatible HTTP API**
- **LM Studio** is the primary supported runtime
- A **single LLM** is sufficient (engine enforces constraints)

---

## 🧭 Current Development Roadmap

### 1️⃣ Lock LLM Runtime & API
- [x] Use **LM Studio** as the reference implementation
- [x] OpenAI-compatible `/v1/chat/completions` schema
- [x] HTTP-based, replaceable backend (LM Studio / Ollama / OpenAI)
- [x] No `.gguf` loading inside the engine

---

### 2️⃣ Complete Character / World JSON Structure
- [x] Lock `WorldDefinition` schema
- [x] Five sections:
  - Meta
  - World
  - Narrator
  - Constraints
  - Output
- [x] Player-editable via UI **or** JSON upload
- [x] Engine treats this as authoritative configuration

---

### 3️⃣ Define LLMRequest + LLMResponse
- [ ] Define engine-facing request struct
- [ ] Include:
  - Prompt text
  - Model name
  - Temperature / top-p (later)
- [ ] Define response struct:
  - Raw text
  - Finish reason
  - Token usage (optional)
- [ ] Keep interface backend-agnostic

---

### 4️⃣ 🧱 Prompt Builder (WorldDefinition → Prompt)
- [ ] Render `WorldDefinition` into deterministic system prompt
- [ ] Inject:
  - World rules
  - Narrator role
  - Style guidelines
  - Hard constraints (`must_not`, `must_always`)
- [ ] Append:
  - Current world state snapshot
  - Recent message history
  - Player input
- [ ] Explicit output rules (machine-readable)

---

### 5️⃣ 🔍 Output Parser + Validator
- [ ] Split narration vs events
- [ ] Parse structured event output (JSON)
- [ ] Validate:
  - Schema correctness
  - Stat existence
  - Rule violations
- [ ] Reject / defer invalid events
- [ ] Never trust raw LLM output

---

### 6️⃣ 🔄 Hook Into `EngineCommand::UserInput`
- [ ] On user input:
  - Build prompt
  - Send LLM request
  - Receive response
- [ ] Parse output
- [ ] Apply validated events
- [ ] Emit:
  - Renderable narration
  - System feedback for rejected actions

---

### 7️⃣ 🧪 Test With a Dummy Model
- [ ] Stub LLM client returning fixed responses
- [ ] Test:
  - Happy path
  - Invalid JSON
  - Rule-breaking events
- [ ] Ensure engine never panics on bad output
- [ ] Confirm UI rendering works without live inference

---

## 🧠 Core Philosophy

> **The engine is law.  
> The LLM is a storyteller.  
> The player is always in control.**

This design ensures:
- Deterministic gameplay
- Replaceable AI backends
- Strong modding potential
- No model lock-in
- Long-term maintainability

---

## 🚀 Future (Not Yet Implemented)

- Multi-character narrator styles
- Streaming token support
- Per-world output formatting
- Advanced prompt debugging tools
- Saveable prompt presets

---

*This README reflects locked decisions.  
Changes to these principles should be deliberate and documented.*

| Section                | Player             | Engine    | LLM  |
| ---------------------- | -----------------  | --------  | ---  |
| Meta                   | ✅ edit            | ❌        | ❌   |
| Identity               | ✅ edit (pre-game) | ❌        | ❌   |
| Mechanical Foundations | ✅ start           | ✅ mutate | ❌   |
| Equipment / Inventory  | ✅ start           | ✅ mutate | ❌   |
| Narrative Directives   | ✅ edit anytime    | ❌        | ❌   |
