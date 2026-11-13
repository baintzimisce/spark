# Spark

A pure-Rust native desktop GUI for interacting with **Antigravity (`agy`)**, **Claude Code (`claude`)**, and **OpenCode (`opencode`)** simultaneously, styled with the **Impeccable** Neo Kinpaku design system.

## Prerequisites
- **OS:** Windows 11 (`x86_64-pc-windows-msvc`) — Spark currently targets Windows desktop (`CREATE_NO_WINDOW`, WinGet/AppData CLI discovery, native Windows folder picker).
- **Rust Toolchain:** `rustc` & `cargo` (1.80+ / installed via `rustup`)
- **Agent CLIs** (authenticated and available on `PATH` or default Windows install paths):
  - `agy` (Antigravity CLI)
  - `claude` (Claude Code CLI)
  - `opencode` (OpenCode CLI)

## Key Features (v0.2)
- **Active Sessions Sidebar:** View all open/running sessions grouped by **Project**, with AI glyphs (`◆` Antigravity, `●` Claude Code, `▲` OpenCode), selected model, and live elapsed timers.
- **Top-Bar AI Session Filters:** Toggle `◆ Antigravity`, `● Claude Code`, and `▲ OpenCode` pills to filter sessions in real time.
- **Model Selector:** Dynamically discovers models from `agy models` and `opencode models`, provides Claude aliases (`sonnet`, `opus`, `haiku`, `fable`), and supports custom model IDs.
- **Project Folders & External Session Import:** Bind Projects to local folders via native Windows dialog (`rfd`) and import/resume external `agy`, `claude`, and `opencode` sessions.

## Quickstart

1. **Check & build:**
   ```powershell
   cargo check
   ```

2. **Run tests:**
   ```powershell
   cargo test
   ```

3. **Launch Spark:**
   ```powershell
   cargo run
   ```

## Architecture & Directory Layout
```text
spark/
├── Cargo.toml          # Rust dependencies & package manifest
├── PRODUCT.md          # Product context, scope, and constraints
├── PLAN.md             # Phased roadmap with "Done When" gates
├── DESIGN.md           # Impeccable (Neo Kinpaku) color & layout tokens
├── AGENTS.md           # Verification commands & agent boundaries
├── CLAUDE.md           # Points to @AGENTS.md
└── src/
    ├── main.rs         # Native desktop entrypoint (eframe / egui)
    ├── app.rs          # Active Sessions sidebar, Project grouping, Model Selector & focused workspace
    ├── history.rs      # Projects, multi-session continuity & turn history (~/.spark/history.json)
    ├── theme.rs        # Impeccable Neo Kinpaku theme tokens
    └── cli/
        ├── detectors.rs # Binary & version discovery for agy, claude, opencode
        ├── models.rs    # Dynamic model catalog discovery & presets
        ├── runner.rs    # Async subprocess streaming & cancellation keyed by session_uid
        └── sessions.rs  # External CLI session discovery (opencode, claude, agy)
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
