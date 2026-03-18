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

pub mod auth;
pub mod batch;
pub mod browser;
pub mod cookie;
#[cfg(feature = "cookie-import")]
pub mod cookie_import;
#[cfg(feature = "cookie-import")]
pub mod detect;
pub mod error;
pub mod network;
pub mod page_map;
pub mod persist;
pub mod selector_cache;
pub mod tab_manager;

pub use auth::{AuthDetection, AuthSignal, LlmAuthChecker};
pub use browser::{AutoAuthStatus, NavigationResult, RayoBrowser, RayoPage, ViewportConfig};
pub use cookie::{
    CookieImportResult, CookieInfo, CookieSetResult, SameSite, SetCookie, matches_domain,
};
pub use error::RayoError;
pub use page_map::BoundingBox;
pub use tab_manager::{TabId, TabInfo, TabManager};
