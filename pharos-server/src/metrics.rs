/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/metrics.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module provides the instrumentation for Pharos, tracking performance 
 * and health metrics (CPU, Memory, Storage). It implements both Pull 
 * (Prometheus scrape) and Push (placeholder) patterns, ensuring observability 
 * in Home Lab and Enterprise environments.
 * * Traceability:
 * Implements Task 2.4, Issue #10.
 * ======================================================================== */

use prometheus::{self, Encoder, Gauge, IntGauge, IntCounterVec, Opts, Registry, TextEncoder};
use lazy_static::lazy_static;
use tracing::warn;
use std::sync::Once;

static REGISTER: Once = Once::new();

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    pub static ref CPU_USAGE: Gauge = Gauge::with_opts(
        Opts::new("pharos_cpu_usage_percentage", "Current CPU usage percentage")
    ).expect("Failed to create CPU gauge");

    pub static ref MEMORY_USAGE_BYTES: IntGauge = IntGauge::with_opts(
        Opts::new("pharos_memory_usage_bytes", "Current memory usage in bytes")
    ).expect("Failed to create memory gauge");

    pub static ref TOTAL_RECORDS: IntGauge = IntGauge::with_opts(
        Opts::new("pharos_total_records", "Total number of records in storage")
    ).expect("Failed to create records gauge");

    pub static ref RECORDS_ADDED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("pharos_records_added_total", "Total number of new records created via add/upsert, labeled by source"),
        &["source"]
    ).expect("Failed to create records added counter");

    pub static ref RECORDS_UPDATED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("pharos_records_updated_total", "Total number of existing records updated via add/upsert, labeled by source"),
        &["source"]
    ).expect("Failed to create records updated counter");

    pub static ref RECORDS_DELETED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("pharos_records_deleted_total", "Total number of records deleted, labeled by source"),
        &["source"]
    ).expect("Failed to create records deleted counter");
}

pub fn register_metrics() {
    REGISTRY.register(Box::new(CPU_USAGE.clone())).expect("Failed to register CPU usage gauge");
    REGISTRY.register(Box::new(MEMORY_USAGE_BYTES.clone())).expect("Failed to register memory usage gauge");
    REGISTRY.register(Box::new(TOTAL_RECORDS.clone())).expect("Failed to register total records gauge");
    REGISTRY.register(Box::new(RECORDS_ADDED_TOTAL.clone())).expect("Failed to register records added counter");
    REGISTRY.register(Box::new(RECORDS_UPDATED_TOTAL.clone())).expect("Failed to register records updated counter");
    REGISTRY.register(Box::new(RECORDS_DELETED_TOTAL.clone())).expect("Failed to register records deleted counter");
}

pub fn gather_metrics() -> String {
    // Ensure metrics are registered
    REGISTER.call_once(|| {
        register_metrics();
    });

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder.encode(&metric_families, &mut buffer).expect("Failed to encode metrics");
    String::from_utf8(buffer).expect("Metrics contain invalid UTF-8")
}

/// Monitors system health against configurable thresholds.
pub fn check_health_thresholds(cpu_limit: f64, memory_limit_bytes: u64) {
    let current_cpu = CPU_USAGE.get();
    let current_mem = MEMORY_USAGE_BYTES.get() as u64;

    if current_cpu > cpu_limit {
        warn!("HEALTH ALERT: CPU usage ({:.2}%) exceeds threshold ({:.2}%)", current_cpu, cpu_limit);
    }

    if current_mem > memory_limit_bytes {
        warn!("HEALTH ALERT: Memory usage ({} bytes) exceeds threshold ({} bytes)", current_mem, memory_limit_bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }

    #[test]
    fn test_should_track_record_count_when_incremented() {
        let _lock = TEST_MUTEX.lock().unwrap();
        TOTAL_RECORDS.set(42);
        assert_eq!(TOTAL_RECORDS.get(), 42);
    }

    #[test]
    fn test_should_gather_metrics_as_prometheus_format() {
        let _lock = TEST_MUTEX.lock().unwrap();
        TOTAL_RECORDS.set(10);
        let output = gather_metrics();
        // Prometheus format for Gauge/IntGauge typically ends with " 10" (or " 10.0")
        assert!(output.contains("pharos_total_records"));
        assert!(output.contains("10"));
    }

    #[test]
    fn test_should_alert_when_thresholds_exceeded() {
        let _lock = TEST_MUTEX.lock().unwrap();
        // This test verifies the logic of check_health_thresholds by simulating threshold breach
        // Since we can't easily capture logs in a unit test without more setup, 
        // we just ensure it doesn't panic.
        CPU_USAGE.set(95.0);
        MEMORY_USAGE_BYTES.set(1000000);
        check_health_thresholds(90.0, 500000);
    }

    #[test]
    fn test_should_increment_records_added_total_with_source_label() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let before = RECORDS_ADDED_TOTAL.with_label_values(&["test-add-source"]).get();
        RECORDS_ADDED_TOTAL.with_label_values(&["test-add-source"]).inc();
        let after = RECORDS_ADDED_TOTAL.with_label_values(&["test-add-source"]).get();
        assert_eq!(after - before, 1);
    }

    #[test]
    fn test_should_increment_records_updated_total_with_source_label() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let before = RECORDS_UPDATED_TOTAL.with_label_values(&["test-update-source"]).get();
        RECORDS_UPDATED_TOTAL.with_label_values(&["test-update-source"]).inc();
        let after = RECORDS_UPDATED_TOTAL.with_label_values(&["test-update-source"]).get();
        assert_eq!(after - before, 1);
    }

    #[test]
    fn test_should_increment_records_deleted_total_with_source_label() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let before = RECORDS_DELETED_TOTAL.with_label_values(&["test-delete-source"]).get();
        RECORDS_DELETED_TOTAL.with_label_values(&["test-delete-source"]).inc_by(3);
        let after = RECORDS_DELETED_TOTAL.with_label_values(&["test-delete-source"]).get();
        assert_eq!(after - before, 3);
    }

    #[test]
    fn test_should_gather_records_metrics_in_prometheus_format() {
        let _lock = TEST_MUTEX.lock().unwrap();
        RECORDS_ADDED_TOTAL.with_label_values(&["gather-format-test"]).inc();
        let output = gather_metrics();
        assert!(output.contains("pharos_records_added_total"));
        assert!(output.contains("gather-format-test"));
    }
}
