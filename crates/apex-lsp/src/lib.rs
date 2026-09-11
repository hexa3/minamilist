//! Language Server Protocol subprocess client used by Apex.

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

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

type Response = Result<Value, String>;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("failed to start language server: {0}")]
    Spawn(#[source] io::Error),
    #[error("language server transport failed: {0}")]
    Io(#[from] io::Error),
    #[error("language server response timed out")]
    Timeout,
    #[error("language server returned an error: {0}")]
    Server(String),
    #[error("invalid workspace edit: {0}")]
    InvalidEdit(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextEdit {
    pub range: Range,
    #[serde(rename = "newText")]
    pub new_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceEdit {
    #[serde(default)]
    pub changes: HashMap<String, Vec<TextEdit>>,
}

impl WorkspaceEdit {
    /// Reject overlapping server edits before they reach the filesystem.
    pub fn validate(&self) -> Result<(), LspError> {
        for (uri, edits) in &self.changes {
            let mut ordered: Vec<_> = edits.iter().collect();
            ordered
                .sort_unstable_by_key(|edit| (edit.range.start.line, edit.range.start.character));
            for pair in ordered.windows(2) {
                let left_end = (&pair[0].range.end.line, &pair[0].range.end.character);
                let right_start = (&pair[1].range.start.line, &pair[1].range.start.character);
                if left_end > right_start {
                    return Err(LspError::InvalidEdit(format!(
                        "overlapping edits for {uri}"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

pub struct LspClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, Sender<Response>>>>,
    notifications: Receiver<Notification>,
}

impl LspClient {
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self, LspError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(LspError::Spawn)?;
        let stdin = Arc::new(Mutex::new(
            child.stdin.take().expect("piped language server stdin"),
        ));
        let stdout = child.stdout.take().expect("piped language server stdout");
        let pending: Arc<Mutex<HashMap<u64, Sender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        let (notification_tx, notifications) = mpsc::channel();
        thread::Builder::new()
            .name("apex-lsp-reader".into())
            .spawn(move || read_loop(stdout, reader_pending, notification_tx))
            .map_err(LspError::Spawn)?;
        Ok(Self {
            child,
            stdin,
            next_id: AtomicU64::new(1),
            pending,
            notifications,
        })
    }

    pub fn initialize(&self, root_uri: &str) -> Result<Value, LspError> {
        let result = self.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "completion": { "completionItem": { "snippetSupport": true } },
                        "hover": {},
                        "definition": {},
                        "rename": { "prepareSupport": true },
                        "publishDiagnostics": {}
                    },
                    "workspace": { "workspaceEdit": { "documentChanges": true } }
                },
                "clientInfo": { "name": "Apex", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;
        self.notify("initialized", json!({}))?;
        Ok(result)
    }

    pub fn did_open(
        &self,
        uri: &str,
        language_id: &str,
        version: i32,
        text: &str,
    ) -> Result<(), LspError> {
        self.notify("textDocument/didOpen", json!({
            "textDocument": { "uri": uri, "languageId": language_id, "version": version, "text": text }
        }))
    }

    pub fn did_change(&self, uri: &str, version: i32, text: &str) -> Result<(), LspError> {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
    }

    pub fn hover(&self, uri: &str, position: Position) -> Result<Value, LspError> {
        self.text_document_request("textDocument/hover", uri, position)
    }

    pub fn completion(&self, uri: &str, position: Position) -> Result<Value, LspError> {
        self.text_document_request("textDocument/completion", uri, position)
    }

    pub fn definition(&self, uri: &str, position: Position) -> Result<Value, LspError> {
        self.text_document_request("textDocument/definition", uri, position)
    }

    pub fn rename(
        &self,
        uri: &str,
        position: Position,
        new_name: &str,
    ) -> Result<WorkspaceEdit, LspError> {
        let value = self.request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri }, "position": position, "newName": new_name
            }),
        )?;
        let edit: WorkspaceEdit = serde_json::from_value(value)
            .map_err(|error| LspError::InvalidEdit(error.to_string()))?;
        edit.validate()?;
        Ok(edit)
    }

    pub fn request(&self, method: &str, params: Value) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .expect("LSP pending lock poisoned")
            .insert(id, sender);
        self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        receiver
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| LspError::Timeout)?
            .map_err(LspError::Server)
    }

    pub fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    pub fn try_notification(&self) -> Option<Notification> {
        self.notifications.try_recv().ok()
    }

    fn text_document_request(
        &self,
        method: &str,
        uri: &str,
        position: Position,
    ) -> Result<Value, LspError> {
        self.request(
            method,
            json!({ "textDocument": { "uri": uri }, "position": position }),
        )
    }

    fn write(&self, message: &Value) -> Result<(), LspError> {
        let mut stdin = self.stdin.lock().expect("LSP stdin lock poisoned");
        write_message(&mut *stdin, message)?;
        Ok(())
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.notify("exit", Value::Null);
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
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "language server closed stdout",
            ));
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            );
        }
    }
    let length = content_length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_loop(
    stdout: impl Read,
    pending: Arc<Mutex<HashMap<u64, Sender<Response>>>>,
    notifications: Sender<Notification>,
) {
    let mut reader = BufReader::new(stdout);
    while let Ok(message) = read_message(&mut reader) {
        if let Some(id) = message.get("id").and_then(Value::as_u64) {
            if let Some(sender) = pending
                .lock()
                .expect("LSP pending lock poisoned")
                .remove(&id)
            {
                let response = message.get("error").map_or_else(
                    || Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                    |error| Err(error.to_string()),
                );
                let _ = sender.send(response);
            }
        } else if let Some(method) = message.get("method").and_then(Value::as_str) {
            let _ = notifications.send(Notification {
                method: method.to_owned(),
                params: message.get("params").cloned().unwrap_or(Value::Null),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_framing_round_trips() {
        let message = json!({ "jsonrpc": "2.0", "id": 7, "result": { "ok": true } });
        let mut wire = Vec::new();
        write_message(&mut wire, &message).unwrap();
        assert_eq!(read_message(&mut wire.as_slice()).unwrap(), message);
    }

    #[test]
    fn rejects_overlapping_rename_edits() {
        let edit = WorkspaceEdit {
            changes: HashMap::from([(
                "file:///main.rs".into(),
                vec![
                    TextEdit {
                        range: Range {
                            start: Position {
                                line: 1,
                                character: 0,
                            },
                            end: Position {
                                line: 1,
                                character: 4,
                            },
                        },
                        new_text: "one".into(),
                    },
                    TextEdit {
                        range: Range {
                            start: Position {
                                line: 1,
                                character: 2,
                            },
                            end: Position {
                                line: 1,
                                character: 6,
                            },
                        },
                        new_text: "two".into(),
                    },
                ],
            )]),
        };
        assert!(matches!(edit.validate(), Err(LspError::InvalidEdit(_))));
    }
}
