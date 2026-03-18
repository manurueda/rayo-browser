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
