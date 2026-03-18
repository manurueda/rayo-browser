//! Ruby on Rails framework analyzer.
//!
//! Detects Rails by checking for `Gemfile` with `rails` or `config/routes.rb`.
//! Parses `config/routes.rb` for `get`, `post`, `resources`, `root` declarations.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct RailsAnalyzer;

impl RailsAnalyzer {
    pub fn detect(project_dir: &Path) -> bool {
        // Check Gemfile
        let gemfile = project_dir.join("Gemfile");
        if let Ok(content) = std::fs::read_to_string(gemfile)
            && (content.contains("'rails'") || content.contains("\"rails\""))
        {
            return true;
        }

        // Check for config/routes.rb
        project_dir.join("config/routes.rb").exists()
    }
}

impl FrameworkAnalyzer for RailsAnalyzer {
    fn name(&self) -> &str {
        "Rails"
    }

    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute> {
        let routes_file = project_dir.join("config/routes.rb");
        let content = match std::fs::read_to_string(&routes_file) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let source_file = routes_file.to_string_lossy().to_string();
        let mut routes = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments
            if trimmed.starts_with('#') {
                continue;
            }

            // root "controller#action" or root to: "controller#action"
            if trimmed.starts_with("root") {
                routes.push(DiscoveredRoute {
                    path: "/".into(),
                    method: "GET".into(),
                    source_file: source_file.clone(),
                    has_form: false,
                    is_api: false,
                });
                continue;
            }

            // get '/path', ... or post '/path', ...
            for method in &["get", "post", "put", "patch", "delete"] {
                if let Some(rest) = strip_method_prefix(trimmed, method)
                    && let Some(path) = extract_ruby_string(rest)
                {
                    let is_api = path.starts_with("/api") || trimmed.contains("format: :json");
                    routes.push(DiscoveredRoute {
                        path,
                        method: method.to_uppercase(),
                        source_file: source_file.clone(),
                        has_form: *method == "get" && !is_api,
                        is_api,
                    });
                }
            }

            // resources :users -> generates standard CRUD routes
            if trimmed.starts_with("resources")
                && let Some(resource_name) = extract_resource_name(trimmed)
            {
                let base = format!("/{resource_name}");
                routes.extend(vec![
                    DiscoveredRoute {
                        path: base.clone(),
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: false,
                        is_api: false,
                    },
                    DiscoveredRoute {
                        path: format!("{base}/new"),
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: true,
                        is_api: false,
                    },
                    DiscoveredRoute {
                        path: format!("{base}/:id"),
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: false,
                        is_api: false,
                    },
                    DiscoveredRoute {
                        path: format!("{base}/:id/edit"),
                        method: "GET".into(),
                        source_file: source_file.clone(),
                        has_form: true,
                        is_api: false,
                    },
                ]);
            }

            // resource :session (singular) -> similar but no :id
            if trimmed.starts_with("resource ")
                && !trimmed.starts_with("resources")
                && let Some(resource_name) = extract_resource_name(trimmed)
            {
                let base = format!("/{resource_name}");
                routes.push(DiscoveredRoute {
                    path: base.clone(),
                    method: "GET".into(),
                    source_file: source_file.clone(),
                    has_form: false,
                    is_api: false,
                });
                routes.push(DiscoveredRoute {
                    path: format!("{base}/new"),
                    method: "GET".into(),
                    source_file: source_file.clone(),
                    has_form: true,
                    is_api: false,
                });
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

        // routes.rb affects all routes
        if rel.contains("routes.rb") {
            return self
                .extract_routes(project_dir)
                .into_iter()
                .map(|r| r.path)
                .collect();
        }

        // Controller file: map controller name to resource route
        // app/controllers/users_controller.rb -> /users
        if rel.contains("controllers/") {
            let filename = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Some(resource) = filename.strip_suffix("_controller") {
                return vec![format!("/{resource}")];
            }
        }

        // View files: app/views/users/ -> /users
        if rel.contains("views/") {
            let parts: Vec<&str> = rel.split(['/', '\\']).collect();
            if let Some(views_idx) = parts.iter().position(|&p| p == "views")
                && views_idx + 1 < parts.len()
            {
                return vec![format!("/{}", parts[views_idx + 1])];
            }
        }

        Vec::new()
    }
}

/// Check if line starts with a method keyword and extract the remainder.
fn strip_method_prefix<'a>(line: &'a str, method: &str) -> Option<&'a str> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix(method)
        && (rest.starts_with(' ') || rest.starts_with('('))
    {
        return Some(rest.trim());
    }
    None
}

/// Extract a Ruby string (single or double quoted) from text.
fn extract_ruby_string(text: &str) -> Option<String> {
    let text = text.trim().trim_start_matches('(');
    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Extract resource name from `resources :users` or `resources :users, only: [...]`.
fn extract_resource_name(line: &str) -> Option<String> {
    let rest = line
        .trim()
        .strip_prefix("resources")
        .or_else(|| line.trim().strip_prefix("resource"))?;
    let rest = rest.trim().trim_start_matches(':').trim_start_matches(' ');
    // Take until comma, space, or end
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_resource_name() {
        assert_eq!(
            extract_resource_name("resources :users"),
            Some("users".into())
        );
        assert_eq!(
            extract_resource_name("resources :posts, only: [:index, :show]"),
            Some("posts".into())
        );
        assert_eq!(
            extract_resource_name("resource :session"),
            Some("session".into())
        );
    }

    #[test]
    fn test_extract_ruby_string() {
        assert_eq!(extract_ruby_string("'/login'"), Some("/login".into()));
        assert_eq!(extract_ruby_string("\"/users\""), Some("/users".into()));
    }
}
