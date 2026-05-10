//! Metrics — Prometheus-compatible counters, gauges, and histograms.
//!
//! This module provides in-process metrics without external dependencies.
//! Exposed via GET /api/metrics in Prometheus text format.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::{Mutex, RwLock};

/// Thread-safe metrics registry.
#[derive(Default)]
pub struct MetricsRegistry {
    counters: RwLock<Vec<Counter>>,
    gauges: RwLock<Vec<Gauge>>,
    histograms: RwLock<Vec<Histogram>>,
}

impl MetricsRegistry {
    /// Create a new metrics registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new counter and return a handle.
    pub fn counter(&self, name: &'static str, help: &'static str, labels: &[(&'static str, &'static str)]) -> CounterHandle {
        let mut counters = self.counters.write();
        let id = counters.len();
        counters.push(Counter {
            name: name.into(),
            help: help.into(),
            labels: labels.into(),
            value: AtomicU64::new(0),
        });
        CounterHandle { id }
    }

    /// Register a new gauge and return a handle.
    pub fn gauge(&self, name: &'static str, help: &'static str, initial: f64) -> GaugeHandle {
        let mut gauges = self.gauges.write();
        let id = gauges.len();
        gauges.push(Gauge {
            name: name.into(),
            help: help.into(),
            value: Mutex::new(initial),
        });
        GaugeHandle { id }
    }

    /// Register a new histogram and return a handle.
    pub fn histogram(&self, name: &'static str, help: &'static str, buckets: Vec<f64>) -> HistogramHandle {
        let mut histograms = self.histograms.write();
        let id = histograms.len();
        let counts: Vec<usize> = vec![0; buckets.len() + 1];
        histograms.push(Histogram {
            name: name.into(),
            help: help.into(),
            buckets: buckets.clone(),
            counts: RwLock::new(counts),
            sum: Mutex::new(0.0),
            count: Mutex::new(0u64),
        });
        HistogramHandle { id, buckets }
    }

    /// Export all metrics in Prometheus text format.
    pub fn export(&self) -> String {
        let mut out = String::new();

        // Counters
        {
            let counters = self.counters.read();
            for c in counters.iter() {
                out.push_str(&format!("# HELP {} {}\n", c.name, c.help));
                out.push_str(&format!("# TYPE {} counter\n", c.name));
                let value = c.value.load(Ordering::Relaxed);
                let labels_str = if c.labels.is_empty() {
                    String::new()
                } else {
                    format!("{{{}}}", c.labels.iter().map(|(k, v)| format!("{}=\"{}\"", k, v)).collect::<Vec<_>>().join(","))
                };
                out.push_str(&format!("{}{} {}\n", c.name, labels_str, value));
            }
        }

        // Gauges
        {
            let gauges = self.gauges.read();
            for g in gauges.iter() {
                out.push_str(&format!("# HELP {} {}\n", g.name, g.help));
                out.push_str(&format!("# TYPE {} gauge\n", g.name));
                let value = *g.value.lock();
                out.push_str(&format!("{} {}\n", g.name, value));
            }
        }

        // Histograms
        {
            let histograms = self.histograms.read();
            for h in histograms.iter() {
                out.push_str(&format!("# HELP {} {}\n", h.name, h.help));
                out.push_str(&format!("# TYPE {} histogram\n", h.name));
                let sum = *h.sum.lock();
                let count = *h.count.lock();
                let bucket_values = h.buckets.clone();
                let counts = h.counts.read();
                let mut cumulative = 0usize;
                for (i, _) in bucket_values.iter().enumerate() {
                    cumulative += counts[i];
                    let boundary = bucket_values[i];
                    out.push_str(&format!("{}{{le=\"{}\"}} {}\n", h.name, boundary, cumulative));
                }
                // +Inf bucket
                out.push_str(&format!("{}{{le=\"+Inf\"}} {}\n", h.name, cumulative));
                out.push_str(&format!("{}_sum {} \n", h.name, sum));
                out.push_str(&format!("{}_count {} \n", h.name, count));
            }
        }

        out
    }
}

/// Global metrics registry.
static REGISTRY: std::sync::OnceLock<MetricsRegistry> = std::sync::OnceLock::new();

/// Get the global metrics registry.
pub fn registry() -> &'static MetricsRegistry {
    REGISTRY.get_or_init(MetricsRegistry::new)
}

#[derive(Clone)]
pub struct CounterHandle { id: usize }

