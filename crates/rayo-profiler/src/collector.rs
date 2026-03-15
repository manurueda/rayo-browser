//! Ring buffer span collector with bounded memory.

use std::time::Duration;

use crate::{ProfileSpan, SpanCategory};

/// Ring buffer collector for profiling spans.
///
/// Fixed capacity prevents unbounded memory growth.
/// When full, oldest spans are evicted.
pub struct Collector {
    spans: Vec<Option<ProfileSpan>>,
    write_idx: usize,
    next_id: u64,
    count: usize,
}

impl Collector {
    pub fn new(capacity: usize) -> Self {
        Self {
            spans: vec![None; capacity],
            write_idx: 0,
            next_id: 1,
            count: 0,
        }
    }

    /// Start a new span. Returns the span ID.
    pub fn start_span(
        &mut self,
        name: String,
        category: SpanCategory,
        start_us: u64,
        parent_id: Option<u64>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let span = ProfileSpan {
            id,
            parent_id,
            name,
            category,
            start_us,
            duration_us: None,
            metadata: serde_json::Value::Null,
        };

        self.spans[self.write_idx] = Some(span);
        self.write_idx = (self.write_idx + 1) % self.spans.len();
        if self.count < self.spans.len() {
            self.count += 1;
        }

        id
    }

    /// End a span by recording its duration.
    pub fn end_span(&mut self, id: u64, duration: Duration) {
        let duration_us = duration.as_micros() as u64;
        // Search backwards from write position (most recent spans)
        for i in (0..self.spans.len()).rev() {
            let idx = (self.write_idx + i) % self.spans.len();
            if let Some(span) = &mut self.spans[idx] {
                if span.id == id {
                    span.duration_us = Some(duration_us);
                    return;
                }
            }
        }
    }

    /// Get all completed spans (with duration).
    pub fn spans(&self) -> Vec<ProfileSpan> {
        self.spans
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.duration_us.is_some())
            .cloned()
            .collect()
    }

    /// Total number of recorded spans (including incomplete).
    pub fn span_count(&self) -> usize {
        self.count
    }

    /// Reset all collected spans.
    pub fn reset(&mut self) {
        for slot in &mut self.spans {
            *slot = None;
        }
        self.write_idx = 0;
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_eviction() {
        let mut collector = Collector::new(3);

        // Fill beyond capacity
        for i in 0..5 {
            let id = collector.start_span(
                format!("span_{i}"),
                SpanCategory::User,
                i * 1000,
                None,
            );
            collector.end_span(id, Duration::from_millis(1));
        }

        let spans = collector.spans();
        // Only 3 should survive (capacity)
        assert_eq!(spans.len(), 3);
        // Should be the most recent 3 (order depends on ring buffer position)
        let names: Vec<&str> = spans.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"span_2"));
        assert!(names.contains(&"span_3"));
        assert!(names.contains(&"span_4"));
        assert!(!names.contains(&"span_0"));
        assert!(!names.contains(&"span_1"));
    }

    #[test]
    fn test_reset() {
        let mut collector = Collector::new(10);
        let id = collector.start_span("test".into(), SpanCategory::User, 0, None);
        collector.end_span(id, Duration::from_millis(1));

        assert_eq!(collector.span_count(), 1);
        collector.reset();
        assert_eq!(collector.span_count(), 0);
        assert!(collector.spans().is_empty());
    }
}
