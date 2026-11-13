# Test Plan — Spark (Use Case & Security)

Written 2026-11-12.

## 1. Use Case Test Suite (`UC-*`)

| ID | Scenario | Verification Method | Expected Result |
|---|---|---|---|
| **UC-01** | CLI discovery & version probing (`agy`, `claude`, `opencode`) | `detectors::detect_all()`, `fallback_paths_from_env()`, `parse_version_output()` | All 3 agents returned in order; installed CLIs report `available = true` with non-empty path & version; missing binaries report `available = false` with error. |
| **UC-02** | Non-interactive CLI argument construction | `runner::build_args()` across all 3 `AgentKind` variants | Correct flags (`-p`/`--output-format text`, `run --auto`, `--conversation`/`--resume`/`--session`, `--model`, `--mode`, `--effort`). |
| **UC-03** | Live async stdout/stderr streaming | `runner::spawn_command()` | `StdoutLine` / `StderrLine` emitted incrementally, followed by `Finished` with exit code. |
| **UC-04** | Process cancellation (`Stop` / `Stop All`) | `RunningProcess::cancel()` | Long-running child killed via `Child::kill()`, `Finished { success: false }` emitted; second `cancel()` returns `false`. |
| **UC-05** | Session continuity & turn history persistence | `HistoryStore::save_to()` / `load_from()` / `record_turn()` / `turns_for()` | Sessions & turns round-trip cleanly to JSON; `turns_for()` filters by agent newest-first; `MAX_HISTORY_TURNS` (500) cap enforced. |
| **UC-06** | Bounded UI output buffer & theme tokens | `OutputBuffer::push_line()`, `theme::*` | Buffer caps at `MAX_OUTPUT_LINES` / `MAX_OUTPUT_BYTES` and sets `truncated = true`; Neo Kinpaku tokens distinct. |
| **UC-07** | Live GUI launch & CLI smoke check | `cargo run` / `spark.exe` startup check | Native `eframe` window launches cleanly without panic. |
| **UC-08** | Projects, multi-model sessions, & top-bar AI filtering | `uc_08_projects_multi_model_sessions_and_ai_filtering`, `v01_json_migrates_cleanly_to_projects_and_multi_sessions` | Multiple models/AIs coexist under one project; v0.1 `history.json` migrates cleanly; AI filter pills filter sessions accurately. |
| **UC-09** | Dynamic model & external CLI session discovery | `uc_09_model_and_external_session_parsers_handle_malformed_inputs`, `discover_claude_and_agy_sessions_from_temp_home`, `spawn_two_same_agent_sessions_route_independently` | Model lists and external CLI sessions parse safely; concurrent same-agent sessions route by `session_uid` without cross-talk. |

---

## 2. Security Test Suite (`SEC-*`)

| ID | Threat / Control | Verification Method | Expected Result |
|---|---|---|---|
| **SEC-01** | Windows stdin hang / stdin injection | `spawn_command_nullifies_stdin_so_child_gets_immediate_eof` | Child attempting to read stdin receives immediate EOF within 5s; never blocks or inherits parent stdin. |
| **SEC-02** | Shell metacharacter / argument injection in prompts | `build_args()` with `;`, `&&`, `|`, `$(...)`, backticks, quotes, newlines, `--flag` | Prompt stays a single literal OS argument element; never split or interpreted as extra CLI flags. |
| **SEC-03** | Control-character injection in CLI flag values | `validate_request()` with `\0`, `\r`, `\n` in `session_id`, `model`, `mode`, `effort` | Rejected with `Err(...)` before any subprocess is spawned. |
| **SEC-04** | Empty prompt & invalid working directory | `validate_request()` with whitespace prompt or non-existent `working_dir` | Rejected with `Err(...)` before any subprocess is spawned. |
| **SEC-05** | Memory exhaustion / output flood (DoS) | Flood `OutputBuffer` with 20,000 lines (>1 MB) | Buffer stays strictly within `MAX_OUTPUT_LINES` (5,000) and `MAX_OUTPUT_BYTES` (512 KiB). |
| **SEC-06** | Corrupt / malformed `history.json` resilience | `HistoryStore::load_from()` with invalid JSON & wrong schema types | Returns `Err(...)` cleanly without panicking; atomic `.json.tmp` write prevents partial corruption. |
| **SEC-07** | No hardcoded secrets & pure-Rust boundary | Scan `src/` for secret patterns; inspect `Cargo.toml` | Zero secret literals; zero Node.js/Electron/Python runtime dependencies. |
