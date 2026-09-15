//! Cross-platform pseudo-terminal sessions for minamilist's integrated terminal.

use std::{
    io::{self, Read, Write},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread,
};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("terminal operation failed: {0}")]
    Pty(#[from] anyhow::Error),
    #[error("terminal I/O failed: {0}")]
    Io(#[from] io::Error),
}

pub struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<Vec<u8>>,
}

impl TerminalSession {
    pub fn spawn(
        shell: &str,
        working_directory: impl AsRef<Path>,
        columns: u16,
        rows: u16,
    ) -> Result<Self, TerminalError> {
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut command = CommandBuilder::new(shell);
        command.cwd(working_directory.as_ref().as_os_str());
        let child = pair.slave.spawn_command(command)?;
        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (sender, output) = mpsc::channel();
        thread::Builder::new()
            .name("apex-terminal-reader".into())
            .spawn(move || {
                let mut buffer = [0; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            if sender.send(buffer[..count].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })?;
        Ok(Self {
            master: pair.master,
            writer,
            child,
            output,
        })
    }

    pub fn send(&mut self, input: &[u8]) -> Result<(), TerminalError> {
        self.writer.write_all(input)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, columns: u16, rows: u16) -> Result<(), TerminalError> {
        self.master.resize(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn try_read(&self) -> Option<Vec<u8>> {
        self.output.try_recv().ok()
    }

    pub fn try_wait(&mut self) -> Result<Option<portable_pty::ExitStatus>, TerminalError> {
        Ok(self.child.try_wait()?)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn real_pty_captures_shell_output() {
        let mut terminal = TerminalSession::spawn("/bin/sh", ".", 80, 24).unwrap();
        terminal.send(b"printf apex-terminal\\n").unwrap();
        let started = Instant::now();
        let mut output = Vec::new();
        while started.elapsed() < Duration::from_secs(2) {
            if let Some(chunk) = terminal.try_read() {
                output.extend(chunk);
            }
            if String::from_utf8_lossy(&output).contains("apex-terminal") {
                return;
            }
            thread::yield_now();
        }
        panic!(
            "PTY did not return expected output: {}",
            String::from_utf8_lossy(&output)
        );
    }
}
