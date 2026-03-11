//! Framework metrics for observability.
//!
//! Provides atomic counters, duration histograms, and a global singleton.
//!
//! # Examples
//!
//! ```
//! use blazegram::metrics::metrics;
//!
//! metrics().inc_updates();
//! {
//!     let _t = metrics().timer("handler");
//!     // … do work …
//! } // duration recorded on drop
//!
//! println!("{}", metrics().summary());
//! ```

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Maximum samples per label in the duration ring buffer.
/// Keeps memory bounded while still providing reasonable percentile accuracy.
const MAX_DURATION_SAMPLES: usize = 10_000;

/// Central metrics store.
pub struct Metrics {
    /// Total updates processed.
    pub updates_total: AtomicU64,
    /// Total errors encountered.
    pub errors_total: AtomicU64,
    /// Total Telegram API calls made.
    pub api_calls_total: AtomicU64,
    /// API calls avoided (diff/cache optimisations).
    pub api_calls_saved: AtomicU64,
    /// Currently active chat count.
    pub active_chats: AtomicU64,
    /// Duration ring buffers: label → capped ring of durations in microseconds.
    durations_us: DashMap<&'static str, RingBuffer>,
    /// Total count and sum per label (never reset, for accurate _count/_sum in Prometheus).
    duration_totals: DashMap<&'static str, (u64, u64)>,
}

/// Simple ring buffer that evicts oldest entries when full.
struct RingBuffer {
    data: Vec<u64>,
    pos: usize,
    full: bool,
}

