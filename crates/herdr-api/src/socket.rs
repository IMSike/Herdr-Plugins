//! Requests to the Herdr API socket: one JSON line out, one JSON line back,
//! one connection per request (the server closes it after answering).

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// A request Herdr has not answered by then is given up: a plugin never
/// hangs on a busy or stuck server.
const TIMEOUT: Duration = Duration::from_secs(3);
/// Requests still waiting for Herdr, each on its thread: past this many, a
/// stuck Herdr is not sent more (their threads would pile up in a
/// long-running pane).
const MAX_WAITING: usize = 4;
static WAITING: AtomicUsize = AtomicUsize::new(0);
/// When the run must be over, if it has a limit: no request lasts past it.
static DEADLINE: OnceLock<Instant> = OnceLock::new();

/// Requests end by `deadline` at the latest (the first one set holds): a
/// run with a time limit of its own is not held up past it.
pub fn set_deadline(deadline: Instant) {
    let _ = DEADLINE.set(deadline);
}

/// Why a request failed.
#[derive(Debug)]
pub struct Error {
    /// The API's error code (`ui_busy`, `not_found`…), when Herdr answered one.
    pub code: Option<String>,
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.message
    }
}

/// A failure before any request: a missing variable, for instance.
impl From<String> for Error {
    fn from(message: String) -> Self {
        failure(message)
    }
}

fn failure(message: String) -> Error {
    Error {
        code: None,
        message,
    }
}

/// Sends one request and returns its `result`.
pub fn request(method: &str, params: Value) -> Result<Value, Error> {
    let path = crate::var(crate::env::SOCKET_PATH).map_err(failure)?;
    let line = json!({ "id": method, "method": method, "params": params }).to_string();
    let answer = exchange(path, line).map_err(|e| failure(format!("{method} : {e}")))?;
    let mut value: Value = serde_json::from_str(&answer)
        .map_err(|e| failure(format!("{method} : réponse illisible : {e}")))?;
    if let Some(error) = value.get("error") {
        return Err(Error {
            code: error["code"].as_str().map(str::to_string),
            message: format!("{method} : {error}"),
        });
    }
    Ok(value
        .get_mut("result")
        .map(Value::take)
        .unwrap_or(Value::Null))
}

/// Sends one request and reads one field of its `result`.
pub fn request_field<T: DeserializeOwned>(
    method: &str,
    params: Value,
    field: &str,
) -> Result<T, Error> {
    let mut result = request(method, params)?;
    let value = result.get_mut(field).map(Value::take).unwrap_or_default();
    serde_json::from_value(value)
        .map_err(|e| failure(format!("{method} : {field} illisible : {e}")))
}

fn talk<S: io::Read + Write>(mut stream: S, line: &str) -> io::Result<String> {
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer)?;
    Ok(answer)
}

/// Neither a Unix socket's connect nor a named pipe opened as a file has a
/// timeout: the whole exchange runs on a thread of its own, given up after
/// `TIMEOUT` or at the run's deadline. The thread then ends too: on its
/// socket's timeouts on Unix, by its read or write being cancelled on Windows.
fn exchange(path: String, line: String) -> io::Result<String> {
    let mut deadline = Instant::now() + TIMEOUT;
    if let Some(&limit) = DEADLINE.get() {
        deadline = deadline.min(limit);
    }
    if WAITING.fetch_add(1, Ordering::SeqCst) >= MAX_WAITING {
        WAITING.fetch_sub(1, Ordering::SeqCst);
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Herdr ne répond plus aux requêtes précédentes",
        ));
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let pending = std::sync::Arc::new(Pending::default());
    let talking = pending.clone();
    std::thread::spawn(move || {
        let answer = connect(&path, deadline).and_then(|stream| talking.talk(&stream, &line));
        WAITING.fetch_sub(1, Ordering::SeqCst);
        let _ = sender.send(answer);
    });
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_else(|_| {
            pending.cancel();
            Err(io::Error::new(io::ErrorKind::TimedOut, "pas de réponse"))
        })
}

/// The exchange of a thread, which the thread that waits for it can end.
#[derive(Default)]
struct Pending {
    /// The pipe's handle while the exchange runs on it, and whether it was
    /// cancelled.
    #[cfg(windows)]
    state: std::sync::Mutex<(Option<usize>, bool)>,
}

impl Pending {
    /// The socket's own timeouts end the exchange.
    #[cfg(unix)]
    fn talk(&self, stream: &std::os::unix::net::UnixStream, line: &str) -> io::Result<String> {
        talk(stream, line)
    }

    #[cfg(unix)]
    fn cancel(&self) {}

    /// The handle is cleared before the pipe closes: `cancel` never acts on
    /// a closed handle, which Windows may have given to another file.
    #[cfg(windows)]
    fn talk(&self, pipe: &std::fs::File, line: &str) -> io::Result<String> {
        use std::os::windows::io::AsRawHandle;
        let state = || self.state.lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut state = state();
            if state.1 {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "pas de réponse"));
            }
            state.0 = Some(pipe.as_raw_handle() as usize);
        }
        let answer = talk(pipe, line);
        state().0 = None;
        answer
    }

    /// Cancels the read or write the exchange is blocked in; one not started
    /// yet does not start.
    #[cfg(windows)]
    fn cancel(&self) {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CancelIoEx(file: *mut std::ffi::c_void, overlapped: *mut std::ffi::c_void) -> i32;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.1 = true;
        if let Some(handle) = state.0 {
            // SAFETY: the handle is the pipe's, still open: `talk` clears it,
            // under this lock, before the pipe closes. A null OVERLAPPED
            // cancels every operation on the handle, synchronous ones included.
            unsafe { CancelIoEx(handle as *mut std::ffi::c_void, std::ptr::null_mut()) };
        }
    }
}

/// Once connected, the socket's own timeouts end the thread too when Herdr
/// never answers. The connect itself has none: it blocks while Herdr's
/// backlog is full, and its thread ends with the process.
#[cfg(unix)]
fn connect(path: &str, deadline: Instant) -> io::Result<std::os::unix::net::UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    let left = deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_millis(1));
    stream.set_read_timeout(Some(left))?;
    stream.set_write_timeout(Some(left))?;
    Ok(stream)
}

/// Several plugin runs at once can find every instance of the pipe busy:
/// try again until the deadline.
#[cfg(windows)]
fn connect(path: &str, deadline: Instant) -> io::Result<std::fs::File> {
    // The named pipe is named after the full socket path.
    use std::os::windows::fs::OpenOptionsExt;
    const PIPE_BUSY: i32 = 231;
    // The server may identify the plugin, never act as it: a process that
    // took the pipe's name before Herdr cannot use the plugin's rights.
    const SECURITY_IDENTIFICATION: u32 = 1 << 16;
    let pipe = format!(r"\\.\pipe\{path}");
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(&pipe)
        {
            Err(e) if e.raw_os_error() == Some(PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}
