//! Spark GUI application state and Impeccable Neo Kinpaku views.

use crate::cli::detectors::{self, CliStatus};
use crate::cli::runner::{self, RunRequest, RunningProcess, StreamEvent};
use crate::cli::AgentKind;
use crate::history::HistoryStore;
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

/// Active top-level navigation view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Matrix,
    Agent(AgentKind),
}

/// Execution lifecycle state for an agent pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
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
    DetectionComplete(Vec<CliStatus>),
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

/// Per-agent runtime and configuration state.
pub struct AgentPaneState {
    pub agent: AgentKind,
    pub cli_status: Option<CliStatus>,
    pub selected: bool,
    pub run_state: RunState,
    pub process: Option<RunningProcess>,
    pub stdout: OutputBuffer,
    pub stderr: OutputBuffer,
    pub session_id: String,
    pub model: String,
    pub mode: String,
    pub effort: String,
}

impl AgentPaneState {
    fn new(agent: AgentKind, history: &HistoryStore) -> Self {
        let saved = history.session(agent);
        Self {
            agent,
            cli_status: None,
            selected: false,
            run_state: RunState::Idle,
            process: None,
            stdout: OutputBuffer::default(),
            stderr: OutputBuffer::default(),
            session_id: saved
                .and_then(|s| s.session_id.clone())
                .unwrap_or_default(),
            model: saved.map(|s| s.model.clone()).unwrap_or_default(),
            mode: saved.map(|s| s.mode.clone()).unwrap_or_default(),
            effort: saved.map(|s| s.effort.clone()).unwrap_or_default(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.cli_status.as_ref().is_some_and(|s| s.available)
    }

    pub fn is_running(&self) -> bool {
        matches!(self.run_state, RunState::Running { .. })
    }

    pub fn cancel(&mut self) {
        if let Some(proc) = self.process.as_mut() {
            proc.cancel();
        }
    }
}

/// Main Spark desktop application.
pub struct SparkApp {
    runtime: Runtime,
    ui_tx: std_mpsc::Sender<UiMessage>,
    ui_rx: std_mpsc::Receiver<UiMessage>,
    active_tab: ActiveTab,
    panes: HashMap<AgentKind, AgentPaneState>,
    prompt: String,
    working_dir: Option<PathBuf>,
    history: HistoryStore,
    history_path: Option<PathBuf>,
    last_error: Option<String>,
    detecting: bool,
}

impl SparkApp {
    pub fn new(ctx: egui::Context) -> Self {
        let runtime = Runtime::new().expect("Tokio runtime should initialize");
        let (ui_tx, ui_rx) = std_mpsc::channel();

        let history_path = HistoryStore::default_path();
        let history = history_path
            .as_deref()
            .and_then(|p| HistoryStore::load_from(p).ok())
            .unwrap_or_default();

        let mut panes = HashMap::new();
        for agent in AgentKind::ALL {
            panes.insert(agent, AgentPaneState::new(agent, &history));
        }

        let working_dir = std::env::current_dir().ok();

        let mut app = Self {
            runtime,
            ui_tx,
            ui_rx,
            active_tab: ActiveTab::Matrix,
            panes,
            prompt: String::new(),
            working_dir,
            history,
            history_path,
            last_error: None,
            detecting: false,
        };

        app.trigger_detection(ctx);
        app
    }

    fn trigger_detection(&mut self, ctx: egui::Context) {
        self.detecting = true;
        let tx = self.ui_tx.clone();
        self.runtime.spawn(async move {
            let statuses = detectors::detect_all().await;
            let _ = tx.send(UiMessage::DetectionComplete(statuses));
            ctx.request_repaint();
        });
    }

    fn persist_history(&mut self) {
        for agent in AgentKind::ALL {
            if let Some(pane) = self.panes.get(&agent) {
                let state = self.history.session_mut(agent);
                state.session_id = if pane.session_id.trim().is_empty() {
                    None
                } else {
                    Some(pane.session_id.trim().to_string())
                };
                state.model = pane.model.trim().to_string();
                state.mode = pane.mode.trim().to_string();
                state.effort = pane.effort.trim().to_string();
            }
        }

        if let Some(path) = self.history_path.as_deref() {
            if let Err(err) = self.history.save_to(path) {
                self.last_error = Some(err);
            }
        }
    }

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMessage::DetectionComplete(statuses) => {
                    self.detecting = false;
                    for status in statuses {
                        if let Some(pane) = self.panes.get_mut(&status.agent) {
                            // Auto-select available CLIs on initial probe.
                            if pane.cli_status.is_none() && status.available {
                                pane.selected = true;
                            }
                            pane.cli_status = Some(status);
                        }
                    }
                }
                UiMessage::Stream(ev) => self.handle_stream_event(ev),
            }
        }
    }

    fn handle_stream_event(&mut self, ev: StreamEvent) {
        match ev {
            StreamEvent::StdoutLine { agent, line } => {
                if let Some(pane) = self.panes.get_mut(&agent) {
                    pane.stdout.push_line(line);
                }
            }
            StreamEvent::StderrLine { agent, line } => {
                if let Some(pane) = self.panes.get_mut(&agent) {
                    pane.stderr.push_line(line);
                }
            }
            StreamEvent::Finished {
                agent,
                exit_code,
                success,
            } => {
                let mut completed_turn = None;
                if let Some(pane) = self.panes.get_mut(&agent) {
                    pane.process = None;
                    let (duration_ms, prompt) = match &pane.run_state {
                        RunState::Running { started_at, prompt } => (
                            started_at.elapsed().as_millis() as u64,
                            prompt.clone(),
                        ),
                        _ => (0, String::new()),
                    };

                    pane.run_state = if success {
                        RunState::Done { duration_ms }
                    } else {
                        RunState::Failed {
                            duration_ms,
                            exit_code,
                        }
                    };

                    completed_turn = Some((
                        prompt,
                        pane.stdout.joined(),
                        pane.stderr.joined(),
                        duration_ms,
                        non_empty(&pane.session_id),
                        non_empty(&pane.model),
                        non_empty(&pane.mode),
                        non_empty(&pane.effort),
                    ));
                }

                if let Some((prompt, output, stderr, duration_ms, session_id, model, mode, effort)) =
                    completed_turn
                {
                    if !prompt.is_empty() {
                        self.history.record_turn(
                            agent,
                            prompt,
                            output,
                            stderr,
                            success,
                            exit_code,
                            duration_ms,
                            session_id,
                            model,
                            mode,
                            effort,
                        );
                        self.persist_history();
                    }
                }
            }
            StreamEvent::Error { agent, message } => {
                if let Some(pane) = self.panes.get_mut(&agent) {
                    pane.process = None;
                    pane.stderr.push_line(message);
                    let duration_ms = match &pane.run_state {
                        RunState::Running { started_at, .. } => {
                            started_at.elapsed().as_millis() as u64
                        }
                        _ => 0,
                    };
                    pane.run_state = RunState::Failed {
                        duration_ms,
                        exit_code: None,
                    };
                }
            }
        }
    }

    fn dispatch_to_agents(&mut self, targets: &[AgentKind], ctx: &egui::Context) {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        self.last_error = None;

        for &agent in targets {
            let Some(pane) = self.panes.get_mut(&agent) else {
                continue;
            };
            if pane.is_running() {
                continue;
            }
            let Some(binary_path) = pane
                .cli_status
                .as_ref()
                .filter(|s| s.available)
                .and_then(|s| s.binary_path.clone())
            else {
                continue;
            };

            let mut req = RunRequest::new(agent, prompt.clone());
            req.working_dir = self.working_dir.clone();
            req.session_id = non_empty(&pane.session_id);
            req.model = non_empty(&pane.model);
            req.mode = non_empty(&pane.mode);
            req.effort = non_empty(&pane.effort);

            let (tokio_tx, mut tokio_rx) = tokio_mpsc::unbounded_channel();
            let _guard = self.runtime.enter();
            match runner::spawn_agent(&binary_path, &req, tokio_tx) {
                Ok(proc_handle) => {
                    pane.stdout.clear();
                    pane.stderr.clear();
                    pane.process = Some(proc_handle);
                    pane.run_state = RunState::Running {
                        started_at: Instant::now(),
                        prompt: prompt.clone(),
                    };

                    let ui_tx = self.ui_tx.clone();
                    let repaint_ctx = ctx.clone();
                    self.runtime.spawn(async move {
                        while let Some(ev) = tokio_rx.recv().await {
                            let _ = ui_tx.send(UiMessage::Stream(ev));
                            repaint_ctx.request_repaint();
                        }
                    });
                }
                Err(err) => {
                    pane.stderr.push_line(err.clone());
                    pane.run_state = RunState::Failed {
                        duration_ms: 0,
                        exit_code: None,
                    };
                    self.last_error = Some(err);
                }
            }
        }
    }

    fn any_running(&self) -> bool {
        self.panes.values().any(AgentPaneState::is_running)
    }

    fn stop_all(&mut self) {
        for pane in self.panes.values_mut() {
            pane.cancel();
        }
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
                    ui.add_space(12.0);

                    self.tab_button(ui, ActiveTab::Matrix, "Matrix");
                    for agent in AgentKind::ALL {
                        let label = format!("{} {}", agent.glyph(), agent.display_name());
                        self.tab_button(ui, ActiveTab::Agent(agent), &label);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let rescan_label = if self.detecting {
                            "Scanning..."
                        } else {
                            "Re-scan CLIs"
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
                    });
                });
            });
    }

    fn tab_button(&mut self, ui: &mut egui::Ui, tab: ActiveTab, label: &str) {
        let active = self.active_tab == tab;
        let text_color = if active {
            theme::KINPAKU_GOLD
        } else {
            theme::TEXT_MUTED
        };
        let fill = if active {
            theme::GRAPHITE_2
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if active {
            Stroke::new(1.0_f32, theme::KINPAKU_GOLD)
        } else {
            Stroke::new(1.0_f32, Color32::TRANSPARENT)
        };

        let btn = egui::Button::new(RichText::new(label).color(text_color).size(13.0))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(6));
        if ui.add(btn).clicked() {
            self.active_tab = tab;
        }
    }

    fn render_prompt_bar(&mut self, ctx: &egui::Context, single_agent: Option<AgentKind>) {
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

                let ctrl_enter = ui.input(|i| {
                    i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)
                });

                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::multiline(&mut self.prompt)
                        .hint_text("Enter prompt... (Ctrl+Enter to transmit)")
                        .desired_rows(2)
                        .desired_width((ui.available_width() - 190.0).max(200.0));
                    ui.add(edit);

                    ui.vertical(|ui| {
                        let targets: Vec<AgentKind> = match single_agent {
                            Some(agent) => vec![agent],
                            None => AgentKind::ALL
                                .into_iter()
                                .filter(|a| {
                                    self.panes
                                        .get(a)
                                        .is_some_and(|p| p.selected && p.is_available())
                                })
                                .collect(),
                        };

                        let can_send = !self.prompt.trim().is_empty() && !targets.is_empty();
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
                            self.dispatch_to_agents(&targets, ctx);
                        }

                        if self.any_running() {
                            ui.add_space(4.0);
                            let stop_btn = egui::Button::new(
                                RichText::new("Stop All")
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

    fn render_matrix_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let available_height = ui.available_height();
        ui.columns(3, |cols| {
            for (col, agent) in cols.iter_mut().zip(AgentKind::ALL) {
                let Some(pane) = self.panes.get_mut(&agent) else {
                    continue;
                };
                render_agent_column(col, pane, available_height, ctx);
            }
        });
    }

    fn render_agent_detail_view(&mut self, ui: &mut egui::Ui, agent: AgentKind, ctx: &egui::Context) {
        let mut settings_changed = false;

        if let Some(pane) = self.panes.get_mut(&agent) {
            Frame::new()
                .fill(theme::RAISED_LACQUER)
                .stroke(Stroke::new(1.0_f32, theme::HAIRLINE))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{} {}", agent.glyph(), agent.display_name()))
                                .strong()
                                .size(16.0)
                                .color(theme::CHAMPAGNE),
                        );
                        render_status_pill(ui, pane);
                        if let Some(status) = &pane.cli_status {
                            if let Some(ver) = &status.version {
                                ui.label(RichText::new(ver).size(12.0).color(theme::TEXT_FAINT));
                            }
                            if let Some(path) = &status.binary_path {
                                ui.label(
                                    RichText::new(path.display().to_string())
                                        .size(11.0)
                                        .color(theme::TEXT_FAINT),
                                );
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if pane.is_running()
                                && ui
                                    .button(RichText::new("Stop").color(theme::ERROR_CORAL))
                                    .clicked()
                            {
                                pane.cancel();
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let session_flag = match agent {
                            AgentKind::Agy => "Session (--conversation):",
                            AgentKind::Claude => "Session (--resume):",
                            AgentKind::OpenCode => "Session (--session):",
                        };
                        ui.label(RichText::new(session_flag).size(12.0).color(theme::TEXT_MUTED));
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut pane.session_id)
                                    .hint_text("optional ID")
                                    .desired_width(140.0),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }
                        if !pane.session_id.is_empty() && ui.button("Clear").clicked() {
                            pane.session_id.clear();
                            settings_changed = true;
                        }

                        ui.add_space(8.0);
                        ui.label(RichText::new("Model:").size(12.0).color(theme::TEXT_MUTED));
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut pane.model)
                                    .hint_text("default")
                                    .desired_width(140.0),
                            )
                            .changed()
                        {
                            settings_changed = true;
                        }

                        if agent == AgentKind::Agy {
                            ui.add_space(8.0);
                            ui.label(RichText::new("Mode:").size(12.0).color(theme::TEXT_MUTED));
                            egui::ComboBox::from_id_salt("agy_mode")
                                .selected_text(if pane.mode.is_empty() {
                                    "default"
                                } else {
                                    pane.mode.as_str()
                                })
                                .show_ui(ui, |ui| {
                                    for opt in ["", "auto", "accept-edits", "plan"] {
                                        let label = if opt.is_empty() { "default" } else { opt };
                                        if ui.selectable_value(&mut pane.mode, opt.to_string(), label).clicked() {
                                            settings_changed = true;
                                        }
                                    }
                                });
                        }

                        if matches!(agent, AgentKind::Agy | AgentKind::Claude) {
                            ui.add_space(8.0);
                            ui.label(RichText::new("Effort:").size(12.0).color(theme::TEXT_MUTED));
                            egui::ComboBox::from_id_salt(format!("{}_effort", agent.binary_name()))
                                .selected_text(if pane.effort.is_empty() {
                                    "default"
                                } else {
                                    pane.effort.as_str()
                                })
                                .show_ui(ui, |ui| {
                                    for opt in ["", "low", "medium", "high", "max"] {
                                        let label = if opt.is_empty() { "default" } else { opt };
                                        if ui.selectable_value(&mut pane.effort, opt.to_string(), label).clicked() {
                                            settings_changed = true;
                                        }
                                    }
                                });
                        }
                    });

                    ui.add_space(8.0);
                    let console_height = (ui.available_height() * 0.55).max(180.0);
                    render_output_console(ui, pane, console_height, ctx);
                });
        }

        if settings_changed {
            self.persist_history();
        }

        ui.add_space(10.0);
        ui.label(
            RichText::new("Turn History")
                .strong()
                .size(13.0)
                .color(theme::CHAMPAGNE),
        );
        ui.add_space(4.0);

        let turns: Vec<_> = self.history.turns_for(agent).into_iter().cloned().collect();
        egui::ScrollArea::vertical()
            .id_salt(format!("history_{}", agent.binary_name()))
            .show(ui, |ui| {
                if turns.is_empty() {
                    ui.label(
                        RichText::new("No recorded turns yet for this agent.")
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
                        ui.label(
                            RichText::new(&turn.prompt)
                                .size(12.0)
                                .color(theme::CHAMPAGNE),
                        );
                    });
                    if !turn.output.is_empty() {
                        ui.label(
                            RichText::new(turn.output.lines().take(4).collect::<Vec<_>>().join("\n"))
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
        let single_agent = match self.active_tab {
            ActiveTab::Matrix => None,
            ActiveTab::Agent(agent) => Some(agent),
        };
        self.render_prompt_bar(ctx, single_agent);

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::LACQUER_BLACK)
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| match self.active_tab {
                ActiveTab::Matrix => self.render_matrix_view(ui, ctx),
                ActiveTab::Agent(agent) => self.render_agent_detail_view(ui, agent, ctx),
            });
    }
}

fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn render_agent_column(
    ui: &mut egui::Ui,
    pane: &mut AgentPaneState,
    available_height: f32,
    ctx: &egui::Context,
) {
    Frame::new()
        .fill(theme::RAISED_LACQUER)
        .stroke(Stroke::new(1.0_f32, theme::HAIRLINE))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_min_height((available_height - 24.0).max(240.0));
            ui.horizontal(|ui| {
                let avail = pane.is_available();
                ui.add_enabled(avail, egui::Checkbox::new(&mut pane.selected, ""));
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        pane.agent.glyph(),
                        pane.agent.display_name()
                    ))
                    .strong()
                    .size(14.0)
                    .color(theme::CHAMPAGNE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pane.is_running()
                        && ui
                            .button(RichText::new("Stop").color(theme::ERROR_CORAL).size(12.0))
                            .clicked()
                    {
                        pane.cancel();
                    }
                    render_status_pill(ui, pane);
                });
            });

            ui.horizontal(|ui| {
                let version_text = match &pane.cli_status {
                    None => "Probing...".to_string(),
                    Some(s) if s.available => s
                        .version
                        .clone()
                        .unwrap_or_else(|| "available".to_string()),
                    Some(s) => s
                        .error
                        .clone()
                        .unwrap_or_else(|| "not found".to_string()),
                };
                ui.label(
                    RichText::new(version_text)
                        .size(11.5)
                        .color(theme::TEXT_FAINT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(elapsed) = format_elapsed(&pane.run_state) {
                        ui.label(
                            RichText::new(elapsed)
                                .monospace()
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });
            });

            ui.separator();
            let console_height = (ui.available_height() - 4.0).max(160.0);
            render_output_console(ui, pane, console_height, ctx);
        });
}

