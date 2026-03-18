//! Next.js framework analyzer.
//!
//! Detects both App Router (`app/` directory with `page.tsx` files)
//! and Pages Router (`pages/` directory) conventions.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct NextJsAnalyzer;

impl NextJsAnalyzer {
    /// Check if the project uses Next.js.
    pub fn detect(project_dir: &Path) -> bool {
        // Check package.json for "next" dependency
        let pkg_path = project_dir.join("package.json");
        if let Ok(content) = std::fs::read_to_string(&pkg_path)
            && content.contains("\"next\"")
        {
            return true;
        }

        // Check for App Router structure
        let app_dir = project_dir.join("app");
        if app_dir.is_dir() {
            let pattern = app_dir.join("**/page.tsx").to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&pattern)
                && entries.count() > 0
            {
                return true;
            }
            let pattern = app_dir.join("**/page.jsx").to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&pattern)
                && entries.count() > 0
            {
                return true;
            }
        }

        // Also check src/app for Next.js projects with src directory
        let src_app_dir = project_dir.join("src/app");
        if src_app_dir.is_dir() {
            return true;
        }

        // Check for Pages Router
        project_dir.join("pages").is_dir()
    }

    fn find_app_routes(project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();

        // Search both app/ and src/app/
        for base in &["app", "src/app"] {
            let app_dir = project_dir.join(base);
            if !app_dir.is_dir() {
                continue;
            }

            // Find page.tsx/page.jsx files (actual pages)
            for ext in &["tsx", "jsx", "ts", "js"] {
                let pattern = app_dir.join(format!("**/page.{ext}"));
                let pattern_str = pattern.to_string_lossy().to_string();
                let entries = match glob::glob(&pattern_str) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                for entry in entries.flatten() {
                    let rel = entry
                        .strip_prefix(&app_dir)
                        .unwrap_or(&entry)
                        .to_string_lossy()
                        .to_string();

                    // Convert file path to route:
                    // app/page.tsx -> /
                    // app/login/page.tsx -> /login
                    // app/users/[id]/page.tsx -> /users/:id
                    let route = path_to_route(&rel);
                    let source_file = entry.to_string_lossy().to_string();

                    routes.push(DiscoveredRoute {
                        path: route,
                        method: "GET".into(),
                        source_file,
                        has_form: false,
                        is_api: false,
                    });
                }
            }

            // Find API route handlers
            for ext in &["ts", "js"] {
                let pattern = app_dir.join(format!("**/route.{ext}"));
                let pattern_str = pattern.to_string_lossy().to_string();
                let entries = match glob::glob(&pattern_str) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                for entry in entries.flatten() {
                    let rel = entry
                        .strip_prefix(&app_dir)
                        .unwrap_or(&entry)
                        .to_string_lossy()
                        .to_string();

                    let route = path_to_route(&rel);
                    let source_file = entry.to_string_lossy().to_string();

                    // Detect methods from file content
                    let methods = detect_api_methods(&entry);
                    for method in methods {
                        routes.push(DiscoveredRoute {
                            path: route.clone(),
                            method,
                            source_file: source_file.clone(),
                            has_form: false,
                            is_api: true,
                        });
                    }
                }
            }
        }

        routes
    }

    fn find_pages_routes(project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();
        let pages_dir = project_dir.join("pages");
        if !pages_dir.is_dir() {
            return routes;
        }

        for ext in &["tsx", "jsx", "ts", "js"] {
            let pattern = pages_dir.join(format!("**/*.{ext}"));
            let pattern_str = pattern.to_string_lossy().to_string();
            let entries = match glob::glob(&pattern_str) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let rel = entry
                    .strip_prefix(&pages_dir)
                    .unwrap_or(&entry)
                    .to_string_lossy()
                    .to_string();

                // Skip special Next.js files
                let filename = entry.file_name().unwrap_or_default().to_string_lossy();
                if filename.starts_with('_') {
                    continue;
                }

                let is_api = rel.starts_with("api/") || rel.starts_with("api\\");
                let route = pages_path_to_route(&rel);
                let source_file = entry.to_string_lossy().to_string();

                routes.push(DiscoveredRoute {
                    path: route,
                    method: "GET".into(),
                    source_file,
                    has_form: false,
                    is_api,
                });
            }
        }

        routes
    }
}

