//! Multi-tab manager for rayo-browser.
//!
//! Replaces the single `Arc<Mutex<Option<RayoPage>>>` pattern with a proper
//! multi-tab architecture. Each tab has a string ID and holds a `RayoPage`.
//! There is always an active tab (when at least one tab exists).

use std::collections::HashMap;

use serde::Serialize;

use crate::browser::RayoPage;

/// Unique identifier for a tab.
pub type TabId = String;

/// Serializable tab metadata for MCP responses.
#[derive(Debug, Clone, Serialize)]
pub struct TabInfo {
    /// The tab's unique identifier.
    pub id: TabId,
    /// Current URL of the tab.
    pub url: String,
    /// Page title of the tab.
    pub title: String,
    /// Whether this is the currently active tab.
    pub is_active: bool,
}

/// Manages multiple browser tabs, each identified by a `TabId`.
///
/// Tracks which tab is active and provides access to individual pages.
pub struct TabManager {
    tabs: HashMap<TabId, RayoPage>,
    active_tab: Option<TabId>,
}

impl TabManager {
    /// Create an empty tab manager with no tabs.
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            active_tab: None,
        }
    }

    /// Add a tab and make it the active tab.
    pub fn add_tab(&mut self, id: TabId, page: RayoPage) {
        self.active_tab = Some(id.clone());
        self.tabs.insert(id, page);
    }

    /// Remove a tab by ID, returning the `RayoPage` if it existed.
    ///
    /// If the removed tab was active, switches to an arbitrary remaining tab.
    pub fn remove_tab(&mut self, id: &str) -> Option<RayoPage> {
        let page = self.tabs.remove(id);
        if page.is_some() && self.active_tab.as_deref() == Some(id) {
            // Switch to any remaining tab
            self.active_tab = self.tabs.keys().next().cloned();
        }
        page
    }

    /// Get an immutable reference to the active tab's page.
    pub fn active_page(&self) -> Option<&RayoPage> {
        self.active_tab.as_ref().and_then(|id| self.tabs.get(id))
    }

    /// Get a mutable reference to the active tab's page.
    pub fn active_page_mut(&mut self) -> Option<&mut RayoPage> {
        self.active_tab
            .as_ref()
            .and_then(|id| self.tabs.get_mut(id))
    }

    /// Get an immutable reference to a specific tab's page.
    pub fn get_page(&self, id: &str) -> Option<&RayoPage> {
        self.tabs.get(id)
    }

    /// Set the active tab by ID. Returns `false` if the tab doesn't exist.
    pub fn set_active(&mut self, id: &str) -> bool {
        if self.tabs.contains_key(id) {
            self.active_tab = Some(id.to_string());
            true
        } else {
            false
        }
    }

    /// Get the active tab's ID, if any.
    pub fn active_tab_id(&self) -> Option<&str> {
        self.active_tab.as_deref()
    }

    /// Number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Whether there are no open tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Remove all tabs.
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.active_tab = None;
    }

    /// List all tab IDs.
    pub fn tab_ids(&self) -> Vec<&str> {
        self.tabs.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal mock for RayoPage so unit tests don't need Chrome.
    ///
    /// We construct a `TabManager` and exercise its bookkeeping without
    /// ever launching a browser.  Because `RayoPage` contains private
    /// fields that can only be built through `RayoBrowser::new_page()`,
    /// we test the manager logic with an empty HashMap and only validate
    /// the ID / active-tab tracking here.

    // Helper: create a TabManager with some fake tab IDs.
    // Since we can't construct RayoPage without a browser, we test
    // the index/active logic through a thin wrapper that doesn't
    // actually store pages.

    // -- Because RayoPage can't be constructed in unit tests, we test
    //    TabManager's bookkeeping via a parallel struct that mirrors
    //    the API but uses () instead of RayoPage. The real integration
    //    test lives in tests/browser_integration.rs.

    /// Stand-in tab manager that mirrors TabManager's logic with unit values.
    struct MockTabManager {
        tabs: HashMap<TabId, ()>,
        active_tab: Option<TabId>,
    }

    impl MockTabManager {
        fn new() -> Self {
            Self {
                tabs: HashMap::new(),
                active_tab: None,
            }
        }

        fn add_tab(&mut self, id: TabId) {
            self.active_tab = Some(id.clone());
            self.tabs.insert(id, ());
        }

        fn remove_tab(&mut self, id: &str) -> bool {
            let removed = self.tabs.remove(id).is_some();
            if removed && self.active_tab.as_deref() == Some(id) {
                self.active_tab = self.tabs.keys().next().cloned();
            }
            removed
        }

        fn set_active(&mut self, id: &str) -> bool {
            if self.tabs.contains_key(id) {
                self.active_tab = Some(id.to_string());
                true
            } else {
                false
            }
        }

        fn active_tab_id(&self) -> Option<&str> {
            self.active_tab.as_deref()
        }

        fn tab_count(&self) -> usize {
            self.tabs.len()
        }

        fn is_empty(&self) -> bool {
            self.tabs.is_empty()
        }

        fn clear(&mut self) {
            self.tabs.clear();
            self.active_tab = None;
        }

        fn tab_ids(&self) -> Vec<&str> {
            self.tabs.keys().map(|s| s.as_str()).collect()
        }
    }

    #[test]
    fn test_add_remove_active_switching() {
        let mut mgr = MockTabManager::new();

        // Empty manager
        assert!(mgr.is_empty());
        assert_eq!(mgr.tab_count(), 0);
        assert_eq!(mgr.active_tab_id(), None);

        // Add first tab — becomes active
        mgr.add_tab("tab-1".into());
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_tab_id(), Some("tab-1"));

        // Add second tab — becomes active
        mgr.add_tab("tab-2".into());
        assert_eq!(mgr.tab_count(), 2);
        assert_eq!(mgr.active_tab_id(), Some("tab-2"));

        // Switch back to first tab
        assert!(mgr.set_active("tab-1"));
        assert_eq!(mgr.active_tab_id(), Some("tab-1"));

        // Remove active tab — should switch to remaining tab
        assert!(mgr.remove_tab("tab-1"));
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_tab_id(), Some("tab-2"));

        // Remove non-active tab
        mgr.add_tab("tab-3".into());
        assert_eq!(mgr.active_tab_id(), Some("tab-3"));
        assert!(mgr.set_active("tab-2"));
        assert!(mgr.remove_tab("tab-3"));
        assert_eq!(mgr.active_tab_id(), Some("tab-2"));

        // Remove last tab
        assert!(mgr.remove_tab("tab-2"));
        assert!(mgr.is_empty());
        assert_eq!(mgr.active_tab_id(), None);
    }

    #[test]
    fn test_clear() {
        let mut mgr = MockTabManager::new();
        mgr.add_tab("a".into());
        mgr.add_tab("b".into());
        mgr.add_tab("c".into());
        assert_eq!(mgr.tab_count(), 3);

        mgr.clear();
        assert!(mgr.is_empty());
        assert_eq!(mgr.tab_count(), 0);
        assert_eq!(mgr.active_tab_id(), None);
    }

    #[test]
    fn test_set_active_nonexistent() {
        let mut mgr = MockTabManager::new();
        mgr.add_tab("tab-1".into());

        assert!(!mgr.set_active("no-such-tab"));
        // Active tab should remain unchanged
        assert_eq!(mgr.active_tab_id(), Some("tab-1"));
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut mgr = MockTabManager::new();
        mgr.add_tab("tab-1".into());

        assert!(!mgr.remove_tab("no-such-tab"));
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_tab_id(), Some("tab-1"));
    }

    #[test]
    fn test_tab_ids() {
        let mut mgr = MockTabManager::new();
        mgr.add_tab("alpha".into());
        mgr.add_tab("beta".into());

        let mut ids = mgr.tab_ids();
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }
}
