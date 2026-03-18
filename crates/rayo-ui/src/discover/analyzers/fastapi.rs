//! FastAPI framework analyzer.
//!
//! Detects FastAPI by checking Python files for `from fastapi` imports.
//! Greps for `@app.get(`, `@app.post(`, `@router.get(` decorators.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct FastApiAnalyzer;

impl FastApiAnalyzer {
    pub fn detect(project_dir: &Path) -> bool {
        // Check for fastapi in requirements.txt
        let reqs = project_dir.join("requirements.txt");
        if let Ok(content) = std::fs::read_to_string(reqs)
            && content.to_lowercase().contains("fastapi")
        {
            return true;
        }

        // Check pyproject.toml
        let pyproject = project_dir.join("pyproject.toml");
        if let Ok(content) = std::fs::read_to_string(pyproject)
            && content.contains("fastapi")
        {
            return true;
        }

        // Check for `from fastapi` in Python files
        let pattern = project_dir.join("**/*.py").to_string_lossy().to_string();
        if let Ok(entries) = glob::glob(&pattern) {
            for entry in entries.flatten() {
                let path_str = entry.to_string_lossy();
                if path_str.contains("venv")
                    || path_str.contains("__pycache__")
                    || path_str.contains(".tox")
                {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&entry)
                    && (content.contains("from fastapi") || content.contains("import fastapi"))
                {
                    return true;
                }
            }
        }

        false
    }

    fn find_python_files(project_dir: &Path) -> Vec<std::path::PathBuf> {
        let pattern = project_dir.join("**/*.py").to_string_lossy().to_string();
        let mut files = Vec::new();
        if let Ok(entries) = glob::glob(&pattern) {
            for entry in entries.flatten() {
                let path_str = entry.to_string_lossy();
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

impl FrameworkAnalyzer for FastApiAnalyzer {
    fn name(&self) -> &str {
        "FastAPI"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let mut routes = Vec::new();
        let files = Self::find_python_files(project_dir);

        for file in files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let source_file = file.to_string_lossy().to_string();

            // Match @app.get("/path") and @router.get("/path") style decorators
            let prefixes = [
                "@app.get(",
                "@app.post(",
                "@app.put(",
                "@app.delete(",
                "@app.patch(",
                "@router.get(",
                "@router.post(",
                "@router.put(",
                "@router.delete(",
                "@router.patch(",
            ];

            for line in content.lines() {
                let trimmed = line.trim();
                for prefix in &prefixes {
                    if let Some(rest) = find_after(trimmed, prefix)
                        && let Some(route_path) = extract_python_string(rest)
                    {
                        let method = prefix
                            .split('.')
                            .nth(1)
                            .unwrap_or("get(")
                            .strip_suffix('(')
                            .unwrap_or("get")
                            .to_uppercase();

                        // Convert {param} to :param
                        let normalized = normalize_fastapi_path(&route_path);
                        let is_api = true; // FastAPI is primarily an API framework

                        routes.push(DiscoveredRoute {
                            path: normalized,
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
        let all_routes = self.extract_routes(project_dir);
        let file_str = file_path.to_string_lossy();

        all_routes
            .into_iter()
            .filter(|r| r.source_file == file_str.as_ref())
            .map(|r| r.path)
            .collect()
    }
}

fn find_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.find(needle).map(|i| &haystack[i + needle.len()..])
}

fn extract_python_string(text: &str) -> Option<String> {
    let text = text.trim();
    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Convert FastAPI path params `{id}` to `:id`.
fn normalize_fastapi_path(path: &str) -> String {
    let mut result = path.to_string();
    while let Some(start) = result.find('{') {
        if let Some(end) = result[start..].find('}') {
            let param = &result[start + 1..start + end].to_string();
            result = format!(
                "{}:{}{}",
                &result[..start],
                param,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }

    // Ensure leading slash
    if !result.starts_with('/') {
        result = format!("/{result}");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_fastapi_path() {
        assert_eq!(normalize_fastapi_path("/users/{id}"), "/users/:id");
        assert_eq!(
            normalize_fastapi_path("/items/{item_id}/reviews/{review_id}"),
            "/items/:item_id/reviews/:review_id"
        );
        assert_eq!(normalize_fastapi_path("/"), "/");
    }

    #[test]
    fn test_normalize_fastapi_path_no_leading_slash() {
        assert_eq!(normalize_fastapi_path("users"), "/users");
    }

    #[test]
    fn test_extract_routes_from_python_file() {
        let dir = std::env::temp_dir().join("rayo_test_fastapi_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("main.py"),
            r#"
from fastapi import FastAPI, APIRouter

app = FastAPI()
router = APIRouter()

@app.get("/api/users")
async def list_users():
    return []

@app.post("/api/users")
async def create_user():
    return {"created": True}

@router.get("/health")
async def health():
    return {"status": "ok"}
"#,
        )
        .unwrap();

        let analyzer = FastApiAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        assert_eq!(routes.len(), 3, "Should extract 3 routes");

        let paths_methods: Vec<(&str, &str)> = routes
            .iter()
            .map(|r| (r.path.as_str(), r.method.as_str()))
            .collect();

        assert!(paths_methods.contains(&("/api/users", "GET")));
        assert!(paths_methods.contains(&("/api/users", "POST")));
        assert!(paths_methods.contains(&("/health", "GET")));

        // All FastAPI routes should be marked as API
        for route in &routes {
            assert!(route.is_api, "All FastAPI routes should be marked is_api");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_fastapi_by_requirements() {
        let dir = std::env::temp_dir().join("rayo_test_fastapi_detect_reqs");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("requirements.txt"), "fastapi==0.100.0\nuvicorn\n").unwrap();

        assert!(FastApiAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_fastapi_by_pyproject() {
        let dir = std::env::temp_dir().join("rayo_test_fastapi_detect_pyproj");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("pyproject.toml"),
            "[project]\ndependencies = [\"fastapi\"]\n",
        )
        .unwrap();

        assert!(FastApiAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_file_to_routes_returns_only_matching_file() {
        let dir = std::env::temp_dir().join("rayo_test_fastapi_map_file");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let file_a = dir.join("routes_a.py");
        let file_b = dir.join("routes_b.py");

        std::fs::write(&file_a, "@app.get(\"/users\")\nasync def users(): pass\n").unwrap();
        std::fs::write(&file_b, "@app.get(\"/items\")\nasync def items(): pass\n").unwrap();

        let analyzer = FastApiAnalyzer;
        let routes = analyzer.map_file_to_routes(&file_a, &dir);
        assert!(routes.contains(&"/users".to_string()));
        assert!(!routes.contains(&"/items".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