impl FrameworkAnalyzer for NextJsAnalyzer {
    fn name(&self) -> &str {
        "Next.js"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Self::find_app_routes(project_dir);
        routes.extend(Self::find_pages_routes(project_dir));
        routes
    }

    fn map_file_to_routes(&self, file_path: &Path, project_dir: &Path) -> Vec<String> {
        let rel = file_path
            .strip_prefix(project_dir)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let mut routes = Vec::new();

        // Direct page file
        if rel.contains("page.") || rel.contains("route.") {
            let route = path_to_route(&rel);
            routes.push(route);
        }

        // Component files: check if they're imported by page files (heuristic)
        // For now, if file is inside app/some-route/, map it to that route
        let parts: Vec<&str> = rel.split(['/', '\\']).collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "app" || *part == "pages" {
                // Build route from segments between app/ and the file
                let route_parts: Vec<&str> = parts[i + 1..]
                    .iter()
                    .take_while(|p| !p.contains('.'))
                    .copied()
                    .collect();
                if !route_parts.is_empty() {
                    let route = format!("/{}", route_parts.join("/"));
                    if !routes.contains(&route) {
                        routes.push(route);
                    }
                }
                break;
            }
        }

        routes
    }
}

/// Convert an App Router file path to a URL route.
fn path_to_route(rel_path: &str) -> String {
    let clean = rel_path
        .replace('\\', "/")
        .replace("page.tsx", "")
        .replace("page.jsx", "")
        .replace("page.ts", "")
        .replace("page.js", "")
        .replace("route.tsx", "")
        .replace("route.jsx", "")
        .replace("route.ts", "")
        .replace("route.js", "");

    // Remove app/ or src/app/ prefix
    let clean = clean
        .strip_prefix("app/")
        .or_else(|| clean.strip_prefix("src/app/"))
        .unwrap_or(&clean);

    // Convert [param] to :param
    let segments: Vec<String> = clean
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.starts_with('[') && s.ends_with(']') {
                format!(":{}", &s[1..s.len() - 1])
            } else {
                s.to_string()
            }
        })
        .collect();

    if segments.is_empty() {
        "/".into()
    } else {
        format!("/{}", segments.join("/"))
    }
}

/// Convert a Pages Router file path to a URL route.
fn pages_path_to_route(rel_path: &str) -> String {
    let clean = rel_path.replace('\\', "/");

    // Remove file extension
    let clean = clean
        .strip_suffix(".tsx")
        .or_else(|| clean.strip_suffix(".jsx"))
        .or_else(|| clean.strip_suffix(".ts"))
        .or_else(|| clean.strip_suffix(".js"))
        .unwrap_or(&clean);

    // Convert index to /
    let clean = if clean == "index" {
        return "/".into();
    } else {
        clean.strip_suffix("/index").unwrap_or(clean)
    };

    // Convert [param] to :param
    let segments: Vec<String> = clean
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.starts_with('[') && s.ends_with(']') {
                format!(":{}", &s[1..s.len() - 1])
            } else {
                s.to_string()
            }
        })
        .collect();

    format!("/{}", segments.join("/"))
}

/// Detect HTTP methods exported from an API route file.
fn detect_api_methods(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec!["GET".into()],
    };

    let mut methods = Vec::new();
    for method in &["GET", "POST", "PUT", "DELETE", "PATCH"] {
        // Look for `export async function GET` or `export function GET` or `export const GET`
        if content.contains(&format!("function {method}"))
            || content.contains(&format!("const {method}"))
        {
            methods.push(method.to_string());
        }
    }

    if methods.is_empty() {
        methods.push("GET".into());
    }

    methods
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_route() {
        assert_eq!(path_to_route("page.tsx"), "/");
        assert_eq!(path_to_route("login/page.tsx"), "/login");
        assert_eq!(path_to_route("users/[id]/page.tsx"), "/users/:id");
        assert_eq!(
            path_to_route("dashboard/settings/page.tsx"),
            "/dashboard/settings"
        );
    }

    #[test]
    fn test_pages_path_to_route() {
        assert_eq!(pages_path_to_route("index.tsx"), "/");
        assert_eq!(pages_path_to_route("about.tsx"), "/about");
        assert_eq!(pages_path_to_route("users/[id].tsx"), "/users/:id");
        assert_eq!(pages_path_to_route("api/users/index.ts"), "/api/users");
    }
}
