//! rayo-rules: AI agent speed rules engine.
//!
//! Detects slow patterns and suggests faster alternatives.
//! Runtime feedback in every MCP response via `_rayo` metadata.

pub mod config;
pub mod defaults;
pub mod engine;

pub use config::RayoRulesConfig;
pub use engine::{RuleEngine, Violation};
