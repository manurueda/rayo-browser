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

pub mod error;
pub mod loader;
pub mod report;
pub mod result;
pub mod runner;
pub mod server;
pub mod types;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "rayo-ui",
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

            let mut all_passed = true;
            let mut all_results = Vec::new();

            for file in &suites_to_run {
                println!("\n  Running: {}", file.suite.name);
                let result = crate::runner::run_suite(&file.suite, &config, None).await?;

                for step in &result.steps {
                    let icon = if step.pass {
                        "\x1b[32m\u{2713}\x1b[0m"
                    } else {
                        "\x1b[31m\u{2717}\x1b[0m"
                    };
                    println!("    {icon} {} ({}ms)", step.name, step.duration_ms);
                    if let Some(ref err) = step.error {
                        println!("      \x1b[31m{err}\x1b[0m");
                    }
                    for a in &step.assertions {
                        let a_icon = if a.pass {
                            "\x1b[32m\u{2713}\x1b[0m"
                        } else {
                            "\x1b[31m\u{2717}\x1b[0m"
                        };
                        print!("      {a_icon} {}", a.assertion_type);
                        if let Some(ref msg) = a.message {
                            print!(" -- {msg}");
                        }
                        println!();
                    }
                }

                if !result.pass {
                    all_passed = false;
                }

                println!(
                    "\n  {} {}/{} steps passed ({}ms)",
                    if result.pass {
                        "\x1b[32mPASS\x1b[0m"
                    } else {
                        "\x1b[31mFAIL\x1b[0m"
                    },
                    result.passed_steps,
                    result.total_steps,
                    result.duration_ms,
                );

                all_results.push(result);
            }

            // Write reports
            if let Some(json_path) = json {
                for result in &all_results {
                    crate::report::write_json_report(result, &json_path)?;
                }
                println!("\n  JSON report: {}", json_path.display());
            }

            if let Some(html_path) = html {
                for result in &all_results {
                    let html_content = crate::report::generate_html_report(result);
                    std::fs::write(&html_path, html_content)?;
                }
                println!("  HTML report: {}", html_path.display());
            }

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
    }

    Ok(())
}
