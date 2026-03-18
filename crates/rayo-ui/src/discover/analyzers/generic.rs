//! Generic (framework-less) analyzer.
//!
//! Fallback when no specific framework is detected. Returns no code routes;
//! the discover orchestrator will rely on browser-only exploration.

use super::{DiscoveredRoute, FrameworkAnalyzer};
use std::path::Path;

pub struct GenericAnalyzer;

impl FrameworkAnalyzer for GenericAnalyzer {
    fn name(&self) -> &str {
        "Generic"
    }

    fn extract_routes(&self, _project_dir: &Path) -> Vec<DiscoveredRoute> {
        // No code analysis possible — rely on browser exploration
        Vec::new()
    }

    fn map_file_to_routes(&self, _file_path: &Path, _project_dir: &Path) -> Vec<String> {
        Vec::new()
    }
}
