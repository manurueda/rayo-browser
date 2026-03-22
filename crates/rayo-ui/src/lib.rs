//! rayo-ui: AI-native E2E test runner and dashboard for rayo-browser.
//!
//! ```text
//! ┌──────────────────────────────┐
//! │ YAML test files              │
//! │ .rayo/tests/*.test.yaml      │
//! └──────────────┬───────────────┘
//!                │
//! ┌──────────────▼───────────────┐
//! │ rayo-ui runner               │
//! │  loader → executor → assert  │
//! │  → results → report          │
//! └──────────────┬───────────────┘
//!                │
//! ┌──────────────▼───────────────┐
//! │ rayo-core (browser)          │
//! │ rayo-visual (diff engine)    │
//! └─────────────────────────────┘
//! ```

pub mod badge;
pub mod discover;
pub mod error;
pub mod loader;
pub mod narrative;
pub mod persistence;
pub mod report;
pub mod result;
pub mod rundiff;
pub mod runner;
pub mod scan;
pub mod server;
pub mod templates;
pub mod terminal;
pub mod types;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rayo-ui",
    version = env!("CARGO_PKG_VERSION"),
    about = "AI-native E2E test runner and dashboard for rayo-browser"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run test suites
    Run {
        /// Path to test files directory
        #[arg(short, long, default_value = ".rayo/tests")]
        tests_dir: PathBuf,

        /// Path to baselines directory
        #[arg(short, long, default_value = ".rayo/baselines")]
        baselines_dir: PathBuf,

        /// Specific suite name to run (runs all if not specified)
        #[arg(short, long)]
        suite: Option<String>,

        /// Base URL for relative navigate paths (e.g. http://localhost:3000).
        /// Overrides .rayo/config.yaml. Env var RAYO_BASE_URL takes highest priority.
        #[arg(long)]
        base_url: Option<String>,

        /// Output JSON report to file
        #[arg(long)]
        json: Option<PathBuf>,

        /// Output HTML report to file
        #[arg(long)]
        html: Option<PathBuf>,

        /// Abort suite on first failure
        #[arg(long)]
        abort_on_failure: bool,

        /// Verbose output (show all steps, assertions, page maps)
        #[arg(short, long)]
        verbose: bool,
    },

    /// List available test suites
    List {
        /// Path to test files directory
        #[arg(short, long, default_value = ".rayo/tests")]
        tests_dir: PathBuf,
    },

    /// Start the dashboard (opens browser automatically)
    Ui {
        /// Path to test files directory
        #[arg(short, long, default_value = ".rayo/tests")]
        tests_dir: PathBuf,

        /// Path to baselines directory
        #[arg(short, long, default_value = ".rayo/baselines")]
        baselines_dir: PathBuf,

        /// Base URL for relative navigate paths (e.g. http://localhost:3000).
        /// Overrides .rayo/config.yaml. Env var RAYO_BASE_URL takes highest priority.
        #[arg(long)]
        base_url: Option<String>,

        /// Server port
        #[arg(short, long, default_value = "4040")]
        port: u16,

        /// Don't open browser automatically
        #[arg(long)]
        no_open: bool,
    },

    /// Scan an app: discover flows, test them, show results
    Scan {
        /// Target URL (e.g., http://localhost:3000)
        url: String,

        /// Project root directory for code analysis
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,

        /// Open dashboard in browser after scan
        #[arg(long)]
        open: bool,

        /// Generate static HTML report
        #[arg(long)]
        report: Option<PathBuf>,

        /// Generate SVG badge
        #[arg(long)]
        badge: Option<PathBuf>,

        /// Only discover routes affected by current branch diff
        #[arg(long)]
        diff: bool,

        /// Maximum pages to explore
        #[arg(long, default_value = "50")]
        max_pages: usize,
    },

    /// Auto-discover user flows and generate test files
    Discover {
        /// Target URL (e.g., http://localhost:3000)
        url: String,

        /// Project root directory for code analysis
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,

        /// Output directory for generated tests
        #[arg(short, long, default_value = ".rayo/tests")]
        tests_dir: PathBuf,

        /// Baselines directory
        #[arg(short, long, default_value = ".rayo/baselines")]
        baselines_dir: PathBuf,

        /// Only discover routes affected by current branch diff
        #[arg(long)]
        diff: bool,

        /// Overwrite existing test files
        #[arg(long)]
        force: bool,

        /// Maximum pages to explore
        #[arg(long, default_value = "50")]
        max_pages: usize,
    },
}

