//! Spark v0.2.0 GUI application state and Impeccable Neo Kinpaku views.

use crate::cli::detectors::{self, CliStatus};
use crate::cli::models::{self, ModelOption};
use crate::cli::runner::{self, RunRequest, RunningProcess, StreamEvent};
use crate::cli::sessions::{self, DiscoveredSession};
use crate::cli::AgentKind;
use crate::history::{HistoryStore, ProjectRecord, SessionRecord};
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::Instant;
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;

const MAX_OUTPUT_LINES: usize = 5_000;
const MAX_OUTPUT_BYTES: usize = 512 * 1024;

/// Execution lifecycle state for an active session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RunState {
    #[default]
    Idle,
    Running {
        started_at: Instant,
        prompt: String,
    },
    Done {
        duration_ms: u64,
    },
    Failed {
        duration_ms: u64,
        exit_code: Option<i32>,
    },
}

/// Background messages delivered to the UI thread.
enum UiMessage {
    DetectionComplete {
        statuses: Vec<CliStatus>,
        models: HashMap<AgentKind, Vec<ModelOption>>,
    },
    ExternalSessionsDiscovered(Vec<DiscoveredSession>),
    Stream(StreamEvent),
}

/// Bounded line buffer for live CLI stdout/stderr.
#[derive(Debug, Default)]
pub struct OutputBuffer {
    pub lines: Vec<String>,
    pub total_bytes: usize,
    pub truncated: bool,
}

impl OutputBuffer {
    pub fn clear(&mut self) {
        self.lines.clear();
        self.total_bytes = 0;
        self.truncated = false;
    }

    pub fn push_line(&mut self, line: String) {
        self.total_bytes = self.total_bytes.saturating_add(line.len() + 1);
        self.lines.push(line);

        while self.lines.len() > MAX_OUTPUT_LINES || self.total_bytes > MAX_OUTPUT_BYTES {
            if let Some(removed) = self.lines.first().cloned() {
                self.lines.remove(0);
                self.total_bytes = self.total_bytes.saturating_sub(removed.len() + 1);
                self.truncated = true;
            } else {
                break;
            }
        }
    }

    pub fn joined(&self) -> String {
        self.lines.join("\n")
    }
}

/// Ephemeral runtime state for a Spark session (process handle + output buffers).
#[derive(Default)]
pub struct SessionRuntimeState {
    pub run_state: RunState,
    pub process: Option<RunningProcess>,
    pub stdout: OutputBuffer,
    pub stderr: OutputBuffer,
}

impl SessionRuntimeState {
    pub fn is_running(&self) -> bool {
        matches!(self.run_state, RunState::Running { .. })
    }

    pub fn cancel(&mut self) {
        if let Some(proc) = self.process.as_mut() {
            proc.cancel();
        }
    }
}

struct NewProjectDraft {
    name: String,
    working_dir: String,
}

struct NewSessionDraft {
    project_id: String,
    agent: AgentKind,
    model: String,
    custom_model_mode: bool,
    title: String,
}

/// Return `true` when `session` is visible under the active top-bar AI filters.
pub fn session_matches_filters(
    session: &SessionRecord,
    ai_filters: &HashMap<AgentKind, bool>,
) -> bool {
    !session.archived && ai_filters.get(&session.agent).copied().unwrap_or(true)
}

/// Main Spark desktop application.
pub struct SparkApp {
    runtime: Runtime,
    ui_tx: std_mpsc::Sender<UiMessage>,
    ui_rx: std_mpsc::Receiver<UiMessage>,
    cli_statuses: HashMap<AgentKind, CliStatus>,
    model_catalog: HashMap<AgentKind, Vec<ModelOption>>,
    runtime_sessions: HashMap<String, SessionRuntimeState>,
    ai_filters: HashMap<AgentKind, bool>,
    custom_model_input_open: HashMap<String, bool>,
    prompt: String,
    history: HistoryStore,
    history_path: Option<PathBuf>,
    last_error: Option<String>,
    detecting: bool,
    new_project_draft: Option<NewProjectDraft>,
    new_session_draft: Option<NewSessionDraft>,
    import_window_open: bool,
    discovering_external: bool,
    discovered_external: Vec<DiscoveredSession>,
    import_target_project_id: Option<String>,
}

impl SparkApp {
    pub fn new(ctx: egui::Context) -> Self {
        let runtime = Runtime::new().expect("Tokio runtime should initialize");
        let (ui_tx, ui_rx) = std_mpsc::channel();

        let history_path = HistoryStore::default_path();
        let mut history = history_path
            .as_deref()
            .and_then(|p| HistoryStore::load_from(p).ok())
            .unwrap_or_default();

        let default_dir = std::env::current_dir().ok();
        history.migrate_v01_if_needed(default_dir);

        let mut model_catalog = HashMap::new();
        let mut ai_filters = HashMap::new();
        for agent in AgentKind::ALL {
            model_catalog.insert(agent, models::default_models_for(agent));
            ai_filters.insert(agent, true);
        }

        let mut app = Self {
            runtime,
            ui_tx,
            ui_rx,
            cli_statuses: HashMap::new(),
            model_catalog,
            runtime_sessions: HashMap::new(),
            ai_filters,
            custom_model_input_open: HashMap::new(),
            prompt: String::new(),
            history,
            history_path,
            last_error: None,
            detecting: false,
            new_project_draft: None,
            new_session_draft: None,
            import_window_open: false,
            discovering_external: false,
            discovered_external: Vec::new(),
            import_target_project_id: None,
        };

        app.persist_history();
        app.trigger_detection(ctx);
        app
    }

