//! Integration tests covering Spark's Use Case (UC-*) and Security (SEC-*) test plan.

use spark::app::{session_matches_filters, OutputBuffer};
use spark::cli::detectors::{detect_all, fallback_paths_from_env};
use spark::cli::models::{parse_agy_models, parse_opencode_models};
use spark::cli::runner::{build_args, spawn_agent, validate_request, RunRequest};
use spark::cli::sessions::parse_opencode_sessions_json;
use spark::cli::AgentKind;
use spark::history::{HistoryStore, MAX_HISTORY_TURNS};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

#[tokio::test]
async fn uc_01_detect_all_probes_local_clis() {
    let statuses = detect_all().await;
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[0].agent, AgentKind::Agy);
    assert_eq!(statuses[1].agent, AgentKind::Claude);
    assert_eq!(statuses[2].agent, AgentKind::OpenCode);

    for status in &statuses {
        if status.available {
            assert!(
                status.binary_path.as_ref().is_some_and(|p| p.is_file()),
                "{} marked available but binary_path missing",
                status.agent
            );
            assert!(
                status.version.as_ref().is_some_and(|v| !v.is_empty()),
                "{} marked available but version missing",
                status.agent
            );
        } else {
            assert!(status.error.is_some());
        }
    }
}

#[test]
fn uc_01_fallback_paths_cover_all_agents() {
    let local = Path::new(r"C:\Users\op\AppData\Local");
    let profile = Path::new(r"C:\Users\op");
    let roaming = Path::new(r"C:\Users\op\AppData\Roaming");

    for agent in AgentKind::ALL {
        let paths = fallback_paths_from_env(agent, Some(local), Some(profile), Some(roaming));
        assert!(!paths.is_empty(), "fallback paths empty for {agent}");
    }
}

#[test]
fn uc_05_history_persistence_and_filtering() {
    let dir = std::env::temp_dir().join(format!("spark_uc05_{}", std::process::id()));
    let path = dir.join("history.json");

    let mut store = HistoryStore::default();
    store.session_mut(AgentKind::Agy).session_id = Some("conv-1".to_string());
    store.session_mut(AgentKind::Claude).effort = "high".to_string();

    store.record_turn(
        AgentKind::Agy,
        "first".to_string(),
        "out-1".to_string(),
        String::new(),
        true,
        Some(0),
        50,
        Some("conv-1".to_string()),
        None,
        Some("auto".to_string()),
        None,
    );
    store.record_turn(
        AgentKind::Agy,
        "second".to_string(),
        "out-2".to_string(),
        String::new(),
        true,
        Some(0),
        75,
        Some("conv-1".to_string()),
        None,
        Some("auto".to_string()),
        None,
    );

    store.save_to(&path).expect("save should succeed");
    let loaded = HistoryStore::load_from(&path).expect("load should succeed");

    let agy_turns = loaded.turns_for(AgentKind::Agy);
    assert_eq!(agy_turns.len(), 2);
    assert_eq!(agy_turns[0].prompt, "second");
    assert_eq!(agy_turns[1].prompt, "first");
    assert!(loaded.turns_for(AgentKind::OpenCode).is_empty());
    const { assert!(MAX_HISTORY_TURNS >= 500) };

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sec_02_prompt_shell_metacharacters_are_never_split_into_flags() {
    let malicious_prompt =
        "hello; whoami && calc.exe | dir $(id) `id` \"quoted\" 'single'\n--dangerously-skip-permissions";

    for agent in AgentKind::ALL {
        let req = RunRequest::new(agent, malicious_prompt);
        let args = build_args(&req);

        assert!(
            args.contains(&malicious_prompt.to_string()),
            "prompt was altered for {agent}"
        );
        assert!(
            !args.contains(&"--dangerously-skip-permissions".to_string()),
            "prompt injected extra flag for {agent}"
        );
    }
}

#[test]
fn sec_03_control_characters_in_flag_values_are_rejected() {
    for bad in ["id\0null", "id\rcr", "id\nlf"] {
        let mut req = RunRequest::new(AgentKind::Agy, "hello");
        req.session_id = Some(bad.to_string());
        assert!(validate_request(&req).is_err());

        let mut req = RunRequest::new(AgentKind::Claude, "hello");
        req.model = Some(bad.to_string());
        assert!(validate_request(&req).is_err());

        let mut req = RunRequest::new(AgentKind::Agy, "hello");
        req.mode = Some(bad.to_string());
        assert!(validate_request(&req).is_err());

        let mut req = RunRequest::new(AgentKind::Claude, "hello");
        req.effort = Some(bad.to_string());
        assert!(validate_request(&req).is_err());
    }
}

#[tokio::test]
async fn sec_04_empty_prompt_and_missing_working_dir_blocked_before_spawn() {
    let (tx, _rx) = mpsc::unbounded_channel();

    let empty_req = RunRequest::new(AgentKind::Agy, "   \t\n ");
    assert!(spawn_agent(Path::new("agy"), "sess-empty", &empty_req, tx.clone()).is_err());

    let mut bad_dir_req = RunRequest::new(AgentKind::Claude, "hello");
    bad_dir_req.working_dir =
        Some(Path::new(r"Z:\definitely_nonexistent_spark_dir_999").to_path_buf());
    assert!(spawn_agent(Path::new("claude"), "sess-baddir", &bad_dir_req, tx).is_err());
}

#[test]
fn sec_05_output_buffer_resists_memory_flood() {
    let mut buf = OutputBuffer::default();
    let payload = "X".repeat(256);
    for _ in 0..20_000 {
        buf.push_line(payload.clone());
    }
    assert!(buf.truncated);
    assert!(buf.lines.len() <= 5_000);
    assert!(buf.total_bytes <= 512 * 1024);
}

#[test]
fn sec_06_corrupt_and_type_mismatched_history_json_handled_safely() {
    let dir = std::env::temp_dir().join(format!("spark_sec06_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bad_json = dir.join("bad.json");
    std::fs::write(&bad_json, "{\"turns\": \"not-an-array\"}").unwrap();
    assert!(HistoryStore::load_from(&bad_json).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sec_07_no_hardcoded_secrets_in_source_tree() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = ["AKIA", "BEGIN PRIVATE KEY", "ghp_", "AIzaSy"];

    fn visit(dir: &Path, forbidden: &[&str]) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, forbidden);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                for marker in forbidden {
                    assert!(
                        !content.contains(marker),
                        "forbidden secret marker {marker} found in {}",
                        path.display()
                    );
                }
            }
        }
    }

    visit(&src_dir, &forbidden);
}

