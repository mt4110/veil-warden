// SPDX-License-Identifier: MIT
//! In-memory tutorial: never loads BPF, opens sockets, or reads host processes.
use crate::{
    terminal_session::Session,
    tui::{self, BackendState, Command, EventRow, Input, State},
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::{io, time::Duration};
use veil_warden_common::{
    ABI_VERSION, ACTION_ALLOWED, ACTION_DENIED, ConnectEvent, POLICY_CAPACITY, TCP, rule_key,
};

#[derive(Default)]
struct Preview {
    backend: BackendState,
    next_id: u32,
}
impl Preview {
    fn new() -> Self {
        Self {
            backend: BackendState {
                ready: true,
                ..Default::default()
            },
            next_id: 1,
        }
    }
    fn apply(&mut self, command: Command) {
        let rules = &mut self.backend.rules;
        self.backend.reply = Some(match command {
            Command::Add(key) if rules.iter().any(|(k, _)| *k == key) => {
                Err("模擬: この宛先は登録済みです".into())
            }
            Command::Add(_) if rules.len() >= POLICY_CAPACITY as usize => {
                Err("模擬: ルールは最大16件です".into())
            }
            Command::Add(key) => {
                rules.push((key, self.next_id));
                self.next_id += 1;
                Ok("模擬ルールを追加。nで次の接続試行を確認できます（実拒否なし）".into())
            }
            Command::Remove(key) => match rules.iter().position(|(k, _)| *k == key) {
                Some(index) => {
                    rules.remove(index);
                    Ok("模擬ルールを解除。nで次の接続試行を確認できます".into())
                }
                None => Err("模擬: この宛先は登録されていません".into()),
            },
        });
    }
    fn attempt(&mut self, mut row: EventRow) -> EventRow {
        let e = &mut row.event;
        let key = rule_key(e.address, e.family, e.port, e.protocol);
        let id = self
            .backend
            .rules
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, id)| *id)
            .unwrap_or(0);
        e.policy_id = id;
        e.action = if id == 0 {
            ACTION_ALLOWED
        } else {
            ACTION_DENIED
        };
        self.backend.stats.attempted += 1;
        self.backend.stats.emitted += 1;
        self.backend.stats.decoded += 1;
        self.backend.stats.denied += u128::from(id != 0);
        e.timestamp_ns = self.backend.stats.attempted as u64;
        row
    }
}
fn sample(ip: [u8; 4], port: u16, pid: u32, name: &str) -> EventRow {
    let mut address = [0; 16];
    address[..4].copy_from_slice(&ip);
    EventRow {
        event: ConnectEvent {
            timestamp_ns: 0,
            cgroup_id: 0,
            tgid: pid,
            tid: pid,
            address,
            port,
            abi_version: ABI_VERSION,
            family: 4,
            protocol: TCP,
            action: ACTION_ALLOWED,
            hook: 4,
            policy_id: 0,
            reserved: 0,
        },
        comm: name.into(),
        comm_status: "模擬 / sample",
    }
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::enter()?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))?;
    let mut state = State::preview();
    let mut preview = Preview::new();
    // Documentation-only addresses. No connections are made to these destinations.
    for row in [
        sample([192, 0, 2, 10], 443, 1001, "package-fetch"),
        sample([198, 51, 100, 20], 5432, 1002, "app-worker"),
        sample([203, 0, 113, 30], 8443, 1002, "app-worker"),
    ] {
        state.push(preview.attempt(row));
    }
    loop {
        state.update(preview.backend.clone());
        preview.backend.reply = None;
        terminal.draw(|frame| tui::draw(frame, &state, 0))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('n') {
                if let Some(row) = state.selected_event() {
                    state.push(preview.attempt(row));
                }
                continue;
            }
            match state.key(key) {
                Input::Quit => break,
                Input::Command(command) => preview.apply(command),
                Input::None => {}
            }
        }
    }
    session.restore()?;
    println!("DRY-RUN終了: 模擬データのみ。実通信・実拒否・VM起動は行っていません。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    #[test]
    fn confirmation_cancellation_and_retry_affect_only_future_sample_events() {
        let mut preview = Preview::new();
        let mut ui = State::preview();
        ui.update(preview.backend.clone());
        let row = preview.attempt(sample([203, 0, 113, 30], 8443, 1002, "app-worker"));
        ui.push(row.clone());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        ui.key(key(KeyCode::Char('b')));
        assert!(preview.backend.rules.is_empty());
        ui.key(key(KeyCode::Esc));
        assert!(matches!(ui.key(key(KeyCode::Enter)), Input::None));
        ui.key(key(KeyCode::Char('b')));
        ui.push(sample([192, 0, 2, 10], 443, 1001, "package-fetch"));
        let Input::Command(command) = ui.key(key(KeyCode::Enter)) else {
            panic!("missing confirmed tuple")
        };
        preview.apply(command);
        assert_eq!(row.event.action, ACTION_ALLOWED);
        assert_eq!(preview.attempt(row.clone()).event.action, ACTION_DENIED);
        assert_eq!(
            preview
                .attempt(sample([192, 0, 2, 10], 443, 1001, "other"))
                .event
                .action,
            ACTION_ALLOWED
        );
        preview.apply(Command::Remove(rule_key(row.event.address, 4, 8443, TCP)));
        assert_eq!(preview.attempt(row).event.action, ACTION_ALLOWED);
    }
    #[test]
    fn preview_label_survives_narrow_terminal() {
        for (width, height) in [(120, 30), (80, 24), (40, 10)] {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|f| tui::draw(f, &State::preview(), 0))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(text.contains("DRY-RUN"));
        }
    }
}
