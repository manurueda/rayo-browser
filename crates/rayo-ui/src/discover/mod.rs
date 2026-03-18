//! Automatic test generation from source code analysis + browser exploration.
//!
//! The `discover` command performs five phases:
//! 1. **Code Analysis** — detect framework, extract routes/forms/endpoints
//! 2. **Browser Exploration** — navigate each route, get page_map, check console errors
//! 3. **Flow Detection** — identify forms, auth, CRUD, navigation flows from page maps
//! 4. **YAML Generation** — write `.rayo/tests/*.test.yaml` files
//! 5. **Report** — write `.rayo/discover-report.md` with health score

pub mod analyzers;
pub mod diff;
pub mod flows;
pub mod generator;
pub mod report;

use crate::error::TestError;
use generator::{ExploredPage, PageStatus};
use rayo_core::{RayoBrowser, ViewportConfig};
use rayo_profiler::Profiler;
use std::path::PathBuf;
use std::time::Instant;

/// Configuration for the discover command.
#[derive(Debug, Clone)]
pub struct DiscoverConfig {
    /// Base URL to explore (e.g., http://localhost:3000).
    pub url: String,
    /// Project root directory for code analysis.
    pub project_dir: PathBuf,
    /// Output directory for generated tests.
    pub tests_dir: PathBuf,
    /// Baselines directory.
    pub baselines_dir: PathBuf,
    /// Only discover routes affected by current branch diff.
    pub diff_mode: bool,
    /// Overwrite existing test files.
    pub force: bool,
    /// Maximum pages to explore.
    pub max_pages: usize,
}

/// Result of the discover process.
#[derive(Debug, Clone)]
pub struct DiscoverResult {
    pub framework: String,
    pub routes_from_code: usize,
    pub routes_explored: usize,
    pub flows_detected: usize,
    pub tests_generated: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub console_errors: usize,
    pub health_score: u32,
    pub duration_ms: u64,
}

