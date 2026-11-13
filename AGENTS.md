# Agent Guidelines — Spark

## Verification Commands
- **Build & Typecheck:** `cargo check`
- **Test Suite:** `cargo test`
- **Lint:** `cargo clippy --all-targets`
- **Run App:** `cargo run`

## Hard Boundaries
1. **Pure Rust Only:** Do not introduce Node.js, Electron, or Python runtime dependencies into the active codebase.
2. **Never commit secrets:** Never hardcode API keys, tokens, or personal credentials in tracked files.
3. **Windows Stdin Safety:** Always spawn child CLI processes (`agy`, `claude`, `opencode`) with `Stdio::null()` for stdin to avoid 3-second timeouts or interactive hangs.
4. **Impeccable Design System:** All UI colors and surface hierarchies must follow `DESIGN.md` (Neo Kinpaku palette). Do not introduce off-palette neon colors or nested cards-in-cards.
5. **Never break baseline tests:** Every change must leave `cargo test` and `cargo check` green.

## Orchestration Loop
- Follow the spec → implement → `cargo clippy` / `cargo test` → review diff → ship loop.
