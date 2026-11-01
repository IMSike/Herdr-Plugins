//! Requests to the Herdr API socket: one JSON line out, one JSON line back,
//! one connection per request (the server closes it after answering).

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const SOCKET_ENV: &str = crate::env::SOCKET_PATH;
/// A request Herdr has not answered by then is given up: a plugin never
/// hangs on a busy or stuck server.
const TIMEOUT: Duration = Duration::from_secs(3);

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

fn failure(message: String) -> Error {
    Error {
        code: None,
        message,
    }
}

/// Sends one request and returns its `result`.
pub fn request(method: &str, params: Value) -> Result<Value, Error> {
    let path = std::env::var(SOCKET_ENV).map_err(|_| failure(format!("{SOCKET_ENV} absent")))?;
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
/// `TIMEOUT` (the thread ends with the process, or when Herdr answers).
fn exchange(path: String, line: String) -> io::Result<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let deadline = Instant::now() + TIMEOUT;
    std::thread::spawn(move || {
        let _ = sender.send(connect(&path, deadline).and_then(|stream| talk(stream, &line)));
    });
    receiver
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "pas de réponse")))
}

#[cfg(unix)]
fn connect(path: &str, _deadline: Instant) -> io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(path)
}

/// Several plugin runs at once can find every instance of the pipe busy:
/// try again until the deadline.
#[cfg(windows)]
fn connect(path: &str, deadline: Instant) -> io::Result<std::fs::File> {
    // The named pipe is named after the full socket path.
    const PIPE_BUSY: i32 = 231;
    let pipe = format!(r"\\.\pipe\{path}");
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe)
        {
            Err(e) if e.raw_os_error() == Some(PIPE_BUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}
