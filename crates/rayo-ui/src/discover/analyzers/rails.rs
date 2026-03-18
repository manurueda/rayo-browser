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

    #[test]
    fn test_strip_method_prefix() {
        assert_eq!(strip_method_prefix("get '/foo'", "get"), Some("'/foo'"));
        assert_eq!(strip_method_prefix("post '/bar'", "post"), Some("'/bar'"));
        assert_eq!(strip_method_prefix("get('/baz')", "get"), Some("('/baz')"));
        assert_eq!(strip_method_prefix("something else", "get"), None);
    }

    #[test]
    fn test_extract_routes_from_routes_rb() {
        let dir = std::env::temp_dir().join("rayo_test_rails_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();

        std::fs::write(
            dir.join("config/routes.rb"),
            r#"Rails.application.routes.draw do
  root "home#index"
  get "/login", to: "sessions#new"
  resources :users
end
"#,
        )
        .unwrap();

        let analyzer = RailsAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();

        // root route
        assert!(paths.contains(&"/"), "Should have root route");

        // get /login
        assert!(paths.contains(&"/login"), "Should have /login route");

        // resources :users generates CRUD routes
        assert!(
            paths.contains(&"/users"),
            "Should have /users index route from resources"
        );
        assert!(
            paths.contains(&"/users/new"),
            "Should have /users/new route from resources"
        );
        assert!(
            paths.contains(&"/users/:id"),
            "Should have /users/:id route from resources"
        );
        assert!(
            paths.contains(&"/users/:id/edit"),
            "Should have /users/:id/edit route from resources"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_rails_by_gemfile() {
        let dir = std::env::temp_dir().join("rayo_test_rails_detect_gemfile");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'rails', '~> 7.0'\n",
        )
        .unwrap();

        assert!(RailsAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_rails_by_routes_rb() {
        let dir = std::env::temp_dir().join("rayo_test_rails_detect_routes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();

        std::fs::write(
            dir.join("config/routes.rb"),
            "Rails.application.routes.draw do\nend\n",
        )
        .unwrap();

        assert!(RailsAnalyzer::detect(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_skips_comments_in_routes() {
        let dir = std::env::temp_dir().join("rayo_test_rails_comments");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("config")).unwrap();

        std::fs::write(
            dir.join("config/routes.rb"),
            r#"Rails.application.routes.draw do
  # get "/old-route", to: "legacy#index"
  get "/active-route", to: "pages#show"
end
"#,
        )
        .unwrap();

        let analyzer = RailsAnalyzer;
        let routes = analyzer.extract_routes(&dir);

        let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
        assert!(
            !paths.contains(&"/old-route"),
            "Should skip commented-out routes"
        );
        assert!(
            paths.contains(&"/active-route"),
            "Should include active routes"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_file_to_routes_controller() {
        let dir = std::env::temp_dir().join("rayo_test_rails_map_controller");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("app/controllers")).unwrap();

        let controller = dir.join("app/controllers/users_controller.rb");
        std::fs::write(
            &controller,
            "class UsersController < ApplicationController\nend\n",
        )
        .unwrap();

        let analyzer = RailsAnalyzer;
        let routes = analyzer.map_file_to_routes(&controller, &dir);
        assert!(routes.contains(&"/users".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
