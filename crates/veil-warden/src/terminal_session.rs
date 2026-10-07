// SPDX-License-Identifier: MIT
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, IsTerminal};
pub struct Session {
    raw: bool,
    alternate: bool,
}
impl Session {
    pub fn enter() -> io::Result<Self> {
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
    pub fn restore(&mut self) -> io::Result<()> {
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