    fn trigger_detection(&mut self, ctx: egui::Context) {
        self.detecting = true;
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            let statuses = detectors::detect_all().await;
            let models = models::fetch_all_models(&statuses).await;
            let _ = tx.send(UiMessage::DetectionComplete { statuses, models });
            ctx.request_repaint();
        });
    }

    fn trigger_external_discovery(&mut self, ctx: egui::Context) {
        self.discovering_external = true;
        let tx = self.ui_tx.clone();
        let statuses: Vec<CliStatus> = self.cli_statuses.values().cloned().collect();
        let working_dirs: Vec<PathBuf> = self
            .history
            .projects
            .iter()
            .filter_map(|p| p.working_dir.clone())
            .collect();

        self.runtime.spawn(async move {
            let found = sessions::discover_all_sessions(&statuses, &working_dirs).await;
            let _ = tx.send(UiMessage::ExternalSessionsDiscovered(found));
            ctx.request_repaint();
        });
    }

    fn persist_history(&mut self) {
        if let Some(path) = self.history_path.as_deref() {
            if let Err(err) = self.history.save_to(path) {
                self.last_error = Some(err);
            }
        }
    }

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::DetectionComplete { statuses, models } => {
                    self.detecting = false;
                    for status in statuses {
                        self.cli_statuses.insert(status.agent, status);
                    }
                    self.model_catalog = models;
                }
                UiMessage::ExternalSessionsDiscovered(found) => {
                    self.discovering_external = false;
                    self.discovered_external = found;
                }
                UiMessage::Stream(ev) => self.handle_stream_event(ev),
            }
        }
    }

    fn handle_stream_event(&mut self, ev: StreamEvent) {
        match ev {
            StreamEvent::StdoutLine {
                session_uid, line, ..
            } => {
                self.runtime_sessions
                    .entry(session_uid)
                    .or_default()
                    .stdout
                    .push_line(line);
            }
            StreamEvent::StderrLine {
                session_uid, line, ..
            } => {
                self.runtime_sessions
                    .entry(session_uid)
                    .or_default()
                    .stderr
                    .push_line(line);
            }
            StreamEvent::Finished {
                session_uid,
                agent,
                exit_code,
                success,
            } => {
                let completed = {
                    let rt = self
                        .runtime_sessions
                        .entry(session_uid.clone())
                        .or_default();
                    rt.process = None;
                    let (duration_ms, prompt) = match &rt.run_state {
                        RunState::Running { started_at, prompt } => {
                            (started_at.elapsed().as_millis() as u64, prompt.clone())
                        }
                        _ => (0, String::new()),
                    };

                    rt.run_state = if success {
                        RunState::Done { duration_ms }
                    } else {
                        RunState::Failed {
                            duration_ms,
                            exit_code,
                        }
                    };

                    (prompt, rt.stdout.joined(), rt.stderr.joined(), duration_ms)
                };

                let (prompt, output, stderr, duration_ms) = completed;
                if !prompt.is_empty() {
                    let meta = self.history.session_by_uid(&session_uid).map(|s| {
                        (
                            s.project_id.clone(),
                            s.cli_session_id.clone(),
                            non_empty(&s.model),
                            non_empty(&s.mode),
                            non_empty(&s.effort),
                        )
                    });
                    if let Some((project_id, cli_session_id, model, mode, effort)) = meta {
                        self.history.record_session_turn(
                            Some(project_id),
                            Some(session_uid),
                            agent,
                            prompt,
                            output,
                            stderr,
                            success,
                            exit_code,
                            duration_ms,
                            cli_session_id,
                            model,
                            mode,
                            effort,
                        );
                        self.persist_history();
                    }
                }
            }
            StreamEvent::Error {
                session_uid,
                message,
                ..
            } => {
                let rt = self.runtime_sessions.entry(session_uid).or_default();
                rt.process = None;
                rt.stderr.push_line(message);
                let duration_ms = match &rt.run_state {
                    RunState::Running { started_at, .. } => started_at.elapsed().as_millis() as u64,
                    _ => 0,
                };
                rt.run_state = RunState::Failed {
                    duration_ms,
                    exit_code: None,
                };
            }
        }
    }

    fn dispatch_to_session(&mut self, session_uid: &str, ctx: &egui::Context) {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        if self
            .runtime_sessions
            .get(session_uid)
            .is_some_and(SessionRuntimeState::is_running)
        {
            return;
        }

        let Some(sess) = self.history.session_by_uid(session_uid).cloned() else {
            return;
        };
        let Some(binary_path) = self
            .cli_statuses
            .get(&sess.agent)
            .filter(|s| s.available)
            .and_then(|s| s.binary_path.clone())
        else {
            self.last_error = Some(format!(
                "{} CLI is not available on this machine",
                sess.agent.display_name()
            ));
            return;
        };

        let working_dir = self
            .history
            .project(&sess.project_id)
            .and_then(|p| p.working_dir.clone());

        self.last_error = None;

        // Auto-name default sessions from their first prompt for quick identification.
        if let Some(sess_mut) = self.history.session_by_uid_mut(session_uid) {
            if sess_mut.title == format!("{} Session", sess_mut.agent.display_name()) {
                sess_mut.title = compact_session_title(&prompt);
            }
        }

        let mut req = RunRequest::new(sess.agent, prompt.clone());
        req.working_dir = working_dir;
        req.session_id = sess.cli_session_id.filter(|s| !s.trim().is_empty());
        req.model = non_empty(&sess.model);
        req.mode = non_empty(&sess.mode);
        req.effort = non_empty(&sess.effort);

        let (tokio_tx, mut tokio_rx) = tokio_mpsc::unbounded_channel();
        let _guard = self.runtime.enter();
        match runner::spawn_agent(&binary_path, session_uid, &req, tokio_tx) {
            Ok(proc_handle) => {
                let rt = self
                    .runtime_sessions
                    .entry(session_uid.to_string())
                    .or_default();
                rt.stdout.clear();
                rt.stderr.clear();
                rt.process = Some(proc_handle);
                rt.run_state = RunState::Running {
                    started_at: Instant::now(),
                    prompt,
                };

                let ui_tx = self.ui_tx.clone();
                let repaint_ctx = ctx.clone();
                self.runtime.spawn(async move {
                    while let Some(ev) = tokio_rx.recv().await {
                        let _ = ui_tx.send(UiMessage::Stream(ev));
                        repaint_ctx.request_repaint();
                    }
                });
                self.persist_history();
            }
            Err(err) => {
                let rt = self
                    .runtime_sessions
                    .entry(session_uid.to_string())
                    .or_default();
                rt.stderr.push_line(err.clone());
                rt.run_state = RunState::Failed {
                    duration_ms: 0,
                    exit_code: None,
                };
                self.last_error = Some(err);
            }
        }
    }

    fn running_count(&self) -> usize {
        self.runtime_sessions
            .values()
            .filter(|rt| rt.is_running())
            .count()
    }

    fn any_running(&self) -> bool {
        self.running_count() > 0
    }

    fn stop_all(&mut self) {
        for rt in self.runtime_sessions.values_mut() {
            rt.cancel();
        }
    }

    fn all_ai_filters_enabled(&self) -> bool {
        AgentKind::ALL
            .iter()
            .all(|a| self.ai_filters.get(a).copied().unwrap_or(true))
    }

    fn reset_ai_filters(&mut self) {
        for agent in AgentKind::ALL {
            self.ai_filters.insert(agent, true);
        }
    }

    fn open_new_session_draft(&mut self, project_id: String) {
        let agent = AgentKind::ALL
            .into_iter()
            .find(|a| self.cli_statuses.get(a).is_some_and(|s| s.available))
            .unwrap_or(AgentKind::Claude);
        let default_model = self
            .model_catalog
            .get(&agent)
            .and_then(|list| list.first())
            .map(|m| m.id.clone())
            .unwrap_or_default();

        self.new_session_draft = Some(NewSessionDraft {
            project_id,
            agent,
            model: default_model,
            custom_model_mode: false,
            title: String::new(),
        });
    }

    fn render_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("spark_header")
            .frame(
                Frame::new()
                    .fill(theme::RAISED_LACQUER)
                    .inner_margin(Margin::symmetric(16, 10))
                    .stroke(Stroke::new(1.0_f32, theme::HAIRLINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("SPARK")
                            .strong()
                            .size(17.0)
                            .color(theme::KINPAKU_GOLD),
                    );
                    ui.label(RichText::new("0.2").size(11.0).color(theme::TEXT_FAINT));
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Filter Sessions:")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );

                    for agent in AgentKind::ALL {
                        let active = self.ai_filters.get(&agent).copied().unwrap_or(true);
                        let count = self
                            .history
                            .active_sessions
                            .iter()
                            .filter(|s| !s.archived && s.agent == agent)
                            .count();
                        let label = format!("{} {} ({count})", agent.glyph(), agent.display_name());
                        if filter_pill(ui, active, &label).clicked() {
                            self.ai_filters.insert(agent, !active);
                        }
                    }

                    if !self.all_ai_filters_enabled()
                        && ui
                            .button(
                                RichText::new("Show All")
                                    .size(12.0)
                                    .color(theme::KINPAKU_GOLD),
                            )
                            .clicked()
                    {
                        self.reset_ai_filters();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let rescan_label = if self.detecting {
                            "Scanning..."
                        } else {
                            "Re-scan"
                        };
                        if ui
                            .add_enabled(
                                !self.detecting,
                                egui::Button::new(RichText::new(rescan_label).size(12.0)),
                            )
                            .clicked()
                        {
                            self.trigger_detection(ctx.clone());
                        }

                        if ui.button(RichText::new("Import CLI").size(12.0)).clicked() {
                            self.import_window_open = true;
                            if self.import_target_project_id.is_none() {
                                self.import_target_project_id =
                                    self.history.projects.first().map(|p| p.id.clone());
                            }
                            self.trigger_external_discovery(ctx.clone());
                        }

                        let new_btn = egui::Button::new(
                            RichText::new("+ New Session")
                                .color(theme::DARK_INK)
                                .strong()
                                .size(12.0),
                        )
                        .fill(theme::KINPAKU_GOLD);
                        if ui.add(new_btn).clicked() {
                            let target_project = self
                                .history
                                .selected_session_uid
                                .as_deref()
                                .and_then(|uid| self.history.session_by_uid(uid))
                                .map(|s| s.project_id.clone())
                                .or_else(|| self.history.projects.first().map(|p| p.id.clone()))
                                .unwrap_or_else(|| self.history.ensure_default_project(None));
                            self.open_new_session_draft(target_project);
                        }
                    });
                });
            });
    }

    fn render_sessions_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("spark_sessions_sidebar")
            .resizable(true)
            .default_width(290.0)
            .min_width(230.0)
            .max_width(440.0)
            .frame(
                Frame::new()
                    .fill(theme::RAISED_LACQUER)
                    .inner_margin(Margin::same(12))
                    .stroke(Stroke::new(1.0_f32, theme::HAIRLINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("ACTIVE SESSIONS")
                            .strong()
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("+ Project").size(11.5)).clicked() {
                            self.new_project_draft = Some(NewProjectDraft {
                                name: String::new(),
                                working_dir: String::new(),
                            });
                        }
                    });
                });

                self.render_new_project_inline(ui);
                ui.separator();

                let projects_snapshot: Vec<ProjectRecord> = self.history.projects.clone();
                let sessions_snapshot: Vec<SessionRecord> = self.history.active_sessions.clone();
                let selected_uid = self.history.selected_session_uid.clone();

                let mut toggle_collapse_id: Option<String> = None;
                let mut pick_folder_project_id: Option<String> = None;
                let mut add_session_project_id: Option<String> = None;
                let mut select_uid: Option<String> = None;
                let mut archive_uid: Option<String> = None;
                let mut stop_uid: Option<String> = None;
                let mut hidden_count: usize = 0;

                egui::ScrollArea::vertical()
                    .id_salt("active_sessions_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for project in &projects_snapshot {
                            let proj_sessions: Vec<&SessionRecord> = sessions_snapshot
                                .iter()
                                .filter(|s| s.project_id == project.id && !s.archived)
                                .collect();
                            let visible_sessions: Vec<&SessionRecord> = proj_sessions
                                .iter()
                                .copied()
                                .filter(|s| session_matches_filters(s, &self.ai_filters))
                                .collect();
                            hidden_count +=
                                proj_sessions.len().saturating_sub(visible_sessions.len());

                            ui.horizontal(|ui| {
                                let arrow = if project.collapsed { "▸" } else { "▾" };
                                if ui
                                    .button(
                                        RichText::new(format!("{arrow} {}", project.name))
                                            .strong()
                                            .size(13.0)
                                            .color(theme::CHAMPAGNE),
                                    )
                                    .clicked()
                                {
                                    toggle_collapse_id = Some(project.id.clone());
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("+")
                                            .on_hover_text("New session in this project")
                                            .clicked()
                                        {
                                            add_session_project_id = Some(project.id.clone());
                                        }
                                        if ui
                                            .small_button("Folder")
                                            .on_hover_text("Choose working directory")
                                            .clicked()
                                        {
                                            pick_folder_project_id = Some(project.id.clone());
                                        }
                                    },
                                );
                            });

                            if let Some(dir) = &project.working_dir {
                                ui.label(
                                    RichText::new(dir.display().to_string())
                                        .size(10.5)
                                        .color(theme::TEXT_FAINT),
                                );
                            }

                            if !project.collapsed {
                                if visible_sessions.is_empty() {
                                    let msg = if proj_sessions.is_empty() {
                                        "No active sessions in this project."
                                    } else {
                                        "Sessions hidden by top AI filter."
                                    };
                                    ui.label(
                                        RichText::new(msg).size(11.0).color(theme::TEXT_FAINT),
                                    );
                                } else {
                                    for sess in visible_sessions {
                                        let is_selected =
                                            selected_uid.as_deref() == Some(sess.uid.as_str());
                                        let rt = self.runtime_sessions.get(&sess.uid);
                                        let is_running =
                                            rt.is_some_and(SessionRuntimeState::is_running);

                                        let row_fill = if is_selected {
                                            theme::GRAPHITE_2
                                        } else {
                                            theme::GRAPHITE
                                        };
                                        let row_stroke = if is_selected {
                                            Stroke::new(1.0_f32, theme::KINPAKU_GOLD)
                                        } else {
                                            Stroke::new(1.0_f32, theme::HAIRLINE)
                                        };

                                        Frame::new()
                                            .fill(row_fill)
                                            .stroke(row_stroke)
                                            .corner_radius(CornerRadius::same(6))
                                            .inner_margin(Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    let glyph_color = session_status_color(
                                                        rt.map(|r| &r.run_state),
                                                        is_selected,
                                                    );
                                                    ui.label(
                                                        RichText::new(sess.agent.glyph())
                                                            .strong()
                                                            .size(14.0)
                                                            .color(glyph_color),
                                                    );

                                                    let title_resp = ui.selectable_label(
                                                        is_selected,
                                                        RichText::new(&sess.title)
                                                            .size(12.5)
                                                            .color(theme::CHAMPAGNE),
                                                    );
                                                    if title_resp.clicked() {
                                                        select_uid = Some(sess.uid.clone());
                                                    }

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if is_running {
                                                                if ui
                                                                    .small_button(
                                                                        RichText::new("Stop")
                                                                            .color(
                                                                                theme::ERROR_CORAL,
                                                                            ),
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    stop_uid =
                                                                        Some(sess.uid.clone());
                                                                }
                                                            } else if ui
                                                                .small_button("×")
                                                                .on_hover_text(
                                                                    "Close / archive session",
                                                                )
                                                                .clicked()
                                                            {
                                                                archive_uid =
                                                                    Some(sess.uid.clone());
                                                            }
                                                        },
                                                    );
                                                });

                                                ui.horizontal(|ui| {
                                                    let model_label = if sess.model.is_empty() {
                                                        "default model".to_string()
                                                    } else {
                                                        sess.model.clone()
                                                    };
                                                    ui.label(
                                                        RichText::new(model_label)
                                                            .size(11.0)
                                                            .color(theme::TEXT_MUTED),
                                                    );
                                                    if let Some(rt) = rt {
                                                        if let Some(elapsed) =
                                                            format_elapsed(&rt.run_state)
                                                        {
                                                            let badge_color = match &rt.run_state {
                                                                RunState::Running { .. }
                                                                | RunState::Done { .. } => {
                                                                    theme::VERDIGRIS_PATINA
                                                                }
                                                                RunState::Failed { .. } => {
                                                                    theme::ERROR_CORAL
                                                                }
                                                                RunState::Idle => theme::TEXT_FAINT,
                                                            };
                                                            ui.with_layout(
                                                                egui::Layout::right_to_left(
                                                                    egui::Align::Center,
                                                                ),
                                                                |ui| {
                                                                    ui.label(
                                                                        RichText::new(elapsed)
                                                                            .monospace()
                                                                            .size(10.5)
                                                                            .color(badge_color),
                                                                    );
                                                                },
                                                            );
                                                        }
                                                    }
                                                });
                                            });
                                        ui.add_space(4.0);
                                    }
                                }
                            }

                            ui.add_space(6.0);
                            ui.separator();
                        }

                        if hidden_count > 0 {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{hidden_count} hidden by AI filter"))
                                        .size(11.0)
                                        .color(theme::TEXT_FAINT),
                                );
                                if ui.small_button("Reset").clicked() {
                                    self.reset_ai_filters();
                                }
                            });
                        }
                    });

                let mut changed = false;
                if let Some(pid) = toggle_collapse_id {
                    if let Some(proj) = self.history.project_mut(&pid) {
                        proj.collapsed = !proj.collapsed;
                        changed = true;
                    }
                }
                if let Some(pid) = pick_folder_project_id {
                    let initial = self
                        .history
                        .project(&pid)
                        .and_then(|p| p.working_dir.clone());
                    if let Some(folder) = pick_folder_dialog(initial.as_deref()) {
                        if let Some(proj) = self.history.project_mut(&pid) {
                            proj.working_dir = Some(folder);
                            changed = true;
                        }
                    }
                }
                if let Some(pid) = add_session_project_id {
                    self.open_new_session_draft(pid);
                }
                if let Some(uid) = select_uid {
                    self.history.selected_session_uid = Some(uid);
                    changed = true;
                }
                if let Some(uid) = stop_uid {
                    if let Some(rt) = self.runtime_sessions.get_mut(&uid) {
                        rt.cancel();
                    }
                }
                if let Some(uid) = archive_uid {
                    self.history.archive_session(&uid);
                    changed = true;
                }
                if changed {
                    self.persist_history();
                }
            });
    }

    fn render_new_project_inline(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.new_project_draft.take() else {
            return;
        };

        let mut keep_open = true;
        let mut create = false;
        ui.add_space(6.0);
        ui.label(
            RichText::new("New Project")
                .strong()
                .size(12.0)
                .color(theme::KINPAKU_GOLD),
        );
        ui.add(
            egui::TextEdit::singleline(&mut draft.name)
                .hint_text("Project name")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.working_dir)
                    .hint_text("Working directory (optional)")
                    .desired_width((ui.available_width() - 78.0).max(90.0)),
            );
            if ui.button("Browse...").clicked() {
                if let Some(folder) = pick_folder_dialog(None) {
                    if draft.name.trim().is_empty() {
                        if let Some(name) = folder.file_name().and_then(|n| n.to_str()) {
                            draft.name = name.to_string();
                        }
                    }
                    draft.working_dir = folder.display().to_string();
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new("Create").color(theme::KINPAKU_GOLD))
                .clicked()
            {
                create = true;
                keep_open = false;
            }
            if ui.button("Cancel").clicked() {
                keep_open = false;
            }
        });

        if create {
            let dir = non_empty(&draft.working_dir).map(PathBuf::from);
            self.history.create_project(draft.name, dir);
            self.persist_history();
        } else if keep_open {
            self.new_project_draft = Some(draft);
        }
    }

    fn render_new_session_window(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.new_session_draft.take() else {
            return;
        };

        let mut open = true;
        let mut create = false;
        let mut cancel = false;

        egui::Window::new("New Session")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(RichText::new("Project").size(12.0).color(theme::TEXT_MUTED));
                let selected_proj_name = self
                    .history
                    .project(&draft.project_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "Select project".to_string());
                egui::ComboBox::from_id_salt("new_sess_project")
                    .selected_text(selected_proj_name)
                    .width(360.0)
                    .show_ui(ui, |ui| {
                        for proj in &self.history.projects {
                            ui.selectable_value(&mut draft.project_id, proj.id.clone(), &proj.name);
                        }
                    });

                ui.add_space(6.0);
                ui.label(
                    RichText::new("AI Provider")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.horizontal(|ui| {
                    for agent in AgentKind::ALL {
                        let label = format!("{} {}", agent.glyph(), agent.display_name());
                        if ui.selectable_label(draft.agent == agent, label).clicked() {
                            draft.agent = agent;
                            draft.custom_model_mode = false;
                            draft.model = self
                                .model_catalog
                                .get(&agent)
                                .and_then(|m| m.first())
                                .map(|m| m.id.clone())
                                .unwrap_or_default();
                        }
                    }
                });

                ui.add_space(6.0);
                ui.label(RichText::new("Model").size(12.0).color(theme::TEXT_MUTED));
                let options = self
                    .model_catalog
                    .get(&draft.agent)
                    .cloned()
                    .unwrap_or_else(|| models::default_models_for(draft.agent));
                render_model_selector_controls(
                    ui,
                    "new_sess_model",
                    &options,
                    &mut draft.model,
                    &mut draft.custom_model_mode,
                );

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Session Title (optional)")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.title)
                        .hint_text("Auto-named from first prompt if left blank")
                        .desired_width(360.0),
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let btn = egui::Button::new(
                        RichText::new("Create Session")
                            .color(theme::DARK_INK)
                            .strong(),
                    )
                    .fill(theme::KINPAKU_GOLD);
                    if ui.add(btn).clicked() {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if create {
            self.history
                .create_session(draft.project_id, draft.agent, draft.model, draft.title);
            self.persist_history();
        } else if open && !cancel {
            self.new_session_draft = Some(draft);
        }
    }

    fn render_import_window(&mut self, ctx: &egui::Context) {
        if !self.import_window_open {
            return;
        }

        let mut open = true;
        let mut import_selected: Option<DiscoveredSession> = None;
        let mut refresh = false;

        egui::Window::new("Import / Resume External CLI Session")
            .open(&mut open)
            .collapsible(false)
            .default_size([560.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Target Project:")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                    let current_name = self
                        .import_target_project_id
                        .as_deref()
                        .and_then(|id| self.history.project(id))
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Auto (match folder)".to_string());

                    egui::ComboBox::from_id_salt("import_target_proj")
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.import_target_project_id,
                                None,
                                "Auto (match folder)",
                            );
                            for proj in &self.history.projects {
                                ui.selectable_value(
                                    &mut self.import_target_project_id,
                                    Some(proj.id.clone()),
                                    &proj.name,
                                );
                            }
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if self.discovering_external {
                            "Scanning..."
                        } else {
                            "Refresh"
                        };
                        if ui
                            .add_enabled(!self.discovering_external, egui::Button::new(label))
                            .clicked()
                        {
                            refresh = true;
                        }
                    });
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("import_external_list")
                    .max_height(330.0)
                    .show(ui, |ui| {
                        if self.discovering_external && self.discovered_external.is_empty() {
                            ui.label(
                                RichText::new("Scanning local CLI sessions...")
                                    .color(theme::TEXT_MUTED),
                            );
                            return;
                        }
                        if self.discovered_external.is_empty() {
                            ui.label(
                                RichText::new(
                                    "No external sessions found in OpenCode, Claude Code, or Antigravity.",
                                )
                                .color(theme::TEXT_FAINT),
                            );
                            return;
                        }

                        for item in &self.discovered_external {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(item.agent.glyph())
                                        .strong()
                                        .size(14.0)
                                        .color(theme::KINPAKU_GOLD),
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&item.title)
                                            .strong()
                                            .size(12.5)
                                            .color(theme::CHAMPAGNE),
                                    );
                                    let detail = match &item.working_dir {
                                        Some(dir) => {
                                            format!("{} • {}", item.cli_session_id, dir.display())
                                        }
                                        None => item.cli_session_id.clone(),
                                    };
                                    ui.label(
                                        RichText::new(detail)
                                            .size(10.5)
                                            .color(theme::TEXT_FAINT),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Import").clicked() {
                                            import_selected = Some(item.clone());
                                        }
                                    },
                                );
                            });
                            ui.separator();
                        }
                    });
            });

        if !open {
            self.import_window_open = false;
        }
        if refresh {
            self.trigger_external_discovery(ctx.clone());
        }
        if let Some(item) = import_selected {
            let project_id = match &self.import_target_project_id {
                Some(pid) => pid.clone(),
                None => match &item.working_dir {
                    Some(dir) if dir.is_dir() => self.history.find_or_create_project_for_dir(dir),
                    _ => self.history.ensure_default_project(None),
                },
            };
            self.history.import_external_session(
                project_id,
                item.agent,
                item.cli_session_id,
                item.title,
                String::new(),
            );
            self.persist_history();
            self.import_window_open = false;
        }
    }

    fn render_prompt_bar(&mut self, ctx: &egui::Context, focused_uid: Option<&str>) {
        egui::TopBottomPanel::bottom("spark_prompt_bar")
            .frame(
                Frame::new()
                    .fill(theme::RAISED_LACQUER)
                    .inner_margin(Margin::symmetric(16, 12))
                    .stroke(Stroke::new(1.0_f32, theme::HAIRLINE)),
            )
            .show(ctx, |ui| {
                if let Some(err) = &self.last_error {
                    ui.label(RichText::new(err).color(theme::ERROR_CORAL).size(12.0));
                    ui.add_space(4.0);
                }

                let ctrl_enter = ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));

                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::multiline(&mut self.prompt)
                        .hint_text("Prompt focused session... (Ctrl+Enter to transmit)")
                        .desired_rows(2)
                        .desired_width((ui.available_width() - 190.0).max(200.0));
                    ui.add(edit);

                    ui.vertical(|ui| {
                        let focused_ready = focused_uid.is_some_and(|uid| {
                            let is_running = self
                                .runtime_sessions
                                .get(uid)
                                .is_some_and(SessionRuntimeState::is_running);
                            let cli_ok = self
                                .history
                                .session_by_uid(uid)
                                .and_then(|s| self.cli_statuses.get(&s.agent))
                                .is_some_and(|st| st.available);
                            !is_running && cli_ok
                        });

                        let can_send = !self.prompt.trim().is_empty() && focused_ready;
                        let transmit_btn = egui::Button::new(
                            RichText::new("Transmit")
                                .color(theme::DARK_INK)
                                .strong()
                                .size(13.0),
                        )
                        .fill(theme::KINPAKU_GOLD)
                        .min_size(egui::vec2(100.0, 32.0));

                        if (ui.add_enabled(can_send, transmit_btn).clicked() || ctrl_enter)
                            && can_send
                        {
                            if let Some(uid) = focused_uid {
                                self.dispatch_to_session(uid, ctx);
                            }
                        }

                        if self.any_running() {
                            ui.add_space(4.0);
                            let stop_label = if self.running_count() > 1 {
                                "Stop All"
                            } else {
                                "Stop"
                            };
                            let stop_btn = egui::Button::new(
                                RichText::new(stop_label)
                                    .color(theme::ERROR_CORAL)
                                    .size(12.0),
                            )
                            .min_size(egui::vec2(100.0, 24.0));
                            if ui.add(stop_btn).clicked() {
                                self.stop_all();
                            }
                        }
                    });
                });
            });
    }

    fn render_focused_session(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(uid) = self.history.selected_session_uid.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("No active session selected. Click '+ New Session' or 'Import CLI' to start.")
                        .size(14.0)
                        .color(theme::TEXT_MUTED),
                );
            });
            return;
        };

        let Some(mut sess) = self.history.session_by_uid(&uid).cloned() else {
            return;
        };

        let projects_list: Vec<(String, String)> = self
            .history
            .projects
            .iter()
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        let project_dir = self
            .history
            .project(&sess.project_id)
            .and_then(|p| p.working_dir.clone());
        let cli_status = self.cli_statuses.get(&sess.agent).cloned();
        let model_options = self
            .model_catalog
            .get(&sess.agent)
            .cloned()
            .unwrap_or_else(|| models::default_models_for(sess.agent));

        let mut settings_changed = false;
        let mut stop_clicked = false;

        let rt = self.runtime_sessions.entry(uid.clone()).or_default();
        let is_running = rt.is_running();

        // Single-level workspace container with hairline dividers (no nested cards-in-cards).
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(sess.agent.glyph())
                    .strong()
                    .size(18.0)
                    .color(theme::KINPAKU_GOLD),
            );
            if ui
                .add(
                    egui::TextEdit::singleline(&mut sess.title)
                        .desired_width(240.0)
                        .font(egui::TextStyle::Heading),
                )
                .changed()
            {
                settings_changed = true;
            }

            render_status_pill(ui, &rt.run_state, cli_status.as_ref());
            if let Some(elapsed) = format_elapsed(&rt.run_state) {
                ui.label(
                    RichText::new(elapsed)
                        .monospace()
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
            }

            if let Some(status) = &cli_status {
                if let Some(ver) = &status.version {
                    ui.label(RichText::new(ver).size(11.5).color(theme::TEXT_FAINT));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if is_running
                    && ui
                        .button(RichText::new("Stop").color(theme::ERROR_CORAL))
                        .clicked()
                {
                    stop_clicked = true;
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Project:")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            let current_proj_name = projects_list
                .iter()
                .find(|(id, _)| *id == sess.project_id)
                .map(|(_, name)| name.as_str())
                .unwrap_or("General");
            egui::ComboBox::from_id_salt(("sess_proj", &uid))
                .selected_text(current_proj_name)
                .show_ui(ui, |ui| {
                    for (pid, pname) in &projects_list {
                        if ui
                            .selectable_value(&mut sess.project_id, pid.clone(), pname)
                            .changed()
                        {
                            settings_changed = true;
                        }
                    }
                });
            if let Some(dir) = &project_dir {
                ui.label(
                    RichText::new(dir.display().to_string())
                        .size(11.0)
                        .color(theme::TEXT_FAINT),
                );
            }

            ui.add_space(10.0);
            ui.label(RichText::new("Model:").size(12.0).color(theme::TEXT_MUTED));
            let custom_open = self
                .custom_model_input_open
                .entry(uid.clone())
                .or_insert(false);
            if render_model_selector_controls(
                ui,
                ("sess_model", &uid),
                &model_options,
                &mut sess.model,
                custom_open,
            ) {
                settings_changed = true;
            }

            if sess.agent == AgentKind::Agy {
                ui.add_space(8.0);
                ui.label(RichText::new("Mode:").size(12.0).color(theme::TEXT_MUTED));
                egui::ComboBox::from_id_salt(("agy_mode", &uid))
                    .selected_text(if sess.mode.is_empty() {
                        "default"
                    } else {
                        sess.mode.as_str()
                    })
                    .show_ui(ui, |ui| {
                        for opt in ["", "auto", "accept-edits", "plan"] {
                            let label = if opt.is_empty() { "default" } else { opt };
                            if ui
                                .selectable_value(&mut sess.mode, opt.to_string(), label)
                                .clicked()
                            {
                                settings_changed = true;
                            }
                        }
                    });
            }

            if matches!(sess.agent, AgentKind::Agy | AgentKind::Claude) {
                ui.add_space(8.0);
                ui.label(RichText::new("Effort:").size(12.0).color(theme::TEXT_MUTED));
                egui::ComboBox::from_id_salt(("sess_effort", &uid))
                    .selected_text(if sess.effort.is_empty() {
                        "default"
                    } else {
                        sess.effort.as_str()
                    })
                    .show_ui(ui, |ui| {
                        for opt in ["", "low", "medium", "high", "max"] {
                            let label = if opt.is_empty() { "default" } else { opt };
                            if ui
                                .selectable_value(&mut sess.effort, opt.to_string(), label)
                                .clicked()
                            {
                                settings_changed = true;
                            }
                        }
                    });
            }

            ui.add_space(8.0);
            let session_flag = match sess.agent {
                AgentKind::Agy => "CLI --conversation:",
                AgentKind::Claude => "CLI --resume:",
                AgentKind::OpenCode => "CLI --session:",
            };
            ui.label(
                RichText::new(session_flag)
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            let mut cli_id_buf = sess.cli_session_id.clone().unwrap_or_default();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut cli_id_buf)
                        .hint_text("optional ID")
                        .desired_width(140.0),
                )
                .changed()
            {
                sess.cli_session_id = non_empty(&cli_id_buf);
                settings_changed = true;
            }
            if sess.cli_session_id.is_some() && ui.button("Clear").clicked() {
                sess.cli_session_id = None;
                settings_changed = true;
            }
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        let console_height = (ui.available_height() * 0.56).max(180.0);
        render_output_console(ui, &uid, rt, console_height, ctx);

        if stop_clicked {
            rt.cancel();
        }
        if settings_changed {
            if let Some(target) = self.history.session_by_uid_mut(&uid) {
                *target = sess;
            }
            self.persist_history();
        }

        ui.add_space(10.0);
        ui.label(
            RichText::new("Session Turn History")
                .strong()
                .size(13.0)
                .color(theme::CHAMPAGNE),
        );
        ui.add_space(4.0);

        let turns: Vec<_> = self
            .history
            .turns_for_session(&uid)
            .into_iter()
            .cloned()
            .collect();
        egui::ScrollArea::vertical()
            .id_salt(("turn_history", &uid))
            .show(ui, |ui| {
                if turns.is_empty() {
                    ui.label(
                        RichText::new("No recorded turns yet for this session.")
                            .size(12.0)
                            .color(theme::TEXT_FAINT),
                    );
                    return;
                }
                for turn in turns {
                    ui.horizontal(|ui| {
                        let status = if turn.success { "DONE" } else { "FAILED" };
                        let color = if turn.success {
                            theme::VERDIGRIS_PATINA
                        } else {
                            theme::ERROR_CORAL
                        };
                        ui.label(RichText::new(status).strong().size(11.0).color(color));
                        ui.label(
                            RichText::new(format!("{}ms", turn.duration_ms))
                                .size(11.0)
                                .color(theme::TEXT_FAINT),
                        );
                        if let Some(model) = &turn.model {
                            ui.label(
                                RichText::new(format!("[{model}]"))
                                    .size(11.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                        ui.label(
                            RichText::new(&turn.prompt)
                                .size(12.0)
                                .color(theme::CHAMPAGNE),
                        );
                    });
                    if !turn.output.is_empty() {
                        ui.label(
                            RichText::new(
                                turn.output.lines().take(4).collect::<Vec<_>>().join("\n"),
                            )
                            .monospace()
                            .size(11.5)
                            .color(theme::TEXT_MUTED),
                        );
                    }
                    ui.separator();
                }
            });
    }
}

impl eframe::App for SparkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages();

        if self.any_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.render_top_bar(ctx);
        let focused_uid = self.history.selected_session_uid.clone();
        self.render_prompt_bar(ctx, focused_uid.as_deref());
        self.render_sessions_sidebar(ctx);

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::LACQUER_BLACK)
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                self.render_focused_session(ui, ctx);
            });

        self.render_new_session_window(ctx);
        self.render_import_window(ctx);
    }
}

