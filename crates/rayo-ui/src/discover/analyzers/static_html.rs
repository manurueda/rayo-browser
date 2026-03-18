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

    #[test]
    fn test_html_path_to_route_nested_index() {
        assert_eq!(html_path_to_route("blog/index.html"), "/blog");
    }

    #[test]
    fn test_extract_routes_from_html_files() {
        let dir = std::env::temp_dir().join("rayo_test_static_html_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("index.html"),
            "<html><body><h1>Home</h1></body></html>",
        )
        .unwrap();
        std::fs::write(
            dir.join("about.html"),
            "<html><body><h1>About</h1></body></html>",
        )
        .unwrap();
        std::fs::write(
            dir.join("contact.html"),
            "<html><body><h1>Contact</h1><form><input></form></body></html>",
        )
        .unwrap();

        let analyzer = StaticHtmlAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(
            paths.contains(&"/"),
            "Should have root route from index.html"
        );
        assert!(
            paths.contains(&"/about"),
            "Should have /about route from about.html"
        );
        assert!(
            paths.contains(&"/contact"),
            "Should have /contact route from contact.html"
        );
        assert_eq!(routes.len(), 3, "Should extract exactly 3 routes");

        // contact.html has a form
        let contact = routes.iter().find(|r| r.path == "/contact").unwrap();
        assert!(contact.has_form, "contact.html should have has_form=true");

        // None should be API routes
        for route in &routes {
            assert!(!route.is_api, "Static HTML routes should not be API");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_static_html() {
        let dir = std::env::temp_dir().join("rayo_test_static_html_detect");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // No HTML files -> should not detect
        assert!(!StaticHtmlAnalyzer::detect(&dir));

        // Add an HTML file -> should detect
        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
        assert!(StaticHtmlAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_static_html_in_public_dir() {
        let dir = std::env::temp_dir().join("rayo_test_static_html_public");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("public")).unwrap();

        std::fs::write(dir.join("public/index.html"), "<html></html>").unwrap();
        assert!(StaticHtmlAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_file_to_routes_html() {
        let dir = std::env::temp_dir().join("rayo_test_static_html_map");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let html_file = dir.join("about.html");
        std::fs::write(&html_file, "<html></html>").unwrap();

        let analyzer = StaticHtmlAnalyzer;
        let routes = analyzer.map_file_to_routes(&html_file, &dir);
        assert!(routes.contains(&"/about".to_string()));

        // Non-HTML files should return empty
        let js_file = dir.join("app.js");
        std::fs::write(&js_file, "console.log('hi')").unwrap();
        let routes = analyzer.map_file_to_routes(&js_file, &dir);
        assert!(routes.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
