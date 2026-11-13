# Spark

A pure-Rust native desktop GUI for interacting with **Antigravity (`agy`)**, **Claude Code (`claude`)**, and **OpenCode (`opencode`)** simultaneously, styled with the **Impeccable** Neo Kinpaku design system.

## Prerequisites
- **Rust Toolchain:** `rustc` & `cargo` (1.80+ / installed via `rustup`)
- **Agent CLIs** (authenticated and available on `PATH` or default Windows install paths):
  - `agy` (Antigravity CLI)
  - `claude` (Claude Code CLI)
  - `opencode` (OpenCode CLI)

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
    └── theme.rs        # Impeccable Neo Kinpaku theme tokens
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
