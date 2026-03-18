//! Askama template context structs for the rayo-ui dashboard.

use askama::Template;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::result::{StepResult, SuiteResult};

/// Blanket helper: render any askama Template into an axum Response.
pub struct HtmlTemplate<T: Template>(pub T);

impl<T: Template> IntoResponse for HtmlTemplate<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => {
                ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Template error: {e}"),
            )
                .into_response(),
        }
    }
}

// ---------------------------------------------------------------------------
// Page templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub stats: StatsData,
    pub results: Vec<SuiteResult>,
    pub suites: Vec<SuiteSummary>,
}

#[derive(Template)]
#[template(path = "pages/suites.html")]
pub struct SuitesTemplate {
    pub suites: Vec<SuiteSummary>,
    pub results: Vec<SuiteResult>,
}

#[derive(Template)]
#[template(path = "pages/suite_detail.html")]
pub struct SuiteDetailTemplate {
    pub result: SuiteResult,
}

#[derive(Template)]
#[template(path = "pages/live.html")]
pub struct LiveTemplate;

#[derive(Template)]
#[template(path = "pages/welcome.html")]
pub struct WelcomeTemplate {
    pub chrome_available: bool,
    pub discovering: bool,
    pub tests_dir: String,
}

// ---------------------------------------------------------------------------
// Fragment templates (HTML partials for htmx)
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "fragments/stats.html")]
pub struct StatsFragment {
    pub stats: StatsData,
}

#[derive(Template)]
#[template(path = "fragments/suite_card.html")]
pub struct SuiteCardFragment {
    pub result: SuiteResult,
}

#[derive(Template)]
#[template(path = "fragments/suite_list.html")]
pub struct SuiteListFragment {
    pub results: Vec<SuiteResult>,
}

#[derive(Template)]
#[template(path = "fragments/step_list.html")]
pub struct StepListFragment {
    pub steps: Vec<StepResult>,
    pub suite_name: String,
}

#[derive(Template)]
#[template(path = "fragments/available_suites.html")]
pub struct AvailableSuitesFragment {
    pub suites: Vec<SuiteSummary>,
}

#[derive(Template)]
#[template(path = "fragments/live_event.html")]
pub struct LiveEventFragment {
    pub step: StepResult,
    pub index: usize,
}

#[derive(Template)]
#[template(path = "fragments/live_progress.html")]
pub struct LiveProgressFragment {
    pub suite_name: String,
    pub current: usize,
    pub total: usize,
    pub percent: f64,
}

#[derive(Template)]
#[template(path = "fragments/error.html")]
pub struct ErrorFragment {
    pub message: String,
}

// ---------------------------------------------------------------------------
// Shared data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StatsData {
    pub pass_rate: f64,
    pub passed_suites: usize,
    pub total_suites: usize,
    pub total_steps: usize,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SuiteSummary {
    pub name: String,
    pub path: String,
    pub steps: usize,
    pub has_setup: bool,
    pub has_teardown: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl StatsData {
    pub fn from_results(results: &[SuiteResult]) -> Self {
        if results.is_empty() {
            return Self::default();
        }
        let total_suites = results.len();
        let passed_suites = results.iter().filter(|r| r.pass).count();
        let pass_rate = if total_suites > 0 {
            (passed_suites as f64 / total_suites as f64) * 100.0
        } else {
            0.0
        };
        let total_steps: usize = results.iter().map(|r| r.total_steps).sum();
        let total_duration_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
        Self {
            pass_rate,
            passed_suites,
            total_suites,
            total_steps,
            total_duration_ms,
        }
    }
}

/// Format duration: "1.2s" for >1000ms, "150ms" otherwise.
pub fn fmt_duration(ms: &u64) -> String {
    let ms = *ms;
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}
