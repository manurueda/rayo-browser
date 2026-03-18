//! Diff-aware discovery mode.
//!
//! Uses git to determine which files changed on the current branch,
//! then maps those files to routes via the framework analyzer.

use super::analyzers::FrameworkAnalyzer;
use crate::error::TestError;
use std::path::{Path, PathBuf};

/// Get files changed between the current branch and a base branch.
///
/// Runs `git diff {base}...HEAD --name-only` to list changed files.
/// Defaults to `main` as the base branch, falling back to `master`.
pub fn get_changed_files(project_dir: &Path) -> Result<Vec<PathBuf>, TestError> {
    // Try main first, then master
    let base = detect_base_branch(project_dir);

    let output = std::process::Command::new("git")
        .args(["diff", &format!("{base}...HEAD"), "--name-only"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| TestError::Other(format!("Failed to run git diff: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TestError::Other(format!("git diff failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<PathBuf> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| project_dir.join(line.trim()))
        .collect();

    Ok(files)
}

/// Map changed files to affected routes using the framework analyzer.
pub fn map_files_to_routes(
    files: &[PathBuf],
    analyzer: &dyn FrameworkAnalyzer,
    project_dir: &Path,
) -> Vec<String> {
    let mut routes = Vec::new();

    for file in files {
        let file_routes = analyzer.map_file_to_routes(file, project_dir);
        for route in file_routes {
            if !routes.contains(&route) {
                routes.push(route);
            }
        }
    }

    routes
}

/// Detect the default base branch (main or master).
fn detect_base_branch(project_dir: &Path) -> String {
    // Check if 'main' branch exists
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "main"])
        .current_dir(project_dir)
        .output();

    if let Ok(o) = output
        && o.status.success()
    {
        return "main".into();
    }

    // Fall back to master
    "master".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_base_branch() {
        // This test depends on the actual repo structure
        let branch = detect_base_branch(Path::new("."));
        assert!(branch == "main" || branch == "master");
    }
}
