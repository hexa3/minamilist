use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 512 * 1024;

#[derive(Debug)]
pub struct PythonResult {
    pub interpreter: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
    pub stopped: bool,
}

pub struct PythonRun {
    cancel: Arc<AtomicBool>,
    receiver: Receiver<PythonResult>,
}

impl PythonRun {
    pub fn start(path: &Path) -> io::Result<Self> {
        let (mut child, interpreter) = spawn_interpreter(path)?;
        let stdout = child.stdout.take().expect("Python stdout is piped");
        let stderr = child.stderr.take().expect("Python stderr is piped");
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("minamilist-python-runner".into())
            .spawn(move || {
                let started = Instant::now();
                let stdout_reader = thread::spawn(move || read_limited(stdout));
                let stderr_reader = thread::spawn(move || read_limited(stderr));
                let mut stopped = false;
                let status = loop {
                    if thread_cancel.load(Ordering::Relaxed) {
                        stopped = true;
                        let _ = child.kill();
                    }
                    match child.try_wait() {
                        Ok(Some(status)) => break Some(status),
                        Ok(None) => thread::sleep(Duration::from_millis(25)),
                        Err(_) => break None,
                    }
                };
                let stdout = stdout_reader.join().unwrap_or_default();
                let stderr = stderr_reader.join().unwrap_or_default();
                let _ = sender.send(PythonResult {
                    interpreter,
                    stdout,
                    stderr,
                    exit_code: status.and_then(|status| status.code()),
                    elapsed: started.elapsed(),
                    stopped,
                });
            })?;

        Ok(Self { cancel, receiver })
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn try_finish(&self) -> Option<PythonResult> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for PythonRun {
    fn drop(&mut self) {
        self.stop();
        for _ in 0..40 {
            match self.receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

fn spawn_interpreter(path: &Path) -> io::Result<(Child, String)> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file = path.file_name().unwrap_or(path.as_os_str());
    let mut last_error = None;
    for interpreter in interpreter_candidates() {
        match Command::new(interpreter)
            .arg(file)
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => return Ok((child, (*interpreter).into())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no Python interpreter was found")
    }))
}

#[cfg(windows)]
fn interpreter_candidates() -> &'static [&'static str] {
    &["python", "python3"]
}

#[cfg(not(windows))]
fn interpreter_candidates() -> &'static [&'static str] {
    &["python3", "python"]
}

fn read_limited(mut reader: impl Read) -> String {
    let mut bytes = Vec::new();
    let _ = reader
        .by_ref()
        .take((OUTPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes);
    let truncated = bytes.len() > OUTPUT_LIMIT;
    bytes.truncate(OUTPUT_LIMIT);
    let mut output = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        output.push_str("\n[output truncated at 512 KiB]\n");
    }
    output
}

pub fn canonical_python_path(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn output_capture_is_bounded() {
        let input = vec![b'x'; OUTPUT_LIMIT + 20];
        let output = read_limited(input.as_slice());
        assert!(output.ends_with("[output truncated at 512 KiB]\n"));
        assert!(output.len() < OUTPUT_LIMIT + 100);
    }

    #[test]
    fn executes_a_python_file_and_captures_both_streams() {
        let path = std::env::temp_dir().join(format!(
            "minamilist-python-{}-{}.py",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            "import sys\nprint('hello from python')\nprint('diagnostic', file=sys.stderr)\n",
        )
        .unwrap();
        let run = match PythonRun::start(&path) {
            Ok(run) => run,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::remove_file(path).unwrap();
                return;
            }
            Err(error) => panic!("could not launch Python test: {error}"),
        };
        let started = Instant::now();
        let result = loop {
            if let Some(result) = run.try_finish() {
                break result;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello from python"));
        assert!(result.stderr.contains("diagnostic"));
        fs::remove_file(path).unwrap();
    }
}
