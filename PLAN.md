# Plan — Spark (Pure Rust Exploration)

Written 2026-11-12.

## Where It Stands

| Phase | Description | Tests / Checks | Status |
|---|---|---|---|
| 0 | Project prep, Rust baseline (`cargo`), 5-doc stack, Impeccable theme tokens | `cargo test` & `cargo check` | ✅ Done |
| 1 | CLI Discovery & Async Subprocess Streaming (`agy`, `claude`, `opencode`) | `cargo test` (CLI arg builders & detector unit tests) | ✅ Done |
| 2 | Impeccable 3-Column Matrix GUI (`eframe` / `egui`) + Live Broadcast & Stop | Manual GUI launch + parallel 3-CLI broadcast smoke test | ✅ Done |
| 3 | Dedicated Agent Tabs, Session Continuity (`--resume`/`--conversation`), & History | `cargo test` + persisted session reload | ✅ Done |

---

## How Work Flows

1. **Test-gated phases:** Every phase passes `cargo test` and `cargo clippy` before moving to the next.
2. **Design fidelity:** All UI surfaces, colors, and spacing must reference `DESIGN.md` (Impeccable Neo Kinpaku tokens).
3. **Audit boundaries:** Respect `AGENTS.md` rules and constraints.

---

## Work Queue

### Phase 1: CLI Discovery & Async Runner
- [x] Implement `src/cli/detectors.rs` to locate `agy`, `claude`, and `opencode` on Windows (`PATH` + WinGet / `.local/bin` / `npm` fallbacks) and query `--version`.
- [x] Implement `src/cli/runner.rs` using `tokio::process::Command` with `Stdio::null()` on stdin and line-by-line stdout/stderr streaming over an `mpsc` channel.
- [x] Add process termination / cancellation support (`Child::kill()`).
- **Done When:** `cargo test` passes unit tests for binary detection and command argument construction for all 3 CLIs.

### Phase 2: Impeccable 3-Column Matrix GUI
- [x] Apply Impeccable Neo Kinpaku palette (`lacquer-black`, `raised-lacquer`, `kinpaku-gold`, `verdigris-patina`) in `src/theme.rs`.
- [x] Build the 3-column Matrix Broadcast view in `eframe`/`egui` with target checkboxes, `Ctrl+Enter` transmit, live elapsed timers, and per-agent Stop buttons.
- **Done When:** `cargo run` opens the native Rust window, detects all 3 CLIs, and streams parallel responses from `agy`, `claude`, and `opencode`.

### Phase 3: Session Continuity & History
- [x] Add dedicated focused views per agent with mode/model/effort controls.
- [x] Persist turn history and session IDs locally (`~/.spark/history.json`).
- **Done When:** `cargo test` passes history serialization tests and resuming a session passes `--conversation` / `--resume` / `--session`.