fn render_status_pill(ui: &mut egui::Ui, pane: &AgentPaneState) {
    let (label, color) = match &pane.run_state {
        RunState::Running { .. } => ("RUNNING", theme::VERDIGRIS_PATINA),
        RunState::Done { .. } => ("DONE", theme::VERDIGRIS_PATINA),
        RunState::Failed { .. } => ("FAILED", theme::ERROR_CORAL),
        RunState::Idle => match &pane.cli_status {
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
    pane: &AgentPaneState,
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
                .id_salt(format!("console_{}", pane.agent.binary_name()))
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if pane.stdout.truncated || pane.stderr.truncated {
                        ui.label(
                            RichText::new("[older output truncated to preserve memory]")
                                .monospace()
                                .size(11.5)
                                .color(theme::TEXT_FAINT),
                        );
                    }
                    if pane.stdout.lines.is_empty() && pane.stderr.lines.is_empty() {
                        let hint = if pane.is_running() {
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
                    for line in &pane.stdout.lines {
                        ui.label(
                            RichText::new(line)
                                .monospace()
                                .size(12.0)
                                .color(theme::TEXT_WARM),
                        );
                    }
                    for line in &pane.stderr.lines {
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
        assert_eq!(buf.lines.last().unwrap(), &format!("line-{}", MAX_OUTPUT_LINES + 49));
    }

    #[test]
    fn non_empty_trims_whitespace() {
        assert_eq!(non_empty("   "), None);
        assert_eq!(non_empty("  sonnet  "), Some("sonnet".to_string()));
    }
}
