//! Django framework analyzer.
//!
//! Detects Django by checking for `manage.py` or `urls.py` files.
//! Parses `urls.py` files for `path(` and `re_path(` calls.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct DjangoAnalyzer;

impl DjangoAnalyzer {
    pub fn detect(project_dir: &Path) -> bool {
        // Check for manage.py
        if project_dir.join("manage.py").exists() {
            return true;
        }

        // Check for any urls.py file
        let pattern = project_dir.join("**/urls.py").to_string_lossy().to_string();
        if let Ok(entries) = glob::glob(&pattern)
            && entries.count() > 0
        {
            return true;
        }

        false
    }

    fn find_urls_files(project_dir: &Path) -> Vec<std::path::PathBuf> {
        let pattern = project_dir.join("**/urls.py").to_string_lossy().to_string();
        let mut files = Vec::new();
        if let Ok(entries) = glob::glob(&pattern) {
            for entry in entries.flatten() {
                let path_str = entry.to_string_lossy();
                // Skip virtualenv, .venv, __pycache__
                if path_str.contains("venv")
                    || path_str.contains("__pycache__")
                    || path_str.contains(".tox")
                {
                    continue;
                }
                files.push(entry);
            }
        }
        files
    }
}

impl FrameworkAnalyzer for DjangoAnalyzer {
    fn name(&self) -> &str {
        "Django"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();
        let urls_files = Self::find_urls_files(project_dir);

        for file in urls_files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let source_file = file.to_string_lossy().to_string();

            for line in content.lines() {
                let trimmed = line.trim();

                // Skip comments
                if trimmed.starts_with('#') {
                    continue;
                }

                // Match path('route/', view, name='...')
                if let Some(rest) = find_after(trimmed, "path(")
                    && let Some(url) = extract_python_string(rest)
                {
                    let path = normalize_django_path(&url);
                    let is_api = path.starts_with("/api");
                    routes.push(DiscoveredRoute {
                        path,
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: false,
                        is_api,
                    });
                }

                // Match re_path(r'^route/', view, name='...')
                if let Some(rest) = find_after(trimmed, "re_path(")
                    && let Some(url) = extract_python_string(rest)
                {
                    let path = normalize_django_regex_path(&url);
                    let is_api = path.starts_with("/api");
                    routes.push(DiscoveredRoute {
                        path,
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: false,
                        is_api,
                    });
                }
            }
        }

        routes
    }

    fn map_file_to_routes(&self, file_path: &Path, project_dir: &Path) -> Vec<String> {
        let rel = file_path
            .strip_prefix(project_dir)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // urls.py: re-parse for routes
        if rel.ends_with("urls.py") {
            return self
                .extract_routes(project_dir)
                .into_iter()
                .map(|r| r.path)
                .collect();
        }

        // views.py: map to the app's URL prefix
        if rel.ends_with("views.py") {
            let parts: Vec<&str> = rel.split(['/', '\\']).collect();
            if parts.len() >= 2 {
                let app_name = parts[parts.len() - 2];
                return vec![format!("/{app_name}")];
            }
        }

        Vec::new()
    }
}

fn find_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.find(needle).map(|i| &haystack[i + needle.len()..])
}

/// Extract a Python string (single or double quoted).
fn extract_python_string(text: &str) -> Option<String> {
    let text = text.trim();

    // Handle raw strings: r'...' or r"..."
    let text = text.strip_prefix('r').unwrap_or(text);

    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Normalize a Django path to a URL route.
fn normalize_django_path(path: &str) -> String {
    let mut route = format!("/{path}");

    // Convert <int:id> / <str:slug> to :param
    while let Some(start) = route.find('<') {
        if let Some(end) = route[start..].find('>') {
            let param_spec = &route[start + 1..start + end];
            let param_name = param_spec.split(':').next_back().unwrap_or(param_spec);
            route = format!(
                "{}:{}{}",
                &route[..start],
                param_name,
                &route[start + end + 1..]
            );
        } else {
            break;
        }
    }

    // Remove trailing slash for consistency (keep root /)
    if route.len() > 1 {
        route = route.trim_end_matches('/').to_string();
    }

    route
}

/// Normalize a Django regex path to a URL route.
fn normalize_django_regex_path(pattern: &str) -> String {
    let clean = pattern
        .trim_start_matches('^')
        .trim_end_matches('$')
        .replace("(?P<", ":")
        .replace(">\\d+)", "")
        .replace(">[-\\w]+)", "")
        .replace(">[^/]+)", "");

    let mut route = format!("/{clean}");
    if route.len() > 1 {
        route = route.trim_end_matches('/').to_string();
    }
    route
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_django_path() {
        assert_eq!(normalize_django_path(""), "/");
        assert_eq!(normalize_django_path("users/"), "/users");
        assert_eq!(normalize_django_path("users/<int:id>/"), "/users/:id");
        assert_eq!(normalize_django_path("posts/<str:slug>/"), "/posts/:slug");
    }

    #[test]
    fn test_extract_python_string() {
        assert_eq!(extract_python_string("'users/'"), Some("users/".into()));
        assert_eq!(extract_python_string("\"api/v1/\""), Some("api/v1/".into()));
    }

    #[test]
    fn test_extract_python_raw_string() {
        assert_eq!(
            extract_python_string("r'^users/$'"),
            Some("^users/$".into())
        );
    }

    #[test]
    fn test_normalize_django_regex_path() {
        assert_eq!(normalize_django_regex_path("^users/$"), "/users");
        assert_eq!(
            normalize_django_regex_path("^users/(?P<id>\\d+)/$"),
            "/users/:id"
        );
    }

    #[test]
    fn test_extract_routes_from_urls_py() {
        let dir = std::env::temp_dir().join("rayo_test_django_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("urls.py"),
            r#"from django.urls import path
from . import views

urlpatterns = [
    path('', views.home),
    path('login/', views.login),
    path('api/users/', views.users),
]
"#,
        )
        .unwrap();

        let analyzer = DjangoAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/"), "Should have root route");
        assert!(paths.contains(&"/login"), "Should have /login route");
        assert!(
            paths.contains(&"/api/users"),
            "Should have /api/users route"
        );

        // /api/users should be marked as API
        let api_route = routes.iter().find(|r| r.path == "/api/users").unwrap();
        assert!(api_route.is_api);

        assert_eq!(routes.len(), 3, "Should extract exactly 3 routes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_django_by_manage_py() {
        let dir = std::env::temp_dir().join("rayo_test_django_detect_manage");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("manage.py"), "#!/usr/bin/env python\n").unwrap();

        assert!(DjangoAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_skips_comments_in_urls() {
        let dir = std::env::temp_dir().join("rayo_test_django_comments");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("urls.py"),
            r#"urlpatterns = [
    # path('old/', views.old),
    path('active/', views.active),
]
"#,
        )
        .unwrap();

        let analyzer = DjangoAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(!paths.contains(&"/old"), "Should skip commented-out paths");
        assert!(paths.contains(&"/active"), "Should include active paths");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_file_to_routes_views() {
        let dir = std::env::temp_dir().join("rayo_test_django_map_views");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("myapp")).unwrap();

        let views_file = dir.join("myapp/views.py");
        std::fs::write(&views_file, "def home(request):\n    pass\n").unwrap();

        let analyzer = DjangoAnalyzer;
        let routes = analyzer.map_file_to_routes(&views_file, &dir);
        assert!(routes.contains(&"/myapp".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
