//! Shared utilities for framework analyzers.
//!
//! Extracts common patterns: file discovery, string extraction, route prefix
//! matching, and HTTP method extraction from route prefixes.

use std::path::{Path, PathBuf};

/// Find source files matching given extensions, skipping directories that match
/// any of the skip patterns.
///
/// # Arguments
/// * `project_dir` — root of the project
/// * `extensions` — file extensions to glob for (e.g. `["js", "ts"]`)
/// * `skip_patterns` — substrings in paths to skip (e.g. `["node_modules", "/dist/"]`)
pub fn find_source_files(
    project_dir: &Path,
    extensions: &[&str],
    skip_patterns: &[&str],
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for ext in extensions {
        let pattern = project_dir.join(format!("**/*.{ext}"));
        let pattern_str = pattern.to_string_lossy().to_string();
        if let Ok(entries) = glob::glob(&pattern_str) {
            for entry in entries.flatten() {
                let path_str = entry.to_string_lossy();
                if skip_patterns.iter().any(|skip| path_str.contains(skip)) {
                    continue;
                }
                files.push(entry);
            }
        }
    }
    files
}

/// Extract a quoted string from `text`, handling both single (`'`) and double (`"`)
/// quotes.
///
/// Returns the content between the matching quotes, or `None` if the text
/// does not start with a quote character or no closing quote is found.
///
/// # Arguments
/// * `text` — text to extract from (leading whitespace is trimmed)
pub fn extract_quoted_string(text: &str) -> Option<String> {
    let text = text.trim();
    let quote = text.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &text[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Find the remainder of `haystack` after the first occurrence of `needle`.
///
/// Returns `None` if `needle` is not found.
pub fn find_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.find(needle).map(|i| &haystack[i + needle.len()..])
}

/// Extract an HTTP method from a dotted route prefix like `"app.get("` or
/// `"@router.post("`.
///
/// Splits on `.`, takes the second segment, strips the trailing `(`, and
/// uppercases. Falls back to `"GET"` if parsing fails.
pub fn extract_method_from_prefix(prefix: &str) -> String {
    prefix
        .split('.')
        .nth(1)
        .unwrap_or("get(")
        .strip_suffix('(')
        .unwrap_or("get")
        .to_uppercase()
}

/// A route-prefix entry: the prefix string to match and the default HTTP method.
pub struct RoutePrefix {
    pub prefix: &'static str,
}

/// Match a trimmed line against a list of route prefixes.
///
/// For each prefix that matches, calls `find_after` to get the remainder,
/// then extracts the first quoted string from the remainder. Applies
/// `transform` to the extracted path (e.g. to normalize FastAPI `{id}` params).
///
/// Returns `(method, transformed_path)` pairs for all matches.
pub fn match_route_prefixes(
    trimmed_line: &str,
    prefixes: &[RoutePrefix],
    transform: impl Fn(&str) -> String,
) -> Vec<(String, String)> {
    let mut results = Vec::new();
    for rp in prefixes {
        if let Some(rest) = find_after(trimmed_line, rp.prefix)
            && let Some(path) = extract_quoted_string(rest)
        {
            let method = extract_method_from_prefix(rp.prefix);
            let path = transform(&path);
            results.push((method, path));
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_quoted_string ---

    #[test]
    fn test_extract_quoted_string_single_quotes() {
        assert_eq!(
            extract_quoted_string("'/api/users'"),
            Some("/api/users".into())
        );
    }

    #[test]
    fn test_extract_quoted_string_double_quotes() {
        assert_eq!(extract_quoted_string("\"/login\""), Some("/login".into()));
    }

    #[test]
    fn test_extract_quoted_string_with_leading_whitespace() {
        assert_eq!(
            extract_quoted_string("  '/trimmed'"),
            Some("/trimmed".into())
        );
    }

    #[test]
    fn test_extract_quoted_string_no_quote() {
        assert_eq!(extract_quoted_string("no-quote"), None);
    }

    #[test]
    fn test_extract_quoted_string_empty() {
        assert_eq!(extract_quoted_string(""), None);
    }

    #[test]
    fn test_extract_quoted_string_unclosed() {
        assert_eq!(extract_quoted_string("'unclosed"), None);
    }

    // --- find_after ---

    #[test]
    fn test_find_after_found() {
        assert_eq!(find_after("app.get('/foo')", "app.get("), Some("'/foo')"));
    }

    #[test]
    fn test_find_after_not_found() {
        assert_eq!(find_after("something else", "app.get("), None);
    }

    #[test]
    fn test_find_after_at_start() {
        assert_eq!(find_after("path('x')", "path("), Some("'x')"));
    }

    // --- extract_method_from_prefix ---

    #[test]
    fn test_extract_method_get() {
        assert_eq!(extract_method_from_prefix("app.get("), "GET");
    }

    #[test]
    fn test_extract_method_post() {
        assert_eq!(extract_method_from_prefix("router.post("), "POST");
    }

    #[test]
    fn test_extract_method_delete() {
        assert_eq!(extract_method_from_prefix("@app.delete("), "DELETE");
    }

    #[test]
    fn test_extract_method_decorated() {
        assert_eq!(extract_method_from_prefix("@router.put("), "PUT");
    }

    #[test]
    fn test_extract_method_fallback() {
        assert_eq!(extract_method_from_prefix("nope"), "GET");
    }

    // --- match_route_prefixes ---

    #[test]
    fn test_match_route_prefixes_express_style() {
        let prefixes = vec![
            RoutePrefix { prefix: "app.get(" },
            RoutePrefix {
                prefix: "app.post(",
            },
        ];
        let line = "app.get('/api/users', handler)";
        let results = match_route_prefixes(line, &prefixes, |p| p.to_string());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("GET".to_string(), "/api/users".to_string()));
    }

    #[test]
    fn test_match_route_prefixes_with_transform() {
        let prefixes = vec![RoutePrefix {
            prefix: "@app.get(",
        }];
        let line = "@app.get(\"/items/{id}\")";
        let results =
            match_route_prefixes(line, &prefixes, |p| p.replace('{', ":").replace('}', ""));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("GET".to_string(), "/items/:id".to_string()));
    }

    #[test]
    fn test_match_route_prefixes_no_match() {
        let prefixes = vec![RoutePrefix { prefix: "app.get(" }];
        let line = "something unrelated";
        let results = match_route_prefixes(line, &prefixes, |p| p.to_string());
        assert!(results.is_empty());
    }

    // --- find_source_files ---

    #[test]
    fn test_find_source_files_basic() {
        let dir = std::env::temp_dir().join("rayo_test_shared_find_files");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();

        std::fs::write(dir.join("src/app.js"), "code").unwrap();
        std::fs::write(dir.join("src/util.ts"), "code").unwrap();
        std::fs::write(dir.join("node_modules/pkg/index.js"), "code").unwrap();

        let files = find_source_files(&dir, &["js", "ts"], &["node_modules"]);
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"app.js".to_string()));
        assert!(names.contains(&"util.ts".to_string()));
        assert!(
            !names.contains(&"index.js".to_string()),
            "Should skip node_modules"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_source_files_empty_dir() {
        let dir = std::env::temp_dir().join("rayo_test_shared_find_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let files = find_source_files(&dir, &["py"], &[]);
        assert!(files.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_source_files_multiple_skip_patterns() {
        let dir = std::env::temp_dir().join("rayo_test_shared_find_multi_skip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("venv/lib")).unwrap();
        std::fs::create_dir_all(dir.join("__pycache__")).unwrap();

        std::fs::write(dir.join("src/main.py"), "code").unwrap();
        std::fs::write(dir.join("venv/lib/dep.py"), "code").unwrap();
        std::fs::write(dir.join("__pycache__/cache.py"), "code").unwrap();

        let files = find_source_files(&dir, &["py"], &["venv", "__pycache__"]);
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("main.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
