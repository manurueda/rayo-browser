//! Persistence for scan run results — saves and loads JSON + screenshots.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level persisted data for a scan run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub url: String,
    pub framework: String,
    pub health_score: u32,
    pub total_flows: usize,
    pub passed_flows: usize,
    pub failed_flows: usize,
    pub total_duration_ms: u64,
    pub scan_duration_ms: u64,
    pub console_errors: u32,
    pub timestamp: String, // ISO 8601
    pub flows: Vec<FlowResult>,
}

/// Per-flow result with screenshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowResult {
    pub name: String,
    pub description: String,
    pub flow_type: String,
    pub importance: String,
    pub url: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub steps: Vec<FlowStepResult>,
    pub error: Option<String>,
}

/// Per-step result within a flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStepResult {
    pub name: String,
    pub action: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub screenshot_path: Option<String>,
}

/// Convert a string into a URL-friendly slug.
///
/// Lowercases, replaces non-alphanumeric characters with hyphens,
/// collapses consecutive hyphens, and trims leading/trailing hyphens.
fn slugify(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse multiple hyphens into one
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    // Trim leading/trailing hyphens
    result.trim_matches('-').to_string()
}

/// Convert an ISO 8601 timestamp to directory-friendly format (YYYYMMDD-HHMMSS).
fn timestamp_to_dirname(timestamp: &str) -> String {
    // Parse ISO 8601 like "2026-03-21T11:00:00Z" or "2026-03-21T11:00:00+00:00"
    // into "20260321-110000"
    let cleaned: String = timestamp.chars().filter(|c| c.is_ascii_digit()).collect();
    if cleaned.len() >= 14 {
        format!("{}-{}", &cleaned[..8], &cleaned[8..14])
    } else {
        // Fallback: use the cleaned digits as-is
        cleaned
    }
}

/// Save a scan run result to disk.
///
/// Creates `.rayo/runs/{timestamp}/` and writes `result.json`.
/// Returns the path to the run directory.
pub fn save_run(result: &ScanResult, base_dir: &Path) -> Result<PathBuf, std::io::Error> {
    let dirname = timestamp_to_dirname(&result.timestamp);
    let run_dir = base_dir.join(".rayo/runs").join(&dirname);
    std::fs::create_dir_all(&run_dir)?;

    let json = serde_json::to_string_pretty(result)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(run_dir.join("result.json"), json)?;

    Ok(run_dir)
}

/// Save a screenshot for a flow step.
///
/// Creates `screenshots/` subdirectory in the run directory if needed,
/// saves the JPEG bytes as `{flow_slug}-step{index}.jpg`, and returns
/// the relative path (e.g. `"screenshots/login-flow-step0.jpg"`).
pub fn save_screenshot(
    run_dir: &Path,
    flow_name: &str,
    step_index: usize,
    jpeg_bytes: &[u8],
) -> Result<String, std::io::Error> {
    let screenshots_dir = run_dir.join("screenshots");
    std::fs::create_dir_all(&screenshots_dir)?;

    let slug = slugify(flow_name);
    let filename = format!("{slug}-step{step_index}.jpg");
    std::fs::write(screenshots_dir.join(&filename), jpeg_bytes)?;

    Ok(format!("screenshots/{filename}"))
}

