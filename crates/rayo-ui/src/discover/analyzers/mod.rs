//! Framework detection and route extraction.
//!
//! Each analyzer detects a specific framework (Next.js, Express, Rails, etc.)
//! and extracts route definitions from source code using pattern matching.

pub mod django;
pub mod express;
pub mod fastapi;
pub mod generic;
pub mod nextjs;
pub mod rails;
pub mod static_html;

use std::path::Path;

/// A route discovered from source code analysis.
#[derive(Debug, Clone)]
pub struct DiscoveredRoute {
    /// URL path, e.g. "/login", "/api/users".
    pub path: String,
    /// HTTP method: GET, POST, etc.
    pub method: String,
    /// Source file where this route was found.
    pub source_file: String,
    /// Whether code analysis hints at a form on this page.
    pub has_form: bool,
    /// Whether this is an API endpoint (not a user-facing page).
    pub is_api: bool,
}

/// Trait for framework-specific route extraction.
pub trait FrameworkAnalyzer: Send + Sync {
    /// Human-readable framework name.
    fn name(&self) -> &str;

    /// Extract all routes from the project directory.
    fn extract_routes(&self, project_dir: &Path) -> Vec<DiscoveredRoute>;

    /// Given a changed file, return the routes it maps to.
    fn map_file_to_routes(&self, file_path: &Path, project_dir: &Path) -> Vec<String>;
}

/// Auto-detect the framework used in a project and return the appropriate analyzer.
pub fn detect_framework(project_dir: &Path) -> Box<dyn FrameworkAnalyzer> {
    // Check in order of specificity
    if nextjs::NextJsAnalyzer::detect(project_dir) {
        return Box::new(nextjs::NextJsAnalyzer);
    }
    if express::ExpressAnalyzer::detect(project_dir) {
        return Box::new(express::ExpressAnalyzer);
    }
    if rails::RailsAnalyzer::detect(project_dir) {
        return Box::new(rails::RailsAnalyzer);
    }
    if django::DjangoAnalyzer::detect(project_dir) {
        return Box::new(django::DjangoAnalyzer);
    }
    if fastapi::FastApiAnalyzer::detect(project_dir) {
        return Box::new(fastapi::FastApiAnalyzer);
    }
    if static_html::StaticHtmlAnalyzer::detect(project_dir) {
        return Box::new(static_html::StaticHtmlAnalyzer);
    }
    Box::new(generic::GenericAnalyzer)
}
