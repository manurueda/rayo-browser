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
    use crate::discover::analyzers::static_html::StaticHtmlAnalyzer;

    #[test]
    fn test_detect_base_branch() {
        // This test depends on the actual repo structure
        let branch = detect_base_branch(Path::new("."));
        assert!(branch == "main" || branch == "master");
    }

    #[test]
    fn test_map_files_to_routes_static_html() {
        let dir = std::env::temp_dir().join("rayo_test_diff_map_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create HTML files in the project directory
        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("about.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("contact.html"), "<html></html>").unwrap();

        let analyzer = StaticHtmlAnalyzer;

        // Simulate changed files: only about.html and contact.html changed
        let changed_files = vec![dir.join("about.html"), dir.join("contact.html")];

        let routes = map_files_to_routes(&changed_files, &analyzer, &dir);

        assert!(
            routes.contains(&"/about".to_string()),
            "Should map about.html to /about"
        );
        assert!(
            routes.contains(&"/contact".to_string()),
            "Should map contact.html to /contact"
        );
        // index.html was NOT changed, so / should not be in the result
        assert!(
            !routes.contains(&"/".to_string()),
            "Should not include routes for unchanged files"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_files_to_routes_deduplicates() {
        let dir = std::env::temp_dir().join("rayo_test_diff_dedup");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();

        let analyzer = StaticHtmlAnalyzer;

        // Pass the same file twice
        let changed_files = vec![dir.join("index.html"), dir.join("index.html")];

        let routes = map_files_to_routes(&changed_files, &analyzer, &dir);
        assert_eq!(
            routes.len(),
            1,
            "Should deduplicate routes from the same file"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_files_to_routes_empty_input() {
        let dir = std::env::temp_dir().join("rayo_test_diff_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let analyzer = StaticHtmlAnalyzer;
        let routes = map_files_to_routes(&[], &analyzer, &dir);
        assert!(routes.is_empty(), "No files should produce no routes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_files_to_routes_non_html_file() {
        let dir = std::env::temp_dir().join("rayo_test_diff_nonhtml");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("style.css"), "body {}").unwrap();

        let analyzer = StaticHtmlAnalyzer;
        let changed_files = vec![dir.join("style.css")];
        let routes = map_files_to_routes(&changed_files, &analyzer, &dir);
        assert!(
            routes.is_empty(),
            "Non-HTML files should not map to routes with StaticHtmlAnalyzer"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
