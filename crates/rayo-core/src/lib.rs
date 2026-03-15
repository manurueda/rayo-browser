//! rayo-core: AI-native browser automation layer
//!
//! Built on chromiumoxide for CDP, adds:
//! - Token-efficient page maps for LLMs
//! - Multi-action batch execution
//! - Selector caching with DOM invalidation
//! - Event-driven waits (not polling)
//!
//! ```text
//! ┌──────────────┐
//! │   rayo-core   │
//! │  page_map     │  ← AI-native features
//! │  batch        │
//! │  cache        │
//! │  wait         │
//! ├──────────────┤
//! │ chromiumoxide │  ← CDP protocol (don't rebuild)
//! └──────────────┘
//! ```

pub mod actions;
pub mod batch;
pub mod browser;
pub mod cookie;
pub mod error;
pub mod page_map;
pub mod selector_cache;
pub mod wait;

pub use browser::{RayoBrowser, RayoPage};
pub use cookie::{CookieInfo, SameSite, SetCookie};
pub use error::RayoError;
