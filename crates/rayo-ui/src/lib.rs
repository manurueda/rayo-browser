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

pub mod discover;
pub mod error;
pub mod loader;
pub mod report;
pub mod result;
pub mod runner;
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

        /// Server port
        #[arg(short, long, default_value = "4040")]
        port: u16,

        /// Don't open browser automatically
        #[arg(long)]
        no_open: bool,
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
            json,
            html,
            abort_on_failure,
            verbose,
        } => {
            let files = crate::loader::load_suites(&tests_dir)?;

            let config = crate::runner::RunnerConfig {
                baselines_dir,
                abort_on_failure,
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
            port,
            no_open,
        } => {
            crate::server::start_server(tests_dir, baselines_dir, port, !no_open).await?;
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
