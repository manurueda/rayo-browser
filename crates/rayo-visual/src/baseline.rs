use crate::error::VisualError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

/// Metadata stored alongside each baseline image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMeta {
    pub width: u32,
    pub height: u32,
    pub created_at: u64, // Unix timestamp
}

/// Info about a stored baseline.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub created_at: u64,
    pub size_bytes: u64,
}

/// Manages baseline images on disk.
pub struct BaselineManager {
    baselines_dir: PathBuf,
}

impl BaselineManager {
    pub fn new(baselines_dir: PathBuf) -> Self {
        Self { baselines_dir }
    }

    /// Save a PNG image as a baseline with the given name.
    pub fn save(
        &self,
        name: &str,
        png_bytes: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), VisualError> {
        validate_name(name)?;
        fs::create_dir_all(&self.baselines_dir)?;

        let png_path = self.png_path(name);
        let meta_path = self.meta_path(name);

        fs::write(&png_path, png_bytes)?;

        let meta = BaselineMeta {
            width,
            height,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(&meta_path, meta_json)?;

        Ok(())
    }

    /// Load a baseline PNG by name.
    pub fn load(&self, name: &str) -> Result<Vec<u8>, VisualError> {
        validate_name(name)?;
        let path = self.png_path(name);
        if !path.exists() {
            return Err(VisualError::BaselineNotFound {
                name: name.to_string(),
            });
        }
        Ok(fs::read(&path)?)
    }

    /// Load baseline metadata.
    pub fn load_meta(&self, name: &str) -> Result<BaselineMeta, VisualError> {
        validate_name(name)?;
        let path = self.meta_path(name);
        if !path.exists() {
            return Err(VisualError::BaselineNotFound {
                name: name.to_string(),
            });
        }
        let json = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// List all baselines.
    pub fn list(&self) -> Result<Vec<BaselineInfo>, VisualError> {
        if !self.baselines_dir.exists() {
            return Ok(Vec::new());
        }

        let mut infos = Vec::new();
        for entry in fs::read_dir(&self.baselines_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "png") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);

                let meta = self.load_meta(&name).ok();
                infos.push(BaselineInfo {
                    name,
                    width: meta.as_ref().map(|m| m.width).unwrap_or(0),
                    height: meta.as_ref().map(|m| m.height).unwrap_or(0),
                    created_at: meta.as_ref().map(|m| m.created_at).unwrap_or(0),
                    size_bytes,
                });
            }
        }

        infos.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(infos)
    }

    /// Delete a baseline and its metadata.
    pub fn delete(&self, name: &str) -> Result<(), VisualError> {
        validate_name(name)?;
        let png_path = self.png_path(name);
        let meta_path = self.meta_path(name);

        if !png_path.exists() {
            return Err(VisualError::BaselineNotFound {
                name: name.to_string(),
            });
        }

        fs::remove_file(&png_path)?;
        let _ = fs::remove_file(&meta_path); // meta might not exist
        Ok(())
    }

    /// Check if a baseline exists.
    pub fn exists(&self, name: &str) -> bool {
        validate_name(name).is_ok() && self.png_path(name).exists()
    }

    fn png_path(&self, name: &str) -> PathBuf {
        self.baselines_dir.join(format!("{name}.png"))
    }

    fn meta_path(&self, name: &str) -> PathBuf {
        self.baselines_dir.join(format!("{name}.meta.json"))
    }
}

/// Validate baseline name: only [a-zA-Z0-9_-], no path separators, no `..`.
fn validate_name(name: &str) -> Result<(), VisualError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(VisualError::InvalidName {
            name: name.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("rayo-visual-test-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        let data = b"fake-png-data";
        mgr.save("test-baseline", data, 100, 50).unwrap();
        let loaded = mgr.load("test-baseline").unwrap();
        assert_eq!(loaded, data);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn path_traversal_rejected() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        assert!(matches!(
            mgr.save("../../evil", b"data", 10, 10),
            Err(VisualError::InvalidName { .. })
        ));
        assert!(matches!(
            mgr.save("with/slash", b"data", 10, 10),
            Err(VisualError::InvalidName { .. })
        ));
        assert!(matches!(
            mgr.save("", b"data", 10, 10),
            Err(VisualError::InvalidName { .. })
        ));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn valid_names_accepted() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        assert!(mgr.save("login-page_v2", b"data", 10, 10).is_ok());
        assert!(mgr.save("UPPERCASE", b"data", 10, 10).is_ok());
        assert!(mgr.save("test123", b"data", 10, 10).is_ok());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_nonexistent_returns_not_found() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        assert!(matches!(
            mgr.load("nonexistent"),
            Err(VisualError::BaselineNotFound { .. })
        ));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_removes_files() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        mgr.save("to-delete", b"data", 10, 10).unwrap();
        assert!(mgr.exists("to-delete"));
        mgr.delete("to-delete").unwrap();
        assert!(!mgr.exists("to-delete"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_returns_all_baselines() {
        let dir = temp_dir();
        let mgr = BaselineManager::new(dir.clone());
        mgr.save("alpha", b"a", 10, 10).unwrap();
        mgr.save("beta", b"b", 20, 20).unwrap();
        mgr.save("gamma", b"c", 30, 30).unwrap();
        let list = mgr.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
        assert_eq!(list[2].name, "gamma");
        fs::remove_dir_all(&dir).ok();
    }
}