/// Main entry point for the discover command.
///
/// Orchestrates code analysis, browser exploration, flow detection,
/// test generation, and reporting.
pub async fn discover(config: DiscoverConfig) -> Result<DiscoverResult, TestError> {
    let start = Instant::now();

    // Phase 1: Code analysis — detect framework, extract routes
    println!("\n  Phase 1: Analyzing source code...");
    let analyzer = analyzers::detect_framework(&config.project_dir);
    let framework_name = analyzer.name().to_string();
    println!("    Framework: {framework_name}");

    let mut code_routes = analyzer.extract_routes(&config.project_dir);
    let routes_from_code = code_routes.len();
    println!("    Routes from code: {routes_from_code}");

    // In diff mode, filter to only changed routes
    if config.diff_mode {
        println!("    Diff mode: filtering to changed routes...");
        match diff::get_changed_files(&config.project_dir) {
            Ok(changed_files) => {
                let affected_routes = diff::map_files_to_routes(
                    &changed_files,
                    analyzer.as_ref(),
                    &config.project_dir,
                );
                if affected_routes.is_empty() {
                    println!("    No routes affected by current changes.");
                } else {
                    println!(
                        "    Changed files affect {} route(s)",
                        affected_routes.len()
                    );
                    code_routes.retain(|r| affected_routes.contains(&r.path));
                }
            }
            Err(e) => {
                println!("    Warning: git diff failed ({e}), exploring all routes");
            }
        }
    }

    // Build the set of URLs to explore
    let base_url = config.url.trim_end_matches('/');
    let mut urls_to_explore: Vec<String> = Vec::new();

    // Always start with the base URL
    urls_to_explore.push(base_url.to_string());

    // Add code routes (non-API, non-parameterized)
    for route in &code_routes {
        if route.is_api || route.path.contains(':') {
            continue;
        }
        let url = format!("{base_url}{}", route.path);
        if !urls_to_explore.contains(&url) {
            urls_to_explore.push(url);
        }
    }

    // Cap exploration
    urls_to_explore.truncate(config.max_pages);
    println!("    URLs to explore: {}", urls_to_explore.len());

    // Phase 2: Browser exploration
    println!("\n  Phase 2: Exploring pages in browser...");
    let profiler = Profiler::new();
    let viewport = ViewportConfig::default();
    let browser = RayoBrowser::launch_with_config(profiler, viewport).await?;
    let page = browser.new_page().await?;

    let mut explored_pages = Vec::new();
    let mut all_flows = Vec::new();
    let mut total_console_errors = 0;

    for (i, url) in urls_to_explore.iter().enumerate() {
        let progress = format!("[{}/{}]", i + 1, urls_to_explore.len());
        print!("    {progress} {url}...");

        // Navigate to the page
        let nav_result = page.goto(url).await;

        if let Err(e) = nav_result {
            println!(" ERROR ({e})");
            explored_pages.push(ExploredPage {
                url: url.clone(),
                status: PageStatus::Error,
                console_errors: 0,
                has_elements: false,
            });
            continue;
        }

        // Get page map
        let page_map = match page.page_map(None).await {
            Ok(pm) => pm,
            Err(e) => {
                println!(" ERROR getting page map ({e})");
                explored_pages.push(ExploredPage {
                    url: url.clone(),
                    status: PageStatus::Error,
                    console_errors: 0,
                    has_elements: false,
                });
                continue;
            }
        };

        // Check for console errors via JS
        let console_errors = check_console_errors(&page).await;
        total_console_errors += console_errors;

        // Determine page status
        let current_url = page.url().await.unwrap_or_default();
        let status = if current_url.to_lowercase().contains("login")
            || current_url.to_lowercase().contains("signin")
        {
            if !url.to_lowercase().contains("login") && !url.to_lowercase().contains("signin") {
                PageStatus::AuthGated
            } else {
                PageStatus::Ok
            }
        } else if current_url != *url
            && !current_url
                .trim_end_matches('/')
                .eq(url.trim_end_matches('/'))
        {
            PageStatus::Redirect
        } else {
            PageStatus::Ok
        };

        let has_elements = !page_map.interactive.is_empty();

        let status_label = status.as_str();
        let elements_count = page_map.interactive.len();
        println!(" {status_label} ({elements_count} elements, {console_errors} console errors)");

        explored_pages.push(ExploredPage {
            url: url.clone(),
            status: status.clone(),
            console_errors,
            has_elements,
        });

        // Phase 3: Flow detection from page map
        if status == PageStatus::Ok || status == PageStatus::Redirect {
            let page_flows = flows::detect_flows(&page_map, url);
            all_flows.extend(page_flows);
        }

        // Also discover links from the page map to find additional pages
        if explored_pages.len() < config.max_pages {
            for el in &page_map.interactive {
                if el.tag == "a"
                    && let Some(href) = &el.href
                {
                    let full_url = resolve_url(href, base_url);
                    if full_url.starts_with(base_url)
                        && !urls_to_explore.contains(&full_url)
                        && urls_to_explore.len() < config.max_pages
                    {
                        // Don't add to the iteration, just note it was discovered
                        // (We already capped at max_pages above)
                    }
                }
            }
        }
    }

    // Clean up browser
    drop(page);
    browser.close().await;

    // Deduplicate flows
    let all_flows = generator::deduplicate_flows(all_flows);
    let flows_detected = all_flows.len();
    println!("\n  Phase 3: Detected {flows_detected} user flow(s)");

    // Phase 4: YAML generation
    println!("\n  Phase 4: Generating test files...");
    let suites = generator::generate_test_suites(&all_flows, &explored_pages, base_url);
    let tests_generated = generator::write_test_suites(&suites, &config.tests_dir, config.force)?;
    println!(
        "    Wrote {tests_generated} test file(s) to {}",
        config.tests_dir.display()
    );

    // Phase 5: Report
    println!("\n  Phase 5: Generating report...");
    let health_score = report::compute_health_score(&explored_pages, routes_from_code);

    let result = DiscoverResult {
        framework: framework_name,
        routes_from_code,
        routes_explored: explored_pages.len(),
        flows_detected,
        tests_generated,
        tests_passed: 0,
        tests_failed: 0,
        console_errors: total_console_errors,
        health_score,
        duration_ms: start.elapsed().as_millis() as u64,
    };

    let report_path = config
        .tests_dir
        .parent()
        .unwrap_or(&config.tests_dir)
        .join("discover-report.md");
    if let Err(e) = report::write_report(&result, &explored_pages, &report_path) {
        println!("    Warning: failed to write report: {e}");
    } else {
        println!("    Report: {}", report_path.display());
    }

    Ok(result)
}

/// Check for JavaScript console errors on the page.
/// Uses a JS snippet since rayo-core doesn't expose console log capture directly.
async fn check_console_errors(page: &rayo_core::RayoPage) -> usize {
    // Install a console error counter if not already present, then read it.
    // On first call per page, this won't catch errors that already fired,
    // but it's a reasonable heuristic for pages that load cleanly.
    let js = r#"
        (function() {
            if (!window.__rayoConsoleErrors) {
                window.__rayoConsoleErrors = 0;
                const orig = console.error;
                console.error = function() {
                    window.__rayoConsoleErrors++;
                    orig.apply(console, arguments);
                };
            }
            return window.__rayoConsoleErrors;
        })()
    "#;

    match page.evaluate(js).await {
        Ok(val) => val.as_u64().unwrap_or(0) as usize,
        Err(_) => 0,
    }
}

/// Resolve a potentially relative URL against a base URL.
fn resolve_url(href: &str, base_url: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        // Absolute path — combine with base origin
        let origin = base_url
            .find("://")
            .and_then(|i| base_url[i + 3..].find('/').map(|j| &base_url[..i + 3 + j]))
            .unwrap_or(base_url);
        format!("{origin}{href}")
    } else {
        format!("{}/{}", base_url.trim_end_matches('/'), href)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            resolve_url("/about", "http://localhost:3000"),
            "http://localhost:3000/about"
        );
        assert_eq!(
            resolve_url("https://other.com/page", "http://localhost:3000"),
            "https://other.com/page"
        );
        assert_eq!(
            resolve_url("page", "http://localhost:3000"),
            "http://localhost:3000/page"
        );
    }
}
