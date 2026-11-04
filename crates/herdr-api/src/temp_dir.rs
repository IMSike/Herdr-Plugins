//! What the plugins' tests share: built for herdr-api's own tests and with
//! the `test-support` feature, which only the plugins' `[dev-dependencies]`
//! turn on.

use std::io;
use std::path::{Path, PathBuf};

/// A folder of its own in the system's temporary folder, for tests: removed
/// with all it holds when dropped, by a test that fails too.
pub struct TempDir(PathBuf);

impl TempDir {
    /// `herdr-<name>-<pid>`, empty.
    pub fn new(name: &str) -> io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("herdr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
