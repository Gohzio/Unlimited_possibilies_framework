# Narrative Engine RPG

Local-first, LLM-driven desktop RPG engine built in Rust + egui.

## Build Instructions (Windows, Linux, macOS)

These steps build the desktop app from source.

### Prerequisites (all platforms)

- Rust toolchain (stable) with Cargo
- Git

Verify:

```bash
rustc --version
cargo --version
git --version
```

### Linux

1) Install system deps for egui/eframe (X11/Wayland and OpenGL):

Examples:
- Debian/Ubuntu: `libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev pkg-config`
- Fedora: `libX11-devel libxkbcommon-devel wayland-devel mesa-libGL-devel pkgconf`

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

1) Install Rust (and Xcode Command Line Tools if prompted).

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

1) Install Rust with the MSVC toolchain.
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

### Optional: Cross-compile Windows from Linux/macOS

```bash
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

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
