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

    #[test]
    fn test_extract_routes_app_router() {
        let dir = std::env::temp_dir().join("rayo_test_nextjs_app_router");
        let _ = std::fs::remove_dir_all(&dir);
        let app = dir.join("app");

        // Create app router pages
        std::fs::create_dir_all(app.join("login")).unwrap();
        std::fs::create_dir_all(app.join("dashboard/settings")).unwrap();
        std::fs::create_dir_all(app.join("api/users")).unwrap();

        std::fs::write(app.join("page.tsx"), "export default function Home() {}").unwrap();
        std::fs::write(
            app.join("login/page.tsx"),
            "export default function Login() {}",
        )
        .unwrap();
        std::fs::write(
            app.join("dashboard/settings/page.tsx"),
            "export default function Settings() {}",
        )
        .unwrap();
        // API route
        std::fs::write(
            app.join("api/users/route.ts"),
            "export async function GET(req) { return Response.json([]); }",
        )
        .unwrap();
        // Files that should be skipped (not page/route files)
        std::fs::write(
            app.join("layout.tsx"),
            "export default function Layout() {}",
        )
        .unwrap();
        std::fs::write(
            app.join("not-found.tsx"),
            "export default function NotFound() {}",
        )
        .unwrap();

        let analyzer = NextJsAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/"), "Should have root route");
        assert!(paths.contains(&"/login"), "Should have /login route");
        assert!(
            paths.contains(&"/dashboard/settings"),
            "Should have /dashboard/settings route"
        );
        assert!(
            paths.contains(&"/api/users"),
            "Should have /api/users API route"
        );
        // layout.tsx and not-found.tsx should not produce routes
        assert!(
            !paths.contains(&"/layout"),
            "layout.tsx should not be a route"
        );
        assert!(
            !paths.contains(&"/not-found"),
            "not-found.tsx should not be a route"
        );

        // API route should be marked as API
        let api_route = routes.iter().find(|r| r.path == "/api/users").unwrap();
        assert!(api_route.is_api);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_routes_pages_router() {
        let dir = std::env::temp_dir().join("rayo_test_nextjs_pages_router");
        let _ = std::fs::remove_dir_all(&dir);
        let pages = dir.join("pages");

        std::fs::create_dir_all(&pages).unwrap();
        std::fs::write(
            pages.join("about.tsx"),
            "export default function About() {}",
        )
        .unwrap();
        // _app.tsx should be skipped (starts with _)
        std::fs::write(pages.join("_app.tsx"), "export default function App() {}").unwrap();

        let analyzer = NextJsAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/about"), "Should have /about route");
        assert!(
            !paths.iter().any(|p| p.contains("_app")),
            "_app.tsx should be skipped"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_nextjs_by_package_json() {
        let dir = std::env::temp_dir().join("rayo_test_nextjs_detect");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"dependencies": {"next": "14.0.0"}}"#,
        )
        .unwrap();

        assert!(NextJsAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_api_methods_from_file() {
        let dir = std::env::temp_dir().join("rayo_test_nextjs_api_methods");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let route_file = dir.join("route.ts");
        std::fs::write(
            &route_file,
            r#"
export async function GET(request: Request) {
    return Response.json({ users: [] });
}

export async function POST(request: Request) {
    return Response.json({ created: true });
}
"#,
        )
        .unwrap();

        let methods = detect_api_methods(&route_file);
        assert!(methods.contains(&"GET".to_string()));
        assert!(methods.contains(&"POST".to_string()));
        assert_eq!(methods.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