impl RingBuffer {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(256), // start small, grow to MAX
            pos: 0,
            full: false,
        }
    }

    fn push(&mut self, value: u64) {
        if self.data.len() < MAX_DURATION_SAMPLES {
            self.data.push(value);
        } else {
            self.data[self.pos] = value;
            self.pos = (self.pos + 1) % MAX_DURATION_SAMPLES;
            self.full = true;
        }
    }

    fn samples(&self) -> &[u64] {
        &self.data
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Metrics {
    /// Create a fresh `Metrics` instance (all counters zero).
    pub fn new() -> Self {
        Self {
            updates_total: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
            api_calls_total: AtomicU64::new(0),
            api_calls_saved: AtomicU64::new(0),
            active_chats: AtomicU64::new(0),
            durations_us: DashMap::new(),
            duration_totals: DashMap::new(),
        }
    }

    /// Increment the total-updates counter.
    pub fn inc_updates(&self) {
        self.updates_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the total-errors counter.
    pub fn inc_errors(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the API-calls counter.
    pub fn inc_api_calls(&self) {
        self.api_calls_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the API-calls-saved counter.
    pub fn inc_api_saved(&self) {
        self.api_calls_saved.fetch_add(1, Ordering::Relaxed);
    }

    /// Set the active-chats gauge.
    pub fn set_active_chats(&self, n: u64) {
        self.active_chats.store(n, Ordering::Relaxed);
    }

    /// Record a duration sample under `label`.
    /// Ring buffer is capped at MAX_DURATION_SAMPLES to prevent unbounded memory growth.
    pub fn record_duration(&self, label: &'static str, duration: Duration) {
        let us = duration.as_micros() as u64;
        self.durations_us
            .entry(label)
            .or_insert_with(RingBuffer::new)
            .push(us);
        // Track accurate total count/sum for Prometheus exposition
        let mut totals = self.duration_totals.entry(label).or_insert((0, 0));
        totals.0 += 1; // count
        totals.1 += us; // sum
    }

    /// Return a [`Timer`] guard that records elapsed time on drop.
    pub fn timer(&self, label: &'static str) -> Timer<'_> {
        Timer {
            metrics: self,
            label,
            start: Instant::now(),
        }
    }

    /// Prometheus-compatible text exposition format.
    pub fn prometheus(&self) -> String {
        let mut out = String::with_capacity(512);

        write_prom_counter(
            &mut out,
            "bg_updates_total",
            "Total updates processed",
            self.updates_total.load(Ordering::Relaxed),
        );
        write_prom_counter(
            &mut out,
            "bg_errors_total",
            "Total errors",
            self.errors_total.load(Ordering::Relaxed),
        );
        write_prom_counter(
            &mut out,
            "bg_api_calls_total",
            "Total API calls",
            self.api_calls_total.load(Ordering::Relaxed),
        );
        write_prom_counter(
            &mut out,
            "bg_api_calls_saved_total",
            "API calls saved by diff/cache",
            self.api_calls_saved.load(Ordering::Relaxed),
        );
        write_prom_gauge(
            &mut out,
            "bg_active_chats",
            "Number of active chats",
            self.active_chats.load(Ordering::Relaxed),
        );

        // Duration summaries per label
        for entry in self.durations_us.iter() {
            let label = *entry.key();
            let ring = entry.value();
            if ring.is_empty() {
                continue;
            }
            let mut sorted: Vec<u64> = ring.samples().to_vec();
            sorted.sort_unstable();

            // Use accurate totals for _count/_sum
            let (total_count, total_sum_us) = self
                .duration_totals
                .get(label)
                .map(|r| *r.value())
                .unwrap_or((sorted.len() as u64, sorted.iter().sum()));

            let name = format!("bg_duration_{}", sanitize_prom(label));
            out.push_str(&format!("# HELP {} Duration of {label} in seconds\n", name));
            out.push_str(&format!("# TYPE {} summary\n", name));
            out.push_str(&format!(
                "{name}{{quantile=\"0.5\"}} {:.6}\n",
                percentile_sec(&sorted, 50)
            ));
            out.push_str(&format!(
                "{name}{{quantile=\"0.95\"}} {:.6}\n",
                percentile_sec(&sorted, 95)
            ));
            out.push_str(&format!(
                "{name}{{quantile=\"0.99\"}} {:.6}\n",
                percentile_sec(&sorted, 99)
            ));
            out.push_str(&format!(
                "{name}_sum {:.6}\n",
                total_sum_us as f64 / 1_000_000.0
            ));
            out.push_str(&format!("{name}_count {total_count}\n"));
        }

        out
    }

    /// Human-readable metrics summary.
    pub fn summary(&self) -> String {
        let mut out = String::with_capacity(512);

        out.push_str(&format!(
            "Updates: {} | Errors: {} | API calls: {} (saved: {}) | Active chats: {}\n",
            self.updates_total.load(Ordering::Relaxed),
            self.errors_total.load(Ordering::Relaxed),
            self.api_calls_total.load(Ordering::Relaxed),
            self.api_calls_saved.load(Ordering::Relaxed),
            self.active_chats.load(Ordering::Relaxed),
        ));

        for entry in self.durations_us.iter() {
            let label = *entry.key();
            let ring = entry.value();
            if ring.is_empty() {
                continue;
            }
            let mut sorted: Vec<u64> = ring.samples().to_vec();
            sorted.sort_unstable();
            let (total_count, total_sum_us) = self
                .duration_totals
                .get(label)
                .map(|r| *r.value())
                .unwrap_or((sorted.len() as u64, sorted.iter().sum()));
            let avg_us = if total_count > 0 {
                total_sum_us / total_count
            } else {
                0
            };

            out.push_str(&format!(
                "  {label}: count={total_count}, avg={avg_us}µs, \
                 p50={}µs, p95={}µs, p99={}µs (window={})
",
                percentile_us(&sorted, 50),
                percentile_us(&sorted, 95),
                percentile_us(&sorted, 99),
                ring.len(),
            ));
        }

        out
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Timer guard ───

/// A guard that records elapsed duration when dropped.
pub struct Timer<'a> {
    metrics: &'a Metrics,
    label: &'static str,
    start: Instant,
}

impl<'a> Timer<'a> {
    /// Elapsed time since the timer was created.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl<'a> Drop for Timer<'a> {
    fn drop(&mut self) {
        self.metrics
            .record_duration(self.label, self.start.elapsed());
    }
}

// ─── Global singleton ───

static GLOBAL_METRICS: OnceLock<Metrics> = OnceLock::new();

/// Returns a reference to the global `Metrics` instance.
pub fn metrics() -> &'static Metrics {
    GLOBAL_METRICS.get_or_init(Metrics::new)
}

// ─── Helpers ───

fn percentile_us(sorted: &[u64], p: u32) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn percentile_sec(sorted: &[u64], p: u32) -> f64 {
    percentile_us(sorted, p) as f64 / 1_000_000.0
}

fn sanitize_prom(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_prom_counter(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} counter\n"));
    out.push_str(&format!("{name} {value}\n"));
}

fn write_prom_gauge(out: &mut String, name: &str, help: &str, value: u64) {
    out.push_str(&format!("# HELP {name} {help}\n"));
    out.push_str(&format!("# TYPE {name} gauge\n"));
    out.push_str(&format!("{name} {value}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_counters() {
        let m = Metrics::new();
        m.inc_updates();
        m.inc_updates();
        m.inc_errors();
        m.inc_api_calls();
        m.inc_api_calls();
        m.inc_api_calls();
        m.inc_api_saved();
        m.set_active_chats(42);

        assert_eq!(m.updates_total.load(Ordering::Relaxed), 2);
        assert_eq!(m.errors_total.load(Ordering::Relaxed), 1);
        assert_eq!(m.api_calls_total.load(Ordering::Relaxed), 3);
        assert_eq!(m.api_calls_saved.load(Ordering::Relaxed), 1);
        assert_eq!(m.active_chats.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn test_record_duration() {
        let m = Metrics::new();
        m.record_duration("test_op", Duration::from_micros(100));
        m.record_duration("test_op", Duration::from_micros(200));
        m.record_duration("test_op", Duration::from_micros(300));

        let ring = m.durations_us.get("test_op").unwrap();
        assert_eq!(ring.value().len(), 3);
        assert_eq!(ring.value().samples(), &[100u64, 200, 300]);
        // Check accurate totals
        let totals = m.duration_totals.get("test_op").unwrap();
        assert_eq!(*totals.value(), (3, 600));
    }

    #[test]
    fn test_timer_records() {
        let m = Metrics::new();
        {
            let _t = m.timer("sleep_test");
            std::thread::sleep(Duration::from_millis(5));
        }
        let ring = m.durations_us.get("sleep_test").unwrap();
        assert_eq!(ring.value().len(), 1);
        // Should be at least 4ms (4000µs) — generous lower bound
        assert!(
            ring.value().samples()[0] >= 4_000,
            "duration was {} µs",
            ring.value().samples()[0]
        );
    }

    #[test]
    fn test_percentile() {
        // 0..100 → values 0,1,2,...,99
        let sorted: Vec<u64> = (0..100).collect();
        // p50 → index round(0.50 * 99) = round(49.5) = 50 → value 50
        assert_eq!(percentile_us(&sorted, 50), 50);
        // p95 → index round(0.95 * 99) = round(94.05) = 94 → value 94
        assert_eq!(percentile_us(&sorted, 95), 94);
        // p99 → index round(0.99 * 99) = round(98.01) = 98 → value 98
        assert_eq!(percentile_us(&sorted, 99), 98);
    }

    #[test]
    fn test_percentile_single() {
        let sorted = vec![42];
        assert_eq!(percentile_us(&sorted, 50), 42);
        assert_eq!(percentile_us(&sorted, 99), 42);
    }

    #[test]
    fn test_percentile_empty() {
        let sorted: Vec<u64> = vec![];
        assert_eq!(percentile_us(&sorted, 50), 0);
    }

    #[test]
    fn test_prometheus_output() {
        let m = Metrics::new();
        m.inc_updates();
        m.record_duration("handler", Duration::from_micros(500));

        let prom = m.prometheus();
        assert!(prom.contains("bg_updates_total 1"));
        assert!(prom.contains("# TYPE bg_updates_total counter"));
        assert!(prom.contains("bg_errors_total 0"));
        assert!(prom.contains("bg_duration_handler"));
        assert!(prom.contains("quantile=\"0.5\""));
    }

    #[test]
    fn test_summary_output() {
        let m = Metrics::new();
        m.inc_updates();
        m.inc_updates();
        m.inc_errors();
        m.set_active_chats(5);
        m.record_duration("process", Duration::from_micros(100));
        m.record_duration("process", Duration::from_micros(200));

        let s = m.summary();
        assert!(s.contains("Updates: 2"));
        assert!(s.contains("Errors: 1"));
        assert!(s.contains("Active chats: 5"));
        assert!(s.contains("process:"));
        assert!(s.contains("p50="));
    }

    #[test]
    fn test_global_singleton() {
        let a = metrics();
        let b = metrics();
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn test_sanitize_prom() {
        assert_eq!(sanitize_prom("hello-world.foo"), "hello_world_foo");
        assert_eq!(sanitize_prom("ok_name"), "ok_name");
    }
}
