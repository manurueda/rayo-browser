//! LRU selector cache with CDP DOM invalidation.
//!
//! Playwright resolves selectors from scratch on every action.
//! We cache resolved selectors and invalidate on DOM mutations.

use std::num::NonZeroUsize;

use lru::LruCache;

/// Cached element reference.
#[derive(Debug, Clone)]
pub struct CachedElement {
    /// The CDP remote object ID.
    pub remote_object_id: String,
    /// The CSS selector used to find this element.
    pub selector: String,
    /// When this entry was cached (monotonic counter).
    pub cached_at: u64,
}

/// Selector cache that invalidates on DOM mutations.
pub struct SelectorCache {
    cache: LruCache<String, CachedElement>,
    /// Monotonic generation counter. Incremented on DOM mutations.
    generation: u64,
    /// Generation at which each entry was cached.
    hits: u64,
    misses: u64,
}

impl SelectorCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1024).unwrap())),
            generation: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a cached element by selector.
    /// Returns None if not cached or if the cache was invalidated.
    pub fn get(&mut self, selector: &str) -> Option<&CachedElement> {
        if let Some(entry) = self.cache.get(selector) {
            if entry.cached_at == self.generation {
                self.hits += 1;
                return Some(entry);
            }
            // Stale entry — DOM has changed since this was cached
            self.misses += 1;
            None
        } else {
            self.misses += 1;
            None
        }
    }

    /// Cache a resolved element.
    pub fn put(&mut self, selector: String, remote_object_id: String) {
        self.cache.put(
            selector.clone(),
            CachedElement {
                remote_object_id,
                selector,
                cached_at: self.generation,
            },
        );
    }

    /// Invalidate the cache. Called when CDP DOM.documentUpdated fires.
    pub fn invalidate(&mut self) {
        self.generation += 1;
    }

    /// Full reset — clear all entries.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.generation += 1;
    }

    /// Cache hit rate (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit() {
        let mut cache = SelectorCache::new(100);
        cache.put("div.foo".into(), "obj-123".into());

        let result = cache.get("div.foo");
        assert!(result.is_some());
        assert_eq!(result.unwrap().remote_object_id, "obj-123");
        assert_eq!(cache.hits(), 1);
    }

    #[test]
    fn test_cache_miss_after_invalidation() {
        let mut cache = SelectorCache::new(100);
        cache.put("div.foo".into(), "obj-123".into());

        // Simulate DOM mutation
        cache.invalidate();

        let result = cache.get("div.foo");
        assert!(result.is_none());
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn test_hit_rate() {
        let mut cache = SelectorCache::new(100);
        cache.put("a".into(), "1".into());

        cache.get("a"); // hit
        cache.get("a"); // hit
        cache.get("b"); // miss

        assert!((cache.hit_rate() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = SelectorCache::new(2);
        cache.put("a".into(), "1".into());
        cache.put("b".into(), "2".into());
        cache.put("c".into(), "3".into()); // evicts "a"

        assert!(cache.get("a").is_none());
        assert!(cache.get("c").is_some());
    }
}
