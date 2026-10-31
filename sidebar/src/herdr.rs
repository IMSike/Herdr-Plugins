//! Requests to the Herdr API socket: one JSON line out, one JSON line back,
//! one connection per request (the server closes it after answering).

use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use serde_json::{Value, json};

const SOCKET_ENV: &str = "HERDR_SOCKET_PATH";

/// Sends one request and returns its `result`, or the API error as text.
pub fn request(method: &str, params: Value) -> Result<Value, String> {
    let path = std::env::var(SOCKET_ENV).map_err(|_| format!("{SOCKET_ENV} absent"))?;
    let line = json!({ "id": method, "method": method, "params": params }).to_string();
    let answer = exchange(&path, &line).map_err(|e| format!("{method}: {e}"))?;
    let mut value: Value =
        serde_json::from_str(&answer).map_err(|e| format!("{method}: réponse illisible: {e}"))?;
    if let Some(error) = value.get("error") {
        return Err(format!("{method}: {error}"));
    }
    Ok(value
        .get_mut("result")
        .map(Value::take)
        .unwrap_or(Value::Null))
}

fn exchange(path: &str, line: &str) -> io::Result<String> {
    let mut stream = connect(path)?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer)?;
    Ok(answer)
}

#[cfg(windows)]
fn connect(path: &str) -> io::Result<std::fs::File> {
    // The named pipe is named after the full socket path.
    const PIPE_BUSY: i32 = 231;
    let pipe = format!(r"\\.\pipe\{path}");
    let mut attempts = 0;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe)
        {
            Err(e) if e.raw_os_error() == Some(PIPE_BUSY) && attempts < 20 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            other => return other,
        }
    }
}

#[cfg(unix)]
fn connect(path: &str) -> io::Result<std::os::unix::net::UnixStream> {
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    Ok(stream)
}
