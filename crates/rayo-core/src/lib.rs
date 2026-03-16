//! rayo-core: AI-native browser automation layer
//!
//! Built on chromiumoxide for CDP, adds:
//! - Token-efficient page maps for LLMs
//! - Multi-action batch execution
//! - Selector caching with DOM invalidation
//!
//! ```text
//! ┌──────────────┐
//! │   rayo-core   │
//! │  page_map     │  ← AI-native features
//! │  batch        │
//! │  cache        │
//! ├──────────────┤
//! │ chromiumoxide │  ← CDP protocol (don't rebuild)
//! └──────────────┘
//! ```

pub mod batch;
pub mod browser;
pub mod cookie;
#[cfg(feature = "cookie-import")]
pub mod cookie_import;
pub mod error;
pub mod network;
pub mod page_map;
pub mod selector_cache;
pub mod tab_manager;

pub use browser::{RayoBrowser, RayoPage};
pub use cookie::{CookieInfo, SameSite, SetCookie};
pub use error::RayoError;
pub use tab_manager::{TabId, TabInfo, TabManager};