fn filter_pill(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let text_color = if active {
        theme::KINPAKU_GOLD
    } else {
        theme::TEXT_FAINT
    };
    let fill = if active {
        theme::GRAPHITE_2
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        Stroke::new(1.0_f32, theme::KINPAKU_GOLD)
    } else {
        Stroke::new(1.0_f32, theme::HAIRLINE)
    };
    let btn = egui::Button::new(RichText::new(label).color(text_color).size(12.5))
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(6));
    ui.add(btn)
}

fn render_model_selector_controls(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    options: &[ModelOption],
    current_model: &mut String,
    custom_open: &mut bool,
) -> bool {
    let mut changed = false;
    let selected_label = if current_model.is_empty() {
        "CLI default".to_string()
    } else {
        options
            .iter()
            .find(|opt| opt.id == *current_model)
            .map(|opt| opt.label.clone())
            .unwrap_or_else(|| current_model.clone())
    };

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_label)
        .width(210.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current_model.is_empty() && !*custom_open, "CLI default")
                .clicked()
            {
                current_model.clear();
                *custom_open = false;
                changed = true;
            }
            for opt in options {
                let is_sel = current_model == &opt.id && !*custom_open;
                if ui.selectable_label(is_sel, &opt.label).clicked() {
                    *current_model = opt.id.clone();
                    *custom_open = false;
                    changed = true;
                }
            }
            if ui.selectable_label(*custom_open, "Custom...").clicked() {
                *custom_open = true;
            }
        });

    let is_unlisted =
        !current_model.is_empty() && !options.iter().any(|opt| opt.id == *current_model);
    if (*custom_open || is_unlisted)
        && ui
            .add(
                egui::TextEdit::singleline(current_model)
                    .hint_text("custom model ID")
                    .desired_width(150.0),
            )
            .changed()
    {
        changed = true;
    }

    changed
}