#[test]
fn uc_08_projects_multi_model_sessions_and_ai_filtering() {
    let mut store = HistoryStore::default();
    let proj_a = store
        .create_project("Spark", Some(PathBuf::from(r"D:\Projects\spark")))
        .id
        .clone();
    let proj_b = store
        .create_project("Docs", Some(PathBuf::from(r"D:\Projects\docs")))
        .id
        .clone();

    let s1 = store
        .create_session(&proj_a, AgentKind::Claude, "sonnet", "Claude Sonnet")
        .uid
        .clone();
    let s2 = store
        .create_session(&proj_a, AgentKind::Claude, "opus", "Claude Opus")
        .uid
        .clone();
    let s3 = store
        .create_session(&proj_a, AgentKind::Agy, "gemini-4-hydrogen-high", "Agy H4")
        .uid
        .clone();
    let s4 = store
        .create_session(
            &proj_b,
            AgentKind::OpenCode,
            "opencode/big-pickle",
            "OC Pickle",
        )
        .uid
        .clone();

    assert_ne!(s1, s2);
    assert_eq!(store.projects.len(), 2);
    assert_eq!(store.active_sessions.len(), 4);

    let mut ai_filters = HashMap::new();
    ai_filters.insert(AgentKind::Agy, true);
    ai_filters.insert(AgentKind::Claude, false);
    ai_filters.insert(AgentKind::OpenCode, true);

    let visible_uids: Vec<&str> = store
        .active_sessions
        .iter()
        .filter(|s| session_matches_filters(s, &ai_filters))
        .map(|s| s.uid.as_str())
        .collect();
    assert_eq!(visible_uids, vec![s3.as_str(), s4.as_str()]);
}

#[test]
fn uc_09_model_and_external_session_parsers_handle_malformed_inputs() {
    assert!(parse_agy_models("").is_empty());
    assert!(parse_opencode_models("INFO startup\nnot-a-model\n").is_empty());
    assert!(parse_opencode_sessions_json("{not-an-array}").is_empty());

    let agy_models = parse_agy_models("gemini-4-hydrogen-high\tGemini 4 Hydrogen (High)\n");
    assert_eq!(agy_models.len(), 1);
    assert_eq!(agy_models[0].id, "gemini-4-hydrogen-high");
}
