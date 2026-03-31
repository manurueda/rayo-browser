//! Tests for rayo-ui discover diff module — route mapping, deduplication.

use rayo_ui::discover::diff;
use std::path::Path;

#[test]
fn test_map_files_to_routes_preserves_order() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_order");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("zebra.html"), "<html></html>").unwrap();
    std::fs::write(dir.join("alpha.html"), "<html></html>").unwrap();
    std::fs::write(dir.join("middle.html"), "<html></html>").unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let changed_files = vec![
        dir.join("zebra.html"),
        dir.join("alpha.html"),
        dir.join("middle.html"),
    ];

    let routes = diff::map_files_to_routes(&changed_files, &analyzer, &dir);
    assert_eq!(routes.len(), 3);
    assert_eq!(routes[0], "/zebra");
    assert_eq!(routes[1], "/alpha");
    assert_eq!(routes[2], "/middle");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_map_files_to_routes_mixed_html_and_non_html() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_mixed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("about.html"), "<html></html>").unwrap();
    std::fs::write(dir.join("style.css"), "body {}").unwrap();
    std::fs::write(dir.join("app.js"), "console.log('hi')").unwrap();
    std::fs::write(dir.join("contact.html"), "<html></html>").unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let changed_files = vec![
        dir.join("about.html"),
        dir.join("style.css"),
        dir.join("app.js"),
        dir.join("contact.html"),
    ];

    let routes = diff::map_files_to_routes(&changed_files, &analyzer, &dir);
    assert_eq!(routes.len(), 2, "Only HTML files should produce routes");
    assert!(routes.contains(&"/about".to_string()));
    assert!(routes.contains(&"/contact".to_string()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_map_files_to_routes_index_html() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_index");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("index.html"), "<html></html>").unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let changed_files = vec![dir.join("index.html")];

    let routes = diff::map_files_to_routes(&changed_files, &analyzer, &dir);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0], "/");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_map_files_to_routes_deduplicates_same_file() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_dedup2");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("index.html"), "<html></html>").unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let changed_files = vec![dir.join("index.html"), dir.join("index.html")];

    let routes = diff::map_files_to_routes(&changed_files, &analyzer, &dir);
    assert_eq!(routes.len(), 1, "Duplicate routes should be deduplicated");
    assert_eq!(routes[0], "/");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_map_files_to_routes_empty() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_empty2");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let routes = diff::map_files_to_routes(&[], &analyzer, &dir);
    assert!(routes.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_get_changed_files_non_git_dir_returns_error() {
    let dir = std::env::temp_dir().join("rayo_itest_diff_not_git");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let result = diff::get_changed_files(&dir);
    assert!(
        result.is_err(),
        "get_changed_files in a non-git directory should return an error"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_get_changed_files_in_git_repo() {
    // Run in the actual repo directory -- should succeed
    let result = diff::get_changed_files(Path::new("."));
    assert!(
        result.is_ok(),
        "get_changed_files should succeed in a real git repo"
    );
    let files = result.unwrap();
    for f in &files {
        assert!(
            !f.to_string_lossy().is_empty(),
            "File paths should not be empty"
        );
    }
}

#[test]
fn test_map_files_to_routes_only_non_html_produces_empty() {
    use rayo_ui::discover::analyzers::static_html::StaticHtmlAnalyzer;

    let dir = std::env::temp_dir().join("rayo_itest_diff_nonhtml2");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("readme.md"), "# Hello").unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();

    let analyzer = StaticHtmlAnalyzer;
    let changed_files = vec![dir.join("readme.md"), dir.join("Cargo.toml")];

    let routes = diff::map_files_to_routes(&changed_files, &analyzer, &dir);
    assert!(routes.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
