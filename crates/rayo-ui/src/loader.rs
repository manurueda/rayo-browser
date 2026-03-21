//! Load and parse test YAML files from .rayo/tests/

use crate::error::TestError;
use crate::types::{RayoConfig, TestSuite};
use std::path::{Path, PathBuf};

/// Discovered test file with its parsed suite.
#[derive(Debug)]
pub struct TestFile {
    pub path: PathBuf,
    pub suite: TestSuite,
}

/// Load all test suites from a directory.
pub fn load_suites(tests_dir: &Path) -> Result<Vec<TestFile>, TestError> {
    let pattern = tests_dir.join("*.test.yaml");
    let pattern_str = pattern.to_string_lossy();

    let mut files = Vec::new();
    for entry in glob::glob(&pattern_str).map_err(|e| TestError::Other(e.to_string()))? {
        let path = entry.map_err(|e| TestError::Io(e.into_error()))?;
        let suite = load_suite(&path)?;
        files.push(TestFile { path, suite });
    }

    if files.is_empty() {
        return Err(TestError::NoTestFiles {
            path: tests_dir.display().to_string(),
        });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Load a single test suite from a YAML file.
pub fn load_suite(path: &Path) -> Result<TestSuite, TestError> {
    let content = std::fs::read_to_string(path)?;
    serde_yaml::from_str(&content).map_err(|source| TestError::YamlParse {
        path: path.display().to_string(),
        source,
    })
}

/// Load project config from `.rayo/config.yaml` (returns default if missing).
pub fn load_config(rayo_dir: &Path) -> RayoConfig {
    let path = rayo_dir.join("config.yaml");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_yaml::from_str(&content).unwrap_or_default(),
        Err(_) => RayoConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_suite() {
        let yaml = r#"
name: Minimal Test
steps:
  - name: Go to example
    navigate: https://example.com
    assert:
      - text_contains: Example Domain
"#;
        let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(suite.name, "Minimal Test");
        assert_eq!(suite.steps.len(), 1);
        assert!(suite.steps[0].navigate.is_some());
        assert!(suite.steps[0].assert.is_some());
    }

    #[test]
    fn parse_full_suite() {
        let yaml = r#"
name: Login Flow
viewport:
  width: 1920
  height: 1080
setup:
  - navigate: https://app.example.com
steps:
  - name: Fill login form
    batch:
      - { action: type, selector: "input[name='email']", value: "test@example.com" }
      - { action: type, selector: "input[name='password']", value: "secret" }
      - { action: click, selector: "button[type='submit']" }
  - name: Verify dashboard
    wait:
      selector: ".dashboard"
      timeout_ms: 5000
    assert:
      - page_map_contains:
          text: Welcome
      - screenshot:
          name: dashboard
          threshold: 0.02
"#;
        let suite: TestSuite = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(suite.name, "Login Flow");
        assert_eq!(suite.viewport.as_ref().unwrap().width, 1920);
        assert_eq!(suite.setup.len(), 1);
        assert_eq!(suite.steps.len(), 2);
        assert!(suite.steps[0].batch.is_some());
        assert!(suite.steps[1].wait.is_some());
        let assertions = suite.steps[1].assert.as_ref().unwrap();
        assert_eq!(assertions.len(), 2);
    }

    #[test]
    fn parse_config_with_base_url() {
        let yaml = "base_url: http://localhost:3000";
        let config: RayoConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("http://localhost:3000"));
    }

    #[test]
    fn parse_empty_config() {
        let yaml = "{}";
        let config: RayoConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.base_url.is_none());
    }

    #[test]
    fn load_config_missing_file() {
        let config = load_config(std::path::Path::new("/nonexistent/path"));
        assert!(config.base_url.is_none());
    }
}
