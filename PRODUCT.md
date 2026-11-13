# Product — Spark

<!-- impeccable:product-schema 1 -->

## Platform

Native Windows Desktop Application (Pure Rust).

## Stack

- **Language:** Rust (2021 edition)
- **GUI Framework:** Pure Rust (`eframe` / `egui` for fast MVP exploration, or `gpui` / `iced` — see `PLAN.md`)
- **Async Runtime:** `tokio` (multi-process CLI streaming, non-blocking I/O)
- **Design System:** Impeccable (Neo Kinpaku — dark warm-black lacquer, kinpaku gold, verdigris patina)
- **Dev Port:** N/A (native desktop window, zero local HTTP server required)

## Users

Solo developer / operator managing multiple AI coding agents simultaneously from a single lightweight desktop command center.

## Product Purpose

Provide a fast, low-memory, pure-Rust desktop GUI to interact with and orchestrate **Antigravity / Gemini** (`agy`), **Claude Code** (`claude`), and **OpenCode** (`opencode`) CLIs from one unified interface—eliminating terminal window juggling and Electron memory bloat.

## Positioning

Spark replaces three earlier fragmented prototypes (`ai-command-deck`, `rivet-desk`, and `spark-legacy-voice`) with a single native Rust exploration. It is intentionally local-first, wrapping existing authenticated CLIs rather than re-implementing LLM API clients.

## Operating Context

- **OS:** Windows 11 (x86_64-pc-windows-msvc)
- **Local CLIs Verified on System:**
  - `agy` (Antigravity CLI `1.5.5`)
  - `claude` (Claude Code CLI `2.1.316`)
  - `opencode` (OpenCode CLI `v2.1.4`)
- **Windows Subprocess Quirks:** All child CLI processes must be spawned with `Stdio::null()` for stdin to prevent 3-second stdin-wait timeouts (`claude`) or interactive hang (`opencode`).

## Capabilities and Constraints

- **Core Capability 1 (MVP):** Detect and verify `agy`, `claude`, and `opencode` CLI binaries and versions.
- **Core Capability 2 (MVP):** Send prompts to any single agent or broadcast simultaneously to all three, streaming stdout/stderr live into the GUI.
- **Core Capability 3 (MVP):** Cancel/kill running agent subprocesses cleanly from the UI.
- **Core Capability 4 (Post-MVP):** Session continuity (`--conversation`, `--resume`, `--session`) and local history persistence.
- **Hard Constraint 1:** Pure Rust only—no Electron, no Node.js runtime, no Python backend.
- **Hard Constraint 2:** Visual design must strictly adhere to the **Impeccable** Neo Kinpaku design tokens (`DESIGN.md`).

## Brand Commitments

- **Design Language:** Impeccable ("Restraint in chrome, brilliance in texture").
- **Color Anchors:** `kinpaku-gold` for primary actions/focus, `verdigris-patina` for active/connected states, `lacquer-black` / `raised-lacquer` for surfaces.

## Evidence on Hand

- Verified non-interactive CLI invocations on Windows for `agy -p ... --output-format text`, `claude -p ... --output-format text`, and `opencode run --auto ...` with null stdin.
- Impeccable Neo Kinpaku token specification (`DESIGN.md`).
