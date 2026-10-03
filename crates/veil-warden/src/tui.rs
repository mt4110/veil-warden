// SPDX-License-Identifier: MIT
use crate::{output::Snapshot, policy};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap},
};
use std::collections::VecDeque;
use veil_warden_common::{ACTION_DENIED, ConnectEvent, POLICY_CAPACITY, RuleKey, rule_key};
pub const HISTORY: usize = 128;
#[derive(Clone)]
pub struct EventRow {
    pub event: ConnectEvent,
    pub comm: String,
    pub comm_status: &'static str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Add(RuleKey),
    Remove(RuleKey),
}
#[derive(Clone, Default)]
pub struct BackendState {
    pub scan: Option<crate::scan::Summary>,
    pub ready: bool,
    pub finished: bool,
    pub error: Option<String>,
    pub stats: Snapshot,
    pub rules: Vec<(RuleKey, u32)>,
    pub reply: Option<Result<String, String>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Events,
    Rules,
}
pub struct State {
    enforce: bool,
    backend: BackendState,
    events: VecDeque<EventRow>,
    event_index: usize,
    rule_index: usize,
    focus: Focus,
    pending: Option<Command>,
    busy: bool,
    status: String,
    pub history_evicted: u64,
}
impl State {
    pub fn new(enforce: bool) -> Self {
        Self {
            enforce,
            backend: BackendState::default(),
            events: VecDeque::new(),
            event_index: 0,
            rule_index: 0,
            focus: Focus::Events,
            pending: None,
            busy: false,
            status: "準備中: 両フックの起動を待っています".into(),
            history_evicted: 0,
        }
    }
    pub fn push(&mut self, row: EventRow) {
        let follow = self.events.is_empty() || self.event_index + 1 == self.events.len();
        if self.events.len() == HISTORY {
            self.events.pop_front();
            self.event_index = self.event_index.saturating_sub(1);
            self.history_evicted += 1;
        }
        self.events.push_back(row);
        if follow {
            self.event_index = self.events.len() - 1;
        }
    }
    pub fn update(&mut self, mut backend: BackendState) {
        if let Some(reply) = backend.reply.take() {
            self.busy = false;
            self.status = match reply {
                Ok(s) => format!("成功: {s}"),
                Err(e) => format!("更新失敗: {e}"),
            };
        }
        if let Some(error) = &backend.error {
            self.status = format!("受信・描画エラー: {error}");
            self.pending = None;
        }
        if backend.ready && !self.backend.ready && backend.error.is_none() {
            self.status = "準備完了。接続試行を表示します（成功判定ではありません）".into();
        }
        self.rule_index = self.rule_index.min(backend.rules.len().saturating_sub(1));
        self.backend = backend;
    }
    fn move_selection(&mut self, down: bool) {
        let (index, len) = if self.focus == Focus::Events {
            (&mut self.event_index, self.events.len())
        } else {
            (&mut self.rule_index, self.backend.rules.len())
        };
        *index = if down {
            (*index + 1).min(len.saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
    }
    fn prepare(&mut self, remove: bool) {
        if !self.backend.ready || self.backend.error.is_some() {
            self.status = "準備未完了: 操作できません".into();
            return;
        }
        if !self.enforce {
            self.status = "監視モード: 拒否・解除は --enforce 起動時だけ可能です".into();
            return;
        }
        if self.busy {
            self.status = "更新中: 応答を待ってください".into();
            return;
        }
        let key = if remove {
            self.backend.rules.get(self.rule_index).map(|(k, _)| *k)
        } else {
            self.events.get(self.event_index).map(|r| {
                rule_key(
                    r.event.address,
                    r.event.family,
                    r.event.port,
                    r.event.protocol,
                )
            })
        };
        if let Some(key) = key {
            self.pending = Some(if remove {
                Command::Remove(key)
            } else {
                Command::Add(key)
            });
        } else {
            self.status = "対象なし: 接続履歴またはルールを選択してください".into();
        }
    }
    /// Pending commands hold an immutable tuple even when new history arrives.
    fn confirm(&mut self) -> Option<Command> {
        let cmd = self.pending.take()?;
        self.busy = true;
        self.status = "更新中…".into();
        Some(cmd)
    }
}
pub fn draw(frame: &mut Frame, state: &State, ui_dropped: u64) {
    let area = frame.area();
    if area.width < 60 || area.height < if area.width >= 100 { 16 } else { 20 } {
        frame.render_widget(
            Paragraph::new("端末を60列×20行以上に広げてください。\n/warden.slice/warden-test.slice\nq / Ctrl+C: 終了"),
            area,
        );
        return;
    }
    let layout = Layout::default()
        .constraints([
            Constraint::Length(if state.backend.scan.is_some() { 7 } else { 3 }),
            Constraint::Min(5),
            Constraint::Length(6),
        ])
        .split(area);
    let mode = if state.enforce {
        "ENFORCE / 拒否有効"
    } else {
        "OBSERVE / 監視のみ"
    };
    let loading = if state.backend.finished {
        "停止済み / 拒否解除"
    } else if state.backend.ready {
        "準備完了"
    } else {
        "準備中"
    };
    let scan = state
        .backend
        .scan
        .map(|s| format!("\n{s}"))
        .unwrap_or_default();
    frame.render_widget(Paragraph::new(format!("veil-warden  {mode}  {loading}\n対象: /warden.slice/warden-test.slice | 新規TCP接続 | 既存接続は継続{scan}")).wrap(Wrap { trim: false }),layout[0]);
    let panes = Layout::default()
        .direction(if area.width >= 100 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);
    let rows = state.events.iter().map(|r| {
        let e = &r.event;
        let key = rule_key(e.address, e.family, e.port, e.protocol);
        Row::new(vec![
            if e.action == ACTION_DENIED {
                "拒否 / DENY".to_owned()
            } else {
                "許可 / ALLOW".to_owned()
            },
            format!("{} {} ({})", e.tgid, r.comm, r.comm_status),
            policy::destination(&key),
        ])
    });
    let mut selected = TableState::default();
    if !state.events.is_empty() {
        selected.select(Some(state.event_index));
    }
    let title = if state.events.is_empty() {
        "接続履歴: まだありません"
    } else {
        "接続履歴（試行）"
    };
    frame.render_stateful_widget(
        Table::new(
            rows,
            [
                Constraint::Length(13),
                Constraint::Percentage(40),
                Constraint::Min(15),
            ],
        )
        .header(Row::new(["判定", "PID / 名前", "宛先TCP"]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(if state.focus == Focus::Events {
                    Color::Cyan
                } else {
                    Color::Gray
                })),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray)),
        panes[0],
        &mut selected,
    );
    let rows =
        state.backend.rules.iter().map(|(key, id)| {
            Row::new([id.to_string(), format!("{} TCP", policy::destination(key))])
        });
    let mut selected = TableState::default();
    if !state.backend.rules.is_empty() {
        selected.select(Some(state.rule_index));
    }
    frame.render_stateful_widget(
        Table::new(rows, [Constraint::Length(4), Constraint::Min(16)])
            .header(Row::new(["ID", "拒否宛先"]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        "拒否ルール {}/{}",
                        state.backend.rules.len(),
                        POLICY_CAPACITY
                    ))
                    .border_style(Style::default().fg(if state.focus == Focus::Rules {
                        Color::Cyan
                    } else {
                        Color::Gray
                    })),
            )
            .row_highlight_style(Style::default().bg(Color::DarkGray)),
        panes[1],
        &mut selected,
    );
    let s = state.backend.stats;
    let status = if let Some(cmd) = state.pending {
        let (op, key) = match cmd {
            Command::Add(k) => ("拒否する", k),
            Command::Remove(k) => ("解除する", k),
        };
        format!(
            "確認: {} TCPを{op} | Enter: 実行 / Esc: 取消",
            policy::destination(&key)
        )
    } else {
        state.status.clone()
    };
    let footer = Layout::default()
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(layout[2]);
    frame.render_widget(
        Paragraph::new(format!(
            "試行={} 拒否={}\n欠落: ring={} decode={} queue={} ui={} 履歴入替={}",
            s.attempted,
            s.denied,
            s.ring_dropped,
            s.decode_errors,
            s.queue_dropped,
            ui_dropped,
            state.history_evicted
        )),
        footer[0],
    );
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: false }), footer[1]);
    frame.render_widget(
        Paragraph::new(
            "Tab: ペイン  ↑↓/j/k: 選択  b: 拒否  d: 解除\nEnter: 実行  Esc: 取消  q/Ctrl+C: 終了",
        ),
        footer[2],
    );
}
#[cfg(target_os = "linux")]
pub mod runtime {
    use super::*;
    use crossterm::{
        cursor::{Hide, Show},
        event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
        execute,
        terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use std::{
        io::{self, IsTerminal},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc::{self, SyncSender, TrySendError},
        },
        thread,
        time::Duration,
    };
    pub struct Session {
        raw: bool,
        alternate: bool,
    }
    impl Session {
        fn enter() -> io::Result<Self> {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                return Err(io::Error::other(
                    "--tui requires interactive stdin/stdout (SSH: use -t)",
                ));
            }
            let mut this = Self {
                raw: false,
                alternate: false,
            };
            terminal::enable_raw_mode()?;
            this.raw = true;
            this.alternate = true;
            execute!(io::stdout(), EnterAlternateScreen, Hide)?;
            Ok(this)
        }
        fn restore(&mut self) -> io::Result<()> {
            let screen = if self.alternate {
                let result = execute!(io::stdout(), Show, LeaveAlternateScreen);
                if result.is_ok() {
                    self.alternate = false;
                }
                result
            } else {
                Ok(())
            };
            let raw = if self.raw {
                let result = terminal::disable_raw_mode();
                if result.is_ok() {
                    self.raw = false;
                }
                result
            } else {
                Ok(())
            };
            screen.and(raw)
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            let _ = self.restore();
        }
    }
    pub struct Ui {
        pub shared: Arc<Mutex<BackendState>>,
        pub rows: SyncSender<EventRow>,
        pub dropped: Arc<AtomicU64>,
        worker: Option<thread::JoinHandle<io::Result<()>>>,
    }
    impl Ui {
        pub fn start(
            enforce: bool,
            stop: Arc<AtomicBool>,
        ) -> io::Result<(Self, tokio::sync::mpsc::Receiver<Command>)> {
            let shared = Arc::new(Mutex::new(BackendState::default()));
            let dropped = Arc::new(AtomicU64::new(0));
            let (rows, rx) = mpsc::sync_channel(128);
            let (commands, commands_rx) = tokio::sync::mpsc::channel(8);
            let (ready, ready_rx) = mpsc::sync_channel(1);
            let backend = shared.clone();
            let lost = dropped.clone();
            let worker = thread::Builder::new().name("tui".into()).spawn(move || {
                let result = (|| {
                    let mut session = match Session::enter() {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = ready.send(Err(e.to_string()));
                            return Err(e);
                        }
                    };
                    let mut terminal = match ratatui::Terminal::new(
                        ratatui::backend::CrosstermBackend::new(io::stdout()),
                    ) {
                        Ok(t) => t,
                        Err(e) => {
                            let _ = ready.send(Err(e.to_string()));
                            return Err(e);
                        }
                    };
                    let mut state = State::new(enforce);
                    if let Err(e) = terminal.draw(|f| draw(f, &state, 0)) {
                        let _ = ready.send(Err(e.to_string()));
                        return Err(e);
                    }
                    let _ = ready.send(Ok(()));
                    loop {
                        let snapshot = {
                            let mut b = backend
                                .lock()
                                .map_err(|_| io::Error::other("UI state poisoned"))?;
                            let snapshot = b.clone();
                            b.reply = None;
                            snapshot
                        };
                        state.update(snapshot);
                        for _ in 0..128 {
                            match rx.try_recv() {
                                Ok(row) => state.push(row),
                                Err(_) => break,
                            }
                        }
                        terminal.draw(|f| draw(f, &state, lost.load(Ordering::Relaxed)))?;
                        if state.backend.finished {
                            break;
                        }
                        if event::poll(Duration::from_millis(50))?
                            && let Event::Key(key) = event::read()?
                        {
                            if key.kind != KeyEventKind::Press {
                                continue;
                            }
                            match key.code {
                                KeyCode::Char('q') => stop.store(true, Ordering::Relaxed),
                                KeyCode::Char('c')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    stop.store(true, Ordering::Relaxed)
                                }
                                KeyCode::Tab => {
                                    state.focus = if state.focus == Focus::Events {
                                        Focus::Rules
                                    } else {
                                        Focus::Events
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => state.move_selection(false),
                                KeyCode::Down | KeyCode::Char('j') => state.move_selection(true),
                                KeyCode::Char('b') => state.prepare(false),
                                KeyCode::Char('d') => state.prepare(true),
                                KeyCode::Esc => {
                                    state.pending = None;
                                    state.status = "操作を取り消しました".into();
                                }
                                KeyCode::Enter => {
                                    if let Some(cmd) = state.confirm()
                                        && commands.try_send(cmd).is_err()
                                    {
                                        state.busy = false;
                                        state.status = "更新失敗: 制御経路が利用できません".into();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    session.restore()
                })();
                if result.is_err() {
                    stop.store(true, Ordering::Relaxed);
                }
                result
            })?;
            match ready_rx.recv() {
                Ok(Ok(())) => Ok((
                    Self {
                        shared,
                        rows,
                        dropped,
                        worker: Some(worker),
                    },
                    commands_rx,
                )),
                other => {
                    let _ = worker.join();
                    Err(io::Error::other(format!("TUI startup failed: {other:?}")))
                }
            }
        }
        pub fn state(&self, update: impl FnOnce(&mut BackendState)) -> io::Result<()> {
            let mut state = self
                .shared
                .lock()
                .map_err(|_| io::Error::other("UI state poisoned"))?;
            update(&mut state);
            Ok(())
        }
        pub fn finish(&mut self) -> io::Result<()> {
            self.state(|s| s.finished = true)?;
            if let Some(worker) = self.worker.take() {
                worker
                    .join()
                    .map_err(|_| io::Error::other("UI thread panicked"))??;
            }
            Ok(())
        }
    }
    impl Drop for Ui {
        fn drop(&mut self) {
            let _ = self.finish();
        }
    }
    pub fn worker(
        mut rx: tokio::sync::mpsc::Receiver<crate::output::Message>,
        tx: SyncSender<EventRow>,
        dropped: Arc<AtomicU64>,
    ) -> io::Result<()> {
        while let Some(message) = rx.blocking_recv() {
            if let crate::output::Message::Event(event) = message {
                let (comm, comm_status) = crate::process::comm(event.tgid);
                match tx.try_send(EventRow {
                    event,
                    comm,
                    comm_status,
                }) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => {
                        dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        return Err(io::Error::other("TUI event receiver stopped"));
                    }
                }
            }
        }
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        struct BrokenWriter;
        impl io::Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("synthetic draw failure"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        #[test]
        #[ignore = "requires a real PTY; exercised by test-tui-vm.sh"]
        fn draw_error_restores_terminal_in_pty() {
            // The dedicated VM PTY suite explicitly runs this ignored fixture.
            assert!(
                io::stdin().is_terminal(),
                "run this fixture inside the PTY suite"
            );
            assert!(!terminal::is_raw_mode_enabled().unwrap());
            {
                let _session = Session::enter().unwrap();
                assert!(terminal::is_raw_mode_enabled().unwrap());
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(BrokenWriter))
                        .unwrap();
                let error = terminal
                    .draw(|f| draw(f, &State::new(false), 0))
                    .unwrap_err();
                assert!(error.to_string().contains("synthetic draw failure"));
            }
            assert!(!terminal::is_raw_mode_enabled().unwrap());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(port: u16) -> EventRow {
        EventRow {
            event: ConnectEvent {
                timestamp_ns: 1,
                cgroup_id: 1,
                tgid: 10,
                tid: 10,
                address: [0; 16],
                port,
                abi_version: veil_warden_common::ABI_VERSION,
                family: 6,
                protocol: 6,
                action: ACTION_DENIED,
                hook: 6,
                policy_id: 1,
                reserved: 0,
            },
            comm: "テスト".into(),
            comm_status: "best_effort",
        }
    }
    #[test]
    fn bounded_history_and_confirmation_keep_exact_tuple() {
        let mut s = State::new(true);
        s.update(BackendState {
            ready: true,
            ..Default::default()
        });
        s.push(event(443));
        s.prepare(false);
        for port in 1..300 {
            s.push(event(port));
        }
        assert_eq!(s.events.len(), HISTORY);
        assert_eq!(s.history_evicted, 172);
        assert_eq!(
            s.confirm(),
            Some(Command::Add(rule_key([0; 16], 6, 443, 6)))
        );
        s.update(BackendState {
            ready: true,
            reply: Some(Err("capacity full".into())),
            ..Default::default()
        });
        assert!(s.status.contains("更新失敗"));
        assert!(!s.busy);
        let mut observed = State::new(false);
        observed.update(BackendState {
            ready: true,
            ..Default::default()
        });
        observed.push(event(443));
        observed.prepare(false);
        assert!(observed.confirm().is_none());
    }
    #[test]
    fn japanese_empty_errors_and_small_resize_render() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 30)).unwrap();
        let mut s = State::new(false);
        terminal.draw(|f| draw(f, &s, 2)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        // Wide glyphs occupy a blank continuation cell in TestBackend's storage.
        let compact = text.replace(' ', "");
        assert!(
            compact.contains("準備中")
                && compact.contains("まだありません")
                && text.contains("ui=2")
        );
        s.update(BackendState {
            ready: true,
            error: Some("receive failed".into()),
            ..Default::default()
        });
        terminal.draw(|f| draw(f, &s, 0)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("receive failed"));
        terminal.backend_mut().resize(40, 10);
        terminal
            .resize(ratatui::layout::Rect::new(0, 0, 40, 10))
            .unwrap();
        terminal.draw(|f| draw(f, &s, 0)).unwrap();
    }
}