/// Load a scan run result from a run directory.
///
/// Reads and deserializes `result.json` from the given directory.
pub fn load_run(run_dir: &Path) -> Result<ScanResult, std::io::Error> {
    let json = std::fs::read_to_string(run_dir.join("result.json"))?;
    let result: ScanResult = serde_json::from_str(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(result)
}

/// List all run directories sorted newest first.
///
/// Scans `.rayo/runs/` under `base_dir` and returns paths to run directories.
pub fn list_runs(base_dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let runs_dir = base_dir.join(".rayo/runs");
    if !runs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&runs_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect();

    // Sort by directory name descending (newest first, since names are timestamps)
    dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    Ok(dirs)
}

/// Load the most recent scan run.
///
/// Returns `None` if no runs exist.
pub fn load_latest_run(base_dir: &Path) -> Result<Option<ScanResult>, std::io::Error> {
    let runs = list_runs(base_dir)?;
    match runs.first() {
        Some(latest) => Ok(Some(load_run(latest)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scan_result(timestamp: &str) -> ScanResult {
        ScanResult {
            url: "http://localhost:3000".to_string(),
            framework: "Next.js".to_string(),
            health_score: 85,
            total_flows: 3,
            passed_flows: 2,
            failed_flows: 1,
            total_duration_ms: 5000,
            scan_duration_ms: 4500,
            console_errors: 2,
            timestamp: timestamp.to_string(),
            flows: vec![
                FlowResult {
                    name: "Login Flow".to_string(),
                    description: "Tests user login with valid credentials".to_string(),
                    flow_type: "auth".to_string(),
                    importance: "critical".to_string(),
                    url: "http://localhost:3000/login".to_string(),
                    passed: true,
                    duration_ms: 1200,
                    steps: vec![
                        FlowStepResult {
                            name: "Navigate to login".to_string(),
                            action: "navigate".to_string(),
                            passed: true,
                            duration_ms: 300,
                            error: None,
                            screenshot_path: None,
                        },
                        FlowStepResult {
                            name: "Fill credentials".to_string(),
                            action: "type".to_string(),
                            passed: true,
                            duration_ms: 200,
                            error: None,
                            screenshot_path: None,
                        },
                    ],
                    error: None,
                },
                FlowResult {
                    name: "Search Form".to_string(),
                    description: "Tests search functionality".to_string(),
                    flow_type: "form".to_string(),
                    importance: "high".to_string(),
                    url: "http://localhost:3000/search".to_string(),
                    passed: false,
                    duration_ms: 800,
                    steps: vec![],
                    error: Some("Element not found".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Login Flow"), "login-flow");
        assert_eq!(slugify("Hello   World!!!"), "hello-world");
        assert_eq!(slugify("  leading-trailing  "), "leading-trailing");
        assert_eq!(slugify("CamelCase Test"), "camelcase-test");
        assert_eq!(slugify("a--b"), "a-b");
    }

    #[test]
    fn test_timestamp_to_dirname() {
        assert_eq!(
            timestamp_to_dirname("2026-03-21T11:00:00Z"),
            "20260321-110000"
        );
        assert_eq!(
            timestamp_to_dirname("2026-03-21T15:30:45+00:00"),
            "20260321-153045"
        );
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let result = sample_scan_result("2026-03-21T11:00:00Z");

        let run_dir = save_run(&result, tmp.path()).unwrap();
        assert!(run_dir.join("result.json").exists());

        let loaded = load_run(&run_dir).unwrap();
        assert_eq!(loaded.url, result.url);
        assert_eq!(loaded.framework, result.framework);
        assert_eq!(loaded.health_score, result.health_score);
        assert_eq!(loaded.total_flows, result.total_flows);
        assert_eq!(loaded.passed_flows, result.passed_flows);
        assert_eq!(loaded.failed_flows, result.failed_flows);
        assert_eq!(loaded.total_duration_ms, result.total_duration_ms);
        assert_eq!(loaded.scan_duration_ms, result.scan_duration_ms);
        assert_eq!(loaded.console_errors, result.console_errors);
        assert_eq!(loaded.timestamp, result.timestamp);
        assert_eq!(loaded.flows.len(), 2);
        assert_eq!(loaded.flows[0].name, "Login Flow");
        assert_eq!(loaded.flows[0].steps.len(), 2);
        assert!(loaded.flows[0].passed);
        assert!(!loaded.flows[1].passed);
        assert_eq!(loaded.flows[1].error, Some("Element not found".to_string()));
    }

    #[test]
    fn test_list_runs_ordering() {
        let tmp = tempfile::tempdir().unwrap();

        // Save runs with different timestamps
        let r1 = sample_scan_result("2026-03-20T10:00:00Z");
        let r2 = sample_scan_result("2026-03-21T11:00:00Z");
        let r3 = sample_scan_result("2026-03-19T09:00:00Z");

        save_run(&r1, tmp.path()).unwrap();
        save_run(&r2, tmp.path()).unwrap();
        save_run(&r3, tmp.path()).unwrap();

        let runs = list_runs(tmp.path()).unwrap();
        assert_eq!(runs.len(), 3);

        // Newest first
        let names: Vec<_> = runs
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert_eq!(names[0], "20260321-110000");
        assert_eq!(names[1], "20260320-100000");
        assert_eq!(names[2], "20260319-090000");
    }

    #[test]
    fn test_save_screenshot() {
        let tmp = tempfile::tempdir().unwrap();
        let run_dir = tmp.path().join("run1");
        std::fs::create_dir_all(&run_dir).unwrap();

        let fake_jpeg = b"\xFF\xD8\xFF\xE0fake-jpeg-data";
        let rel_path = save_screenshot(&run_dir, "Login Flow", 0, fake_jpeg).unwrap();
        assert_eq!(rel_path, "screenshots/login-flow-step0.jpg");

        let saved = std::fs::read(run_dir.join(&rel_path)).unwrap();
        assert_eq!(saved, fake_jpeg);

        // Second screenshot in same flow
        let rel_path2 = save_screenshot(&run_dir, "Login Flow", 1, fake_jpeg).unwrap();
        assert_eq!(rel_path2, "screenshots/login-flow-step1.jpg");
    }

    #[test]
    fn test_load_latest_run_no_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_latest_run(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_latest_run() {
        let tmp = tempfile::tempdir().unwrap();

        let r1 = sample_scan_result("2026-03-20T10:00:00Z");
        let r2 = sample_scan_result("2026-03-21T11:00:00Z");

        save_run(&r1, tmp.path()).unwrap();
        save_run(&r2, tmp.path()).unwrap();

        let latest = load_latest_run(tmp.path()).unwrap().unwrap();
        assert_eq!(latest.timestamp, "2026-03-21T11:00:00Z");
    }
}
