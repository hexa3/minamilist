use std::{env, path::Path};

use apex_term::{TerminalError, TerminalSession};

const TERMINAL_LIMIT: usize = 512 * 1024;

pub struct TerminalPane {
    session: Option<TerminalSession>,
    screen: String,
    escape: EscapeState,
}

impl TerminalPane {
    pub fn start(working_directory: &Path, columns: u16, rows: u16) -> Result<Self, TerminalError> {
        let shell = default_shell();
        let session = TerminalSession::spawn(&shell, working_directory, columns, rows)?;
        Ok(Self {
            session: Some(session),
            screen: format!("minamilist terminal · {shell}\n"),
            escape: EscapeState::Normal,
        })
    }

    pub fn send(&mut self, input: &[u8]) -> Result<(), TerminalError> {
        if let Some(session) = self.session.as_mut() {
            session.send(input)?;
        }
        Ok(())
    }

    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), TerminalError> {
        if let Some(session) = self.session.as_ref() {
            session.resize(columns, rows)?;
        }
        Ok(())
    }

    pub fn poll(&mut self) -> Result<bool, TerminalError> {
        let mut changed = false;
        while let Some(chunk) = self.session.as_ref().and_then(TerminalSession::try_read) {
            self.push_output(&chunk);
            changed = true;
        }
        let exited = self
            .session
            .as_mut()
            .is_some_and(|session| session.try_wait().ok().flatten().is_some());
        if exited {
            self.session = None;
            self.screen.push_str("\n[terminal exited]\n");
            changed = true;
        }
        Ok(changed)
    }

    pub fn clear(&mut self) {
        self.screen.clear();
    }

    pub fn screen(&self) -> &str {
        &self.screen
    }

    pub fn is_running(&self) -> bool {
        self.session.is_some()
    }

    fn push_output(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        for character in text.chars() {
            match self.escape {
                EscapeState::Normal => match character {
                    '\u{1b}' => self.escape = EscapeState::Escape,
                    '\r' => {}
                    '\u{8}' => {
                        if !self.screen.ends_with('\n') {
                            self.screen.pop();
                        }
                    }
                    character
                        if character == '\n' || character == '\t' || !character.is_control() =>
                    {
                        self.screen.push(character);
                    }
                    _ => {}
                },
                EscapeState::Escape => {
                    self.escape = match character {
                        '[' => EscapeState::ControlSequence,
                        ']' => EscapeState::OperatingSystemCommand,
                        _ => EscapeState::Normal,
                    };
                }
                EscapeState::ControlSequence => {
                    if ('@'..='~').contains(&character) {
                        if character == 'J' {
                            self.screen.clear();
                        }
                        self.escape = EscapeState::Normal;
                    }
                }
                EscapeState::OperatingSystemCommand => {
                    if character == '\u{7}' {
                        self.escape = EscapeState::Normal;
                    } else if character == '\u{1b}' {
                        self.escape = EscapeState::OperatingSystemEscape;
                    }
                }
                EscapeState::OperatingSystemEscape => {
                    self.escape = if character == '\\' {
                        EscapeState::Normal
                    } else {
                        EscapeState::OperatingSystemCommand
                    };
                }
            }
        }
        if self.screen.len() > TERMINAL_LIMIT {
            let mut drain_to = self.screen.len() - TERMINAL_LIMIT;
            while !self.screen.is_char_boundary(drain_to) {
                drain_to += 1;
            }
            self.screen.drain(..drain_to);
        }
    }
}

#[derive(Clone, Copy)]
enum EscapeState {
    Normal,
    Escape,
    ControlSequence,
    OperatingSystemCommand,
    OperatingSystemEscape,
}

#[cfg(windows)]
fn default_shell() -> String {
    env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
}

#[cfg(not(windows))]
fn default_shell() -> String {
    env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_terminal_control_sequences() {
        let mut pane = TerminalPane {
            session: None,
            screen: String::new(),
            escape: EscapeState::Normal,
        };
        pane.push_output(b"plain\x1b[31m red\x1b[0m\r\nnext");
        assert_eq!(pane.screen(), "plain red\nnext");
    }

    #[cfg(unix)]
    #[test]
    fn interactive_shell_accepts_commands_and_streams_output() {
        use std::{
            thread,
            time::{Duration, Instant},
        };

        let mut pane = TerminalPane::start(Path::new("."), 80, 24).unwrap();
        pane.send(b"printf 'minamilist-terminal-ok\\n'\r").unwrap();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            pane.poll().unwrap();
            if pane.screen().contains("minamilist-terminal-ok\n") {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("terminal output was not captured: {}", pane.screen());
    }
}