/// Entry point for the rayo-ui binary.
///
/// Parses CLI arguments, initializes tracing, and dispatches to the
/// appropriate subcommand (run, list, or ui).
pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rayo_ui=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            tests_dir,
            baselines_dir,
            suite,
            base_url,
            json,
            html,
            abort_on_failure,
            verbose,
        } => {
            let files = crate::loader::load_suites(&tests_dir)?;

            // Resolve base_url: env var > CLI flag > config file
            let rayo_config_dir = if tests_dir.ends_with("tests") {
                tests_dir
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".rayo"))
            } else {
                std::path::Path::new(".rayo")
            };
            let file_config = crate::loader::load_config(rayo_config_dir);
            let resolved_base_url = std::env::var("RAYO_BASE_URL")
                .ok()
                .or(base_url)
                .or(file_config.base_url);

            if let Some(ref url) = resolved_base_url {
                eprintln!("  base_url: {url}");
            }

            let config = crate::runner::RunnerConfig {
                baselines_dir,
                abort_on_failure,
                base_url: resolved_base_url,
            };

            let suites_to_run: Vec<_> = if let Some(ref name) = suite {
                files.iter().filter(|f| f.suite.name == *name).collect()
            } else {
                files.iter().collect()
            };

            if suites_to_run.is_empty() {
                if let Some(name) = suite {
                    anyhow::bail!("Suite '{name}' not found");
                } else {
                    anyhow::bail!("No test suites found in {}", tests_dir.display());
                }
            }

            let mut all_results = Vec::new();

            for file in &suites_to_run {
                eprint!("  Running: {}...", file.suite.name);
                let result = crate::runner::run_suite(&file.suite, &config, None).await?;
                let icon = if result.pass {
                    "\x1b[32m\u{2713}\x1b[0m"
                } else {
                    "\x1b[31m\u{2717}\x1b[0m"
                };
                eprintln!(" {icon}");
                all_results.push(result);
            }

            // Rich summary
            crate::terminal::print_run_summary(&all_results, verbose);

            // Write reports
            if let Some(json_path) = json {
                for result in &all_results {
                    crate::report::write_json_report(result, &json_path)?;
                }
                eprintln!("  JSON report: {}", json_path.display());
            }

            if let Some(html_path) = html {
                for result in &all_results {
                    let html_content = crate::report::generate_html_report(result);
                    std::fs::write(&html_path, html_content)?;
                }
                eprintln!("  HTML report: {}", html_path.display());
            }

            let all_passed = all_results.iter().all(|r| r.pass);
            if !all_passed {
                std::process::exit(1);
            }
        }

        Commands::List { tests_dir } => match crate::loader::load_suites(&tests_dir) {
            Ok(files) => {
                println!("\nTest suites in {}:\n", tests_dir.display());
                for file in &files {
                    println!("  {} ({} steps)", file.suite.name, file.suite.steps.len());
                }
                println!("\n  {} suite(s) found", files.len());
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        },

        Commands::Ui {
            tests_dir,
            baselines_dir,
            base_url,
            port,
            no_open,
        } => {
            // Resolve base_url: env var > CLI flag > config file
            let rayo_config_dir = if tests_dir.ends_with("tests") {
                tests_dir
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".rayo"))
            } else {
                std::path::Path::new(".rayo")
            };
            let file_config = crate::loader::load_config(rayo_config_dir);
            let resolved_base_url = std::env::var("RAYO_BASE_URL")
                .ok()
                .or(base_url)
                .or(file_config.base_url);

            crate::server::start_server(
                tests_dir,
                baselines_dir,
                port,
                !no_open,
                resolved_base_url,
            )
            .await?;
        }

        Commands::Scan {
            url,
            project_dir,
            open,
            report,
            badge,
            diff,
            max_pages,
        } => {
            let scan_start = std::time::Instant::now();
            let tests_dir = PathBuf::from(".rayo/tests");
            let baselines_dir = PathBuf::from(".rayo/baselines");

            // Phase 1-3: Discover
            println!("\n  \x1b[1m\u{26a1} rayo scan\x1b[0m");
            println!("  =========");

            let discover_config = crate::discover::DiscoverConfig {
                url: url.clone(),
                project_dir,
                tests_dir: tests_dir.clone(),
                baselines_dir: baselines_dir.clone(),
                diff_mode: diff,
                force: true, // always overwrite during scan
                max_pages,
            };

            let discover_result = crate::discover::discover(discover_config).await?;

            // Phase 4: Run flows through generated test suites
            println!("\n  Phase 4: Running discovered flows...");
            let flow_results =
                crate::scan::run_scan(&url, &discover_result, &tests_dir, &baselines_dir).await;

            // Phase 5: Build ScanResult
            let total_flows = flow_results.len();
            let passed_flows = flow_results.iter().filter(|f| f.passed).count();
            let failed_flows = total_flows - passed_flows;

            // Compute health score: weighted by importance
            let health_score = if total_flows == 0 {
                discover_result.health_score
            } else {
                let total_weight: u32 = flow_results
                    .iter()
                    .map(|f| importance_weight(&f.importance))
                    .sum();
                let passed_weight: u32 = flow_results
                    .iter()
                    .filter(|f| f.passed)
                    .map(|f| importance_weight(&f.importance))
                    .sum();
                if total_weight > 0 {
                    ((passed_weight as f64 / total_weight as f64) * 100.0) as u32
                } else {
                    discover_result.health_score
                }
            };

            let scan_duration_ms = scan_start.elapsed().as_millis() as u64;
            let total_duration_ms: u64 = flow_results.iter().map(|f| f.duration_ms).sum();

            let scan_result = crate::persistence::ScanResult {
                url: url.clone(),
                framework: discover_result.framework.clone(),
                health_score,
                total_flows,
                passed_flows,
                failed_flows,
                total_duration_ms,
                scan_duration_ms,
                console_errors: discover_result.console_errors as u32,
                timestamp: chrono::Utc::now().to_rfc3339(),
                flows: flow_results,
            };

            // Phase 6: Save run
            let run_dir = crate::persistence::save_run(&scan_result, std::path::Path::new("."))?;

            // Phase 7: Compare with previous run
            let scan_diff =
                crate::rundiff::compare_with_latest(&scan_result, std::path::Path::new("."));

            // Phase 8: Terminal summary
            crate::terminal::print_scan_summary(&scan_result, scan_diff.as_ref(), &url);

            // Phase 9: Report & badge
            if let Some(report_path) = report {
                let mut scan_for_report = scan_result.clone();
                crate::report::inline_screenshots(&mut scan_for_report, &run_dir);
                let html =
                    crate::report::generate_scan_report(&scan_for_report, scan_diff.as_ref());
                std::fs::write(&report_path, html)?;
                eprintln!("  Report: {}", report_path.display());
            }
            if let Some(badge_path) = badge {
                crate::badge::save_badge(scan_result.health_score, &badge_path)?;
                eprintln!("  Badge: {}", badge_path.display());
            }

            // Phase 10: System sound
            #[cfg(target_os = "macos")]
            {
                let sound = if scan_result.failed_flows == 0 {
                    "Glass"
                } else {
                    "Basso"
                };
                let _ = std::process::Command::new("afplay")
                    .arg(format!("/System/Library/Sounds/{sound}.aiff"))
                    .spawn();
            }

            // Phase 11: Open dashboard if requested
            if open {
                let _ = open::that(format!("file://{}", run_dir.join("result.json").display()));
            }

            if scan_result.failed_flows > 0 {
                std::process::exit(1);
            }
        }

        Commands::Discover {
            url,
            project_dir,
            tests_dir,
            baselines_dir,
            diff,
            force,
            max_pages,
        } => {
            let config = crate::discover::DiscoverConfig {
                url,
                project_dir,
                tests_dir,
                baselines_dir,
                diff_mode: diff,
                force,
                max_pages,
            };

            println!("\n  rayo-ui discover");
            println!("  ================");

            let result = crate::discover::discover(config).await?;

            println!("\n  Results");
            println!("  -------");
            println!("  Framework:       {}", result.framework);
            println!("  Routes (code):   {}", result.routes_from_code);
            println!("  Routes explored: {}", result.routes_explored);
            println!("  Flows detected:  {}", result.flows_detected);
            println!("  Tests generated: {}", result.tests_generated);
            println!("  Console errors:  {}", result.console_errors);
            println!("  Health score:    {}%", result.health_score);
            println!("  Duration:        {}ms", result.duration_ms);
        }
    }

    Ok(())
}

/// Weight factor for importance levels (used in health score calculation).
fn importance_weight(importance: &str) -> u32 {
    match importance {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 1,
    }
}