impl CounterHandle {
    /// Increment the counter by 1.
    pub fn inc(&self) {
        let r = registry();
        let counters = r.counters.read();
        if let Some(c) = counters.get(self.id) {
            c.value.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[derive(Clone)]
pub struct GaugeHandle { id: usize }

impl GaugeHandle {
    /// Set the gauge to a specific value.
    pub fn set(&self, v: f64) {
        let r = registry();
        let gauges = r.gauges.read();
        if let Some(g) = gauges.get(self.id) {
            *g.value.lock() = v;
        }
    }

    /// Increment the gauge by 1.
    pub fn inc(&self) {
        let r = registry();
        let gauges = r.gauges.read();
        if let Some(g) = gauges.get(self.id) {
            let mut val = g.value.lock();
            *val += 1.0;
        }
    }

    /// Decrement the gauge by 1.
    pub fn dec(&self) {
        let r = registry();
        let gauges = r.gauges.read();
        if let Some(g) = gauges.get(self.id) {
            let mut val = g.value.lock();
            *val -= 1.0;
        }
    }
}

#[derive(Clone)]
pub struct HistogramHandle { id: usize, buckets: Vec<f64> }

impl HistogramHandle {
    /// Observe a value, adding it to the histogram.
    pub fn observe(&self, value: f64) {
        let r = registry();
        let histograms = r.histograms.read();
        if let Some(h) = histograms.get(self.id) {
            {
                let mut sum = h.sum.lock();
                *sum += value;
            }
            {
                let mut count = h.count.lock();
                *count += 1;
            }
            {
                let mut counts = h.counts.write();
                for (i, boundary) in self.buckets.iter().enumerate() {
                    if value <= *boundary {
                        counts[i] += 1;
                    }
                }
                // +Inf bucket
                counts[self.buckets.len()] += 1;
            }
        }
    }
}

struct Counter {
    name: String,
    help: String,
    labels: Vec<(&'static str, &'static str)>,
    value: AtomicU64,
}

struct Gauge {
    name: String,
    help: String,
    value: Mutex<f64>,
}

struct Histogram {
    name: String,
    help: String,
    buckets: Vec<f64>,
    counts: RwLock<Vec<usize>>,
    sum: Mutex<f64>,
    count: Mutex<u64>,
}

/// Metrics handles initialized at startup.
#[derive(Clone)]
pub struct MetricsHandles {
    pub agents_forked: CounterHandle,
    pub agents_completed: CounterHandle,
    pub agents_failed: CounterHandle,
    pub orch_duration: HistogramHandle,
    pub messages: CounterHandle,
}

impl MetricsHandles {
    /// Increment agents_forked counter.
    pub fn inc_agents_forked(&self) {
        self.agents_forked.inc();
    }

    /// Increment agents_completed counter.
    pub fn inc_agents_completed(&self) {
        self.agents_completed.inc();
    }

    /// Increment agents_failed counter.
    pub fn inc_agents_failed(&self) {
        self.agents_failed.inc();
    }

    /// Increment messages counter.
    pub fn inc_messages(&self) {
        self.messages.inc();
    }

    /// Observe orchestration duration.
    pub fn observe_orch_duration(&self, value: f64) {
        self.orch_duration.observe(value);
    }
}

/// Global lazy metric handles.
static METRICS: std::sync::OnceLock<MetricsHandles> = std::sync::OnceLock::new();

/// Get or create the metrics handles.
pub fn get_metrics() -> &'static MetricsHandles {
    METRICS.get_or_init(|| {
        let r = registry();
        MetricsHandles {
            agents_forked: r.counter("oxios_agents_forked_total", "Total agents forked", &[]),
            agents_completed: r.counter("oxios_agents_completed_total", "Total agents completed", &[]),
            agents_failed: r.counter("oxios_agents_failed_total", "Total agents failed", &[]),
            orch_duration: r.histogram("oxios_orchestration_duration_seconds", "Orchestration duration", vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
            messages: r.counter("oxios_messages_processed_total", "Messages processed", &[]),
        }
    })
}

/// Register all built-in metrics. Call once at startup.
pub fn register_builtin_metrics() {
    let r = registry();

    // Agent metrics
    r.counter("oxios_agents_forked_total", "Total agents forked", &[]);
    r.gauge("oxios_agents_running", "Currently running agents", 0.0);
    r.counter("oxios_agents_completed_total", "Total agents completed", &[]);
    r.counter("oxios_agents_failed_total", "Total agents failed", &[]);

    // Message metrics
    r.counter("oxios_messages_processed_total", "User messages processed", &[]);
    r.histogram("oxios_orchestration_duration_seconds", "Full orchestration duration", vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]);
    r.histogram("oxios_phase_duration_seconds", "Phase duration", vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0]);

    // LLM metrics
    r.counter("oxios_llm_calls_total", "LLM API calls", &[]);
    r.histogram("oxios_llm_duration_seconds", "LLM call duration", vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0]);
    r.counter("oxios_llm_errors_total", "LLM API errors", &[]);

    // Tool metrics
    r.counter("oxios_tool_calls_total", "Tool calls", &[]);
    r.histogram("oxios_tool_duration_seconds", "Tool call duration", vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]);
    r.counter("oxios_tool_errors_total", "Tool errors", &[]);

    // Memory metrics
    r.gauge("oxios_memory_entries_total", "Total memory entries", 0.0);
    r.counter("oxios_memory_recall_total", "Memory recall operations", &[]);

    // Container metrics
    r.counter("oxios_exec_total", "Exec calls", &[]);
    r.histogram("oxios_exec_duration_seconds", "Exec duration", vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0]);

    // Session metrics
    r.gauge("oxios_active_sessions", "Active sessions", 0.0);

    // Initialize get_metrics() handles
    let _ = get_metrics();
}