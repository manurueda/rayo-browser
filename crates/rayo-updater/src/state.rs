use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;

/// State directory for rayo update files (~/.rayo/).
pub struct StateDir {
    path: PathBuf,
}

impl StateDir {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn default_path() -> PathBuf {
        dirs_path()
    }

    fn last_check_path(&self) -> PathBuf {
        self.path.join("last-check")
    }

    fn marker_path(&self) -> PathBuf {
        self.path.join("update-marker.json")
    }

    fn lock_path(&self) -> PathBuf {
        self.path.join("update.lock")
    }

    // --- Rate limiting ---

    /// Returns seconds since last check, or u64::MAX if never checked.
    pub fn seconds_since_last_check(&self) -> u64 {
        let path = self.last_check_path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return u64::MAX,
        };
        let last: u64 = match content.trim().parse() {
            Ok(t) => t,
            Err(_) => return u64::MAX,
        };
        let now = now_secs();
        // Handle clock skew: if last_check is in the future, treat as expired
        now.saturating_sub(last)
    }

    pub fn write_last_check(&self) -> io::Result<()> {
        fs::write(self.last_check_path(), now_secs().to_string())
    }

    // --- Update marker ---

    pub fn read_marker(&self) -> Option<UpdateMarker> {
        let content = fs::read_to_string(self.marker_path()).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn write_marker(&self, marker: &UpdateMarker) -> io::Result<()> {
        let json = serde_json::to_string_pretty(marker).map_err(io::Error::other)?;
        fs::write(self.marker_path(), json)
    }

    pub fn clear_marker(&self) -> io::Result<()> {
        match fs::remove_file(self.marker_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    // --- File lock ---

    /// Try to acquire the update lock. Returns None if already held.
    pub fn try_lock(&self) -> Option<UpdateLock> {
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.lock_path())
            .ok()?;

        match file.try_lock_exclusive() {
            Ok(()) => Some(UpdateLock { _file: file }),
            Err(_) => None,
        }
    }
}

/// Held while an update is in progress. Lock is released on drop.
pub struct UpdateLock {
    _file: fs::File,
}

/// Persisted after a binary replacement, read on next startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMarker {
    pub from_version: String,
    pub to_version: String,
    pub updated_at: u64,
    #[serde(default)]
    pub crash_count: u32,
}

impl UpdateMarker {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from_version: from.to_string(),
            to_version: to.to_string(),
            updated_at: now_secs(),
            crash_count: 0,
        }
    }

    /// A marker is stale if it was written but never cleared (startup crashed).
    /// We detect this by checking if the marker's to_version matches current.
    pub fn is_stale(&self, current_version: &str) -> bool {
        // If we updated to version X but we're still running version X,
        // the startup succeeded. If we're running something else, it crashed
        // before clearing the marker.
        self.to_version != current_version
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".rayo")
}
