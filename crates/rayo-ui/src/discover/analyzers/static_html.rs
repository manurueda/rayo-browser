//! Static HTML site analyzer.
//!
//! Detects projects that are plain HTML files without a framework.
//! Each `.html` file maps to a route.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct StaticHtmlAnalyzer;

/// Directories to search for HTML files.
const HTML_DIRS: &[&str] = &[".", "public", "src", "dist", "www", "static", "html"];

impl StaticHtmlAnalyzer {
    pub fn detect(project_dir: &Path) -> bool {
        for dir in HTML_DIRS {
            let search_dir = if *dir == "." {
                project_dir.to_path_buf()
            } else {
                project_dir.join(dir)
            };

            if !search_dir.is_dir() {
                continue;
            }

            let pattern = search_dir.join("*.html").to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&pattern)
                && entries.count() > 0
            {
                return true;
            }
        }
        false
    }

    fn find_html_files(project_dir: &Path) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
        let mut files = Vec::new();

        for dir in HTML_DIRS {
            let search_dir = if *dir == "." {
                project_dir.to_path_buf()
            } else {
                project_dir.join(dir)
            };

            if !search_dir.is_dir() {
                continue;
            }

            let pattern = search_dir.join("**/*.html").to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&pattern) {
                for entry in entries.flatten() {
                    let path_str = entry.to_string_lossy();
                    // Skip node_modules, build artifacts, etc.
                    if path_str.contains("node_modules")
                        || path_str.contains("/.next/")
                        || path_str.contains("/target/")
                    {
                        continue;
                    }
                    files.push((entry, search_dir.clone()));
                }
            }
        }

        files
    }
}

impl FrameworkAnalyzer for StaticHtmlAnalyzer {
    fn name(&self) -> &str {
        "Static HTML"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();
        let files = Self::find_html_files(project_dir);

        for (file, base_dir) in files {
            let rel = file
                .strip_prefix(&base_dir)
                .unwrap_or(&file)
                .to_string_lossy()
                .to_string();

            let route = html_path_to_route(&rel);
            let source_file = file.to_string_lossy().to_string();

            // Check if the file contains a form
            let has_form = if let Ok(content) = std::fs::read_to_string(&file) {
                content.contains("<form")
            } else {
                false
            };

            routes.push(DiscoveredRoute {
                path: route,
                method: "GET".into(),
                source_file,
                has_form,
                is_api: false,
            });
        }

        routes
    }

    fn map_file_to_routes(&self, file_path: &Path, project_dir: &Path) -> Vec<String> {
        if file_path.extension().is_some_and(|e| e == "html") {
            let rel = file_path
                .strip_prefix(project_dir)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();
            vec![html_path_to_route(&rel)]
        } else {
            Vec::new()
        }
    }
}

/// Convert HTML file path to URL route.
fn html_path_to_route(rel_path: &str) -> String {
    let clean = rel_path.replace('\\', "/");

    // index.html -> /
    if clean == "index.html" {
        return "/".into();
    }

    // Remove .html extension
    let clean = clean.strip_suffix(".html").unwrap_or(&clean);

    // Remove index suffix: about/index -> /about
    let clean = clean.strip_suffix("/index").unwrap_or(clean);

    if clean.is_empty() {
        "/".into()
    } else {
        format!("/{clean}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_path_to_route() {
        assert_eq!(html_path_to_route("index.html"), "/");
        assert_eq!(html_path_to_route("about.html"), "/about");
        assert_eq!(html_path_to_route("pages/contact.html"), "/pages/contact");
    }
}
