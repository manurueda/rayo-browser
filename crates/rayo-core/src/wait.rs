//! Event-driven waits — NOT polling.
//!
//! Wait implementation uses a MutationObserver-based Promise that
//! resolves instantly when the target element appears in the DOM.
//! Saves 100-500ms per wait operation vs Playwright's polling approach.
