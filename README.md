📌 RPG Chat App – GitHub Roadmap

Status legend
⬜ Not started
🟨 In progress
✅ Done

🟢 Milestone 1 — Stabilization & Cleanup

Goal: Make the codebase easy to reason about and safe to extend.

Issue #1: Editor & Structure Hygiene

⬜ Enable Rust Analyzer

⬜ Enable bracket/brace matching

⬜ Add section comments in update() (// Settings, // Input, // Messages)

⬜ Collapse long UI blocks where possible

Issue #2: Message Bubble Helper

Description: Reduce duplication in draw_message.

⬜ Create message_bubble(ui, bg_color, text)

⬜ Replace duplicated egui::Frame code

⬜ Keep text styling centralized

Acceptance Criteria

Only one place controls bubble padding, rounding, font size

Issue #3: Constrain Bubble Width

Description: Prevent messages from stretching edge-to-edge.

⬜ Limit bubble width to ~60–70% of available width

⬜ Ensure long text wraps correctly

⬜ Works for both left & right aligned messages

🟡 Milestone 2 — UX Polish

Goal: Make the app feel intentional and readable.

Issue #4: Speaker Labels

⬜ Add speaker name above message bubble

⬜ Match label color to bubble theme

⬜ Hide label for System messages

Issue #5: Optional Timestamps

⬜ Add timestamp field to Message

⬜ Display in subtle gray text

⬜ Toggleable via settings

Issue #6: Keyboard UX Improvements

⬜ Enter = send message

⬜ Shift+Enter = newline

⬜ Esc = clear input

⬜ (Optional) Ctrl+↑ edits last user message

🟠 Milestone 3 — Persistent Settings

Goal: User preferences survive restarts.

Issue #7: AppSettings Struct

⬜ Create AppSettings (ui scale, theme, speakers later)

⬜ Derive Serialize / Deserialize

⬜ Default fallback implementation

Issue #8: Save Settings to Disk

⬜ Save on theme change

⬜ Save on UI scale change

⬜ Store in config file (json or ron)

Issue #9: Load Settings on Startup

⬜ Load settings in MyApp::new()

⬜ Graceful fallback on file error

⬜ Apply theme + scale immediately

🔵 Milestone 4 — Speaker System

Goal: Support multiple characters cleanly and extensibly.

Issue #10: Expand RoleplaySpeaker Enum

⬜ Change to:

Narrator
Npc(String)
PartyMember(String)


⬜ Update engine message creation

⬜ Update UI rendering logic

Issue #11: Speaker Registry

⬜ Create Speaker { name, color }

⬜ Store in HashMap<String, Speaker>

⬜ Default speakers added on first run

Issue #12: Speaker Editor Window

⬜ List all speakers

⬜ Edit speaker color

⬜ Rename speakers

⬜ Add/remove speakers

🟣 Milestone 5 — Engine Intelligence

Goal: Make the engine feel alive and reactive.

Issue #13: Streaming Responses

⬜ Engine emits partial tokens

⬜ UI updates message incrementally

⬜ Typing indicator shown

Issue #14: System vs Roleplay Logic

⬜ Narrator never speaks as User

⬜ NPC/Party roles respected

⬜ System messages styled uniquely

Issue #15: Context Management

⬜ Trim old messages automatically

⬜ Pin lore / important messages

⬜ Reset session button

⚫ Milestone 6 — Identity & Polish

Goal: Turn this into a finished application.

Issue #16: Visual Identity

⬜ App icon

⬜ Font selection

⬜ Dark / light themes

Issue #17: Animations

⬜ Message fade-in

⬜ Slide-in for user messages

⬜ Smooth scroll to bottom

Issue #18: Session Export

⬜ Export chat to file

⬜ Markdown or plain text

⬜ Include speaker metadata
