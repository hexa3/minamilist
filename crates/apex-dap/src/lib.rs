//! Debug Adapter Protocol client with a crash-isolated adapter subprocess.

use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use serde_json::{Value, json};
use thiserror::Error;

type Response = Result<Value, String>;

#[derive(Debug, Error)]
pub enum DapError {
    #[error("failed to start debug adapter: {0}")]
    Spawn(#[source] io::Error),
    #[error("debug adapter transport failed: {0}")]
    Io(#[from] io::Error),
    #[error("debug adapter response timed out")]
    Timeout,
    #[error("debug adapter request failed: {0}")]
    Adapter(String),
}

#[derive(Clone, Debug)]
pub struct DebugEvent {
    pub name: String,
    pub body: Value,
}

pub struct DapClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    sequence: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, Sender<Response>>>>,
    events: Receiver<DebugEvent>,
}

impl DapClient {
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self, DapError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(DapError::Spawn)?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().expect("piped adapter stdin")));
        let stdout = child.stdout.take().expect("piped adapter stdout");
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        let (event_sender, events) = mpsc::channel();
        thread::Builder::new()
            .name("apex-dap-reader".into())
            .spawn(move || read_loop(stdout, reader_pending, event_sender))
            .map_err(DapError::Spawn)?;
        Ok(Self {
            child,
            stdin,
            sequence: AtomicU64::new(1),
            pending,
            events,
        })
    }

    pub fn initialize(&self, adapter_id: &str) -> Result<Value, DapError> {
        self.request(
            "initialize",
            json!({
                "clientID": "apex", "clientName": "Apex", "adapterID": adapter_id,
                "pathFormat": "path", "linesStartAt1": true, "columnsStartAt1": true,
                "supportsVariableType": true, "supportsRunInTerminalRequest": true
            }),
        )
    }

    pub fn launch(&self, configuration: Value) -> Result<Value, DapError> {
        self.request("launch", configuration)
    }

    pub fn attach(&self, configuration: Value) -> Result<Value, DapError> {
        self.request("attach", configuration)
    }

    pub fn set_breakpoints(&self, source_path: &str, lines: &[u32]) -> Result<Value, DapError> {
        self.request(
            "setBreakpoints",
            json!({
                "source": { "path": source_path },
                "breakpoints": lines.iter().map(|line| json!({ "line": line })).collect::<Vec<_>>()
            }),
        )
    }

    pub fn continue_thread(&self, thread_id: u64) -> Result<Value, DapError> {
        self.request("continue", json!({ "threadId": thread_id }))
    }

    pub fn next(&self, thread_id: u64) -> Result<Value, DapError> {
        self.request("next", json!({ "threadId": thread_id }))
    }

    pub fn step_in(&self, thread_id: u64) -> Result<Value, DapError> {
        self.request("stepIn", json!({ "threadId": thread_id }))
    }

    pub fn stack_trace(&self, thread_id: u64) -> Result<Value, DapError> {
        self.request("stackTrace", json!({ "threadId": thread_id }))
    }

    pub fn variables(&self, variables_reference: u64) -> Result<Value, DapError> {
        self.request(
            "variables",
            json!({ "variablesReference": variables_reference }),
        )
    }

    pub fn try_event(&self) -> Option<DebugEvent> {
        self.events.try_recv().ok()
    }

    pub fn request(&self, command: &str, arguments: Value) -> Result<Value, DapError> {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .expect("DAP pending lock poisoned")
            .insert(sequence, sender);
        self.write(&json!({
            "seq": sequence, "type": "request", "command": command, "arguments": arguments
        }))?;
        receiver
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| DapError::Timeout)?
            .map_err(DapError::Adapter)
    }

    fn write(&self, message: &Value) -> Result<(), DapError> {
        let mut stdin = self.stdin.lock().expect("DAP stdin lock poisoned");
        write_message(&mut *stdin, message)?;
        Ok(())
    }
}

impl Drop for DapClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Value> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "debug adapter closed stdout",
            ));
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            );
        }
    }
    let mut body = vec![
        0;
        length.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length")
        })?
    ];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_loop(
    stdout: impl Read,
    pending: Arc<Mutex<HashMap<u64, Sender<Response>>>>,
    events: Sender<DebugEvent>,
) {
    let mut reader = BufReader::new(stdout);
    while let Ok(message) = read_message(&mut reader) {
        match message.get("type").and_then(Value::as_str) {
            Some("response") => {
                let Some(request_seq) = message.get("request_seq").and_then(Value::as_u64) else {
                    continue;
                };
                let Some(sender) = pending
                    .lock()
                    .expect("DAP pending lock poisoned")
                    .remove(&request_seq)
                else {
                    continue;
                };
                let success = message
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let response = if success {
                    Ok(message.get("body").cloned().unwrap_or(Value::Null))
                } else {
                    Err(message
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown adapter error")
                        .to_owned())
                };
                let _ = sender.send(response);
            }
            Some("event") => {
                let _ = events.send(DebugEvent {
                    name: message
                        .get("event")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_owned(),
                    body: message.get("body").cloned().unwrap_or(Value::Null),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dap_framing_round_trips() {
        let message = json!({ "seq": 1, "type": "event", "event": "stopped" });
        let mut bytes = Vec::new();
        write_message(&mut bytes, &message).unwrap();
        assert_eq!(read_message(&mut bytes.as_slice()).unwrap(), message);
    }
}
