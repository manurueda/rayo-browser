//! Express.js framework analyzer.
//!
//! Detects Express by checking `package.json` for the `express` dependency,
//! then greps for `app.get(`, `router.post(`, etc. to extract routes.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct ExpressAnalyzer;

impl ExpressAnalyzer {
    pub fn detect(project_dir: &Path) -> bool {
        let pkg_path = project_dir.join("package.json");
        if let Ok(content) = std::fs::read_to_string(pkg_path) {
            return content.contains("\"express\"");
        }
        false
    }

    fn find_route_files(project_dir: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for ext in &["js", "ts", "mjs"] {
            let pattern = project_dir.join(format!("**/*.{ext}"));
            let pattern_str = pattern.to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&pattern_str) {
                for entry in entries.flatten() {
                    // Skip node_modules and dist/build dirs
                    let path_str = entry.to_string_lossy();
                    if path_str.contains("node_modules")
                        || path_str.contains("/dist/")
                        || path_str.contains("/build/")
                        || path_str.contains("/.next/")
                    {
                        continue;
                    }
                    files.push(entry);
                }
            }
        }
        files
    }
}

impl FrameworkAnalyzer for ExpressAnalyzer {
    fn name(&self) -> &str {
        "Express"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();
        let files = Self::find_route_files(project_dir);

        for file in files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let source_file = file.to_string_lossy().to_string();

            // Match patterns like app.get('/path', ...) or router.post("/path", ...)
            let prefixes = [
                "app.get(",
                "app.post(",
                "app.put(",
                "app.delete(",
                "app.patch(",
                "router.get(",
                "router.post(",
                "router.put(",
                "router.delete(",
                "router.patch(",
            ];

            for line in content.lines() {
                let trimmed = line.trim();
                for prefix in &prefixes {
                    if let Some(rest) = find_after(trimmed, prefix)
                        && let Some(route_path) = extract_string_arg(rest)
                    {
                        let method = prefix
                            .split('.')
                            .nth(1)
                            .unwrap_or("get")
                            .strip_suffix('(')
                            .unwrap_or("get")
                            .to_uppercase();

                        let is_api = route_path.starts_with("/api");

                        routes.push(DiscoveredRoute {
                            path: route_path,
                            method,
                            source_file: source_file.clone(),
                            has_form: false,
                            is_api,
                        });
                    }
                }
            }
        }

        routes
    }

    fn map_file_to_routes(&self, file_path: &Path, project_dir: &Path) -> Vec<String> {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_routes = self.extract_routes(project_dir);
        let file_str = file_path.to_string_lossy();

        all_routes
            .into_iter()
            .filter(|r| {
                // If the file matches, or if the content contains this route path
                r.source_file == file_str.as_ref() || content.contains(&r.path)
            })
            .map(|r| r.path)
            .collect()
    }
}

/// Find the remainder of a string after a needle.
fn find_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.find(needle).map(|i| &haystack[i + needle.len()..])
}

/// Extract a quoted string argument from the beginning of text.
/// Handles both single and double quotes: `'/path'` or `"/path"`.
fn extract_string_arg(text: &str) -> Option<String> {
    let text = text.trim();
    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    let path = &rest[..end];

    // Basic validation: must start with /
    if path.starts_with('/') {
        Some(path.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_string_arg() {
        assert_eq!(
            extract_string_arg("'/api/users'"),
            Some("/api/users".into())
        );
        assert_eq!(extract_string_arg("\"/login\""), Some("/login".into()));
        assert_eq!(extract_string_arg("middleware"), None);
    }
}
