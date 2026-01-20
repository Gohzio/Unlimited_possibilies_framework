# 🧭 Unlimited Possibilities Framework — Development Roadmap

> **Goal**  
> A fully offline, moddable RPG / narrative framework driven by structured events,  
> with optional LLM integration — *never required*.

---

## ✅ Phase 0 — Foundations (Mostly Done)

> Core architecture, data flow, and safety rails

- [x] Project compiles and runs
- [x] Engine ↔ UI thread separation
- [x] InternalGameState (authoritative mutable state)
- [x] NarrativeEvent enum (typed world changes)
- [x] apply_event system with Applied / Rejected / Deferred outcomes
- [x] NarrativeApplyReport for event application results
- [x] GameStateSnapshot (read-only, UI/LLM safe)
- [x] Basic egui UI with message log
- [x] Fake / stub LLM JSON decoding (`llm_decode`)

---

## 🧩 Phase 1 — State Visibility & Trust (Current Focus)

> “If we can’t see it, we can’t reason about it.”

- [ ] Engine emits GameStateSnapshot with NarrativeApplyReport
- [ ] UI stores latest snapshot in UiState
- [ ] Sidebar panel renders snapshot data (read-only)
- [ ] Temporary adapter maps snapshot → display rows
- [ ] Deferred events show explicit reasons in UI
- [ ] Rejected events show explicit reasons in UI
- [ ] No gameplay assumptions in UI (pure data rendering)

---

## 🧠 Phase 2 — Event Completeness & Safety

> “Every event is either applied, rejected, or deferred — never silent.”

- [ ] Ensure NarrativeEvent match is exhaustive
- [ ] Add default `_ => Deferred` handling where appropriate
- [ ] Add `AddItem` event (Deferred until inventory exists)
- [ ] Add `ModifyStat` event
- [ ] Add `SetFlag` event
- [ ] Add `StartQuest` / `UpdateQuest` events
- [ ] Improve EventApplyOutcome clarity

---

## 🧪 Phase 3 — LLM Integration (Optional, Controlled)

> “LLMs suggest. The engine decides.”

- [ ] Define official NarrativeEvent JSON schema
- [ ] Validate LLM output before decoding
- [ ] Decode LLM JSON → NarrativeEvent
- [ ] Display decoded events in debug UI
- [ ] Apply LLM events through apply_event pipeline
- [ ] Surface Deferred / Rejected reasons back to user
- [ ] No direct LLM → state mutation

---

## 🎛 Phase 4 — User-Defined State & Monitoring

> “Stats are concepts, not hardcoded numbers.”

- [ ] Convert stats to key/value model (e.g. `"souls": 120`)
- [ ] Allow arbitrary stat names
- [ ] Allow users to choose which stats to monitor
- [ ] UI supports dynamic stat lists
- [ ] Snapshot reflects only current truth
- [ ] No STR/DEX/INT assumptions

---

## 🧱 Phase 5 — Modding & Persistence  
*(Codename: Post-Hyperific Sentinel Codifying Conjunction)*

- [ ] Serialize InternalGameState to disk
- [ ] Load saved state safely
- [ ] External narrative packs (JSON / RON / YAML)
- [ ] Mod-defined NarrativeEvents
- [ ] Versioned save compatibility
- [ ] Clear error messages for broken mods

---

## 🎨 Phase 6 — Polish (After Everything Works)

- [ ] Improved snapshot UI
- [ ] Collapsible state sections
- [ ] Optional animation
- [ ] Theme presets
- [ ] Accessibility pass
- [ ] Performance cleanup

---

## 🧠 Core Design Rules (Non-Negotiable)

- The engine is authoritative
- The UI never mutates state
- The LLM is optional
- All state changes go through NarrativeEvent
- Every event produces an outcome
- Snapshots are read-only
- Nothing is hardcoded unless unavoidable

---

## 🧩 If You’re Lost

Start here:  
**Phase 1 → State Visibility & Trust**

If you can:
- See the snapshot
- See applied / deferred / rejected events

Then the framework is already a success.