fn pick_folder_dialog(initial_dir: Option<&std::path::Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dir) = initial_dir.filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder()
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn compact_session_title(prompt: &str) -> String {
    let first = prompt.lines().next().unwrap_or("").trim();
    let mut out = String::new();
    for ch in first.chars().take(36) {
        out.push(ch);
    }
    if first.chars().count() > 36 {
        out.push_str("...");
    }
    if out.is_empty() {
        "Session".to_string()
    } else {
        out
    }
}

fn session_status_color(state: Option<&RunState>, is_selected: bool) -> Color32 {
    match state {
        Some(RunState::Running { .. }) | Some(RunState::Done { .. }) => theme::VERDIGRIS_PATINA,
        Some(RunState::Failed { .. }) => theme::ERROR_CORAL,
        _ if is_selected => theme::KINPAKU_GOLD,
        _ => theme::TEXT_MUTED,
    }
}

fn render_status_pill(ui: &mut egui::Ui, run_state: &RunState, cli_status: Option<&CliStatus>) {
    let (label, color) = match run_state {
        RunState::Running { .. } => ("RUNNING", theme::VERDIGRIS_PATINA),
        RunState::Done { .. } => ("DONE", theme::VERDIGRIS_PATINA),
        RunState::Failed { .. } => ("FAILED", theme::ERROR_CORAL),
        RunState::Idle => match cli_status {
            None => ("PROBING", theme::TEXT_FAINT),
            Some(s) if s.available => ("READY", theme::TEXT_MUTED),
            Some(_) => ("UNAVAILABLE", theme::ERROR_CORAL),
        },
    };
    ui.label(RichText::new(label).strong().size(11.0).color(color));
}

fn format_elapsed(state: &RunState) -> Option<String> {
    match state {
        RunState::Running { started_at, .. } => {
            Some(format!("{:.1}s", started_at.elapsed().as_secs_f32()))
        }
        RunState::Done { duration_ms } | RunState::Failed { duration_ms, .. } => {
            Some(format!("{:.2}s", *duration_ms as f32 / 1000.0))
        }
        RunState::Idle => None,
    }
}

fn render_output_console(
    ui: &mut egui::Ui,
    session_uid: &str,
    rt: &SessionRuntimeState,
    height: f32,
    _ctx: &egui::Context,
) {
    Frame::new()
        .fill(theme::LACQUER_DEEP)
        .stroke(Stroke::new(1.0_f32, theme::HAIRLINE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_max_height(height);
            egui::ScrollArea::vertical()
                .id_salt(("console", session_uid))
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if rt.stdout.truncated || rt.stderr.truncated {
                        ui.label(
                            RichText::new("[older output truncated to preserve memory]")
                                .monospace()
                                .size(11.5)
                                .color(theme::TEXT_FAINT),
                        );
                    }
                    if rt.stdout.lines.is_empty() && rt.stderr.lines.is_empty() {
                        let hint = if rt.is_running() {
                            "Waiting for output..."
                        } else {
                            "Output stream will appear here."
                        };
                        ui.label(
                            RichText::new(hint)
                                .monospace()
                                .size(12.0)
                                .color(theme::TEXT_FAINT),
                        );
                    }
                    for line in &rt.stdout.lines {
                        ui.label(
                            RichText::new(line)
                                .monospace()
                                .size(12.0)
                                .color(theme::TEXT_WARM),
                        );
                    }
                    for line in &rt.stderr.lines {
                        ui.label(
                            RichText::new(line)
                                .monospace()
                                .size(12.0)
                                .color(theme::ERROR_CORAL),
                        );
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_buffer_enforces_line_and_byte_caps() {
        let mut buf = OutputBuffer::default();
        for i in 0..(MAX_OUTPUT_LINES + 50) {
            buf.push_line(format!("line-{i}"));
        }
        assert!(buf.truncated);
        assert!(buf.lines.len() <= MAX_OUTPUT_LINES);
        assert!(buf.total_bytes <= MAX_OUTPUT_BYTES);
        assert_eq!(
            buf.lines.last().unwrap(),
            &format!("line-{}", MAX_OUTPUT_LINES + 49)
        );
    }

    #[test]
    fn non_empty_trims_whitespace() {
        assert_eq!(non_empty("   "), None);
        assert_eq!(non_empty("  sonnet  "), Some("sonnet".to_string()));
    }

    #[test]
    fn top_bar_ai_filters_filter_sessions_accurately() {
        let mut filters = HashMap::new();
        filters.insert(AgentKind::Agy, true);
        filters.insert(AgentKind::Claude, false);
        filters.insert(AgentKind::OpenCode, true);

        let agy_sess = SessionRecord {
            uid: "s1".to_string(),
            project_id: "p1".to_string(),
            title: "Agy".to_string(),
            agent: AgentKind::Agy,
            cli_session_id: None,
            model: String::new(),
            mode: String::new(),
            effort: String::new(),
            imported: false,
            archived: false,
            created_at_epoch_secs: 0,
            updated_at_epoch_secs: 0,
        };
        let mut claude_sess = agy_sess.clone();
        claude_sess.uid = "s2".to_string();
        claude_sess.agent = AgentKind::Claude;

        let mut archived_agy = agy_sess.clone();
        archived_agy.uid = "s3".to_string();
        archived_agy.archived = true;

        assert!(session_matches_filters(&agy_sess, &filters));
        assert!(!session_matches_filters(&claude_sess, &filters));
        assert!(!session_matches_filters(&archived_agy, &filters));
    }
}
