use serde_json::Value;
use std::{collections::HashMap, path::Path};

fn load_monitors() -> Vec<Value> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let monitor_path = repo_root.join("dd-monitors/temperpaw-monitors.json");
    let monitors: Value =
        serde_json::from_str(&std::fs::read_to_string(monitor_path).unwrap()).unwrap();
    monitors.as_array().unwrap().clone()
}

fn monitors_by_name(monitors: &[Value]) -> HashMap<&str, &Value> {
    monitors
        .iter()
        .map(|monitor| (monitor["name"].as_str().unwrap(), monitor))
        .collect()
}

#[test]
fn openpaw_monitors_use_current_emitted_metrics_instead_of_stale_trace_custom_metrics() {
    let monitors = load_monitors();
    let by_name = monitors_by_name(&monitors);

    let expected = [
        (
            "[OpenPaw] Error Rate Spike",
            "sum(last_15m):default_zero(sum:openpaw.logs.errors{service:openpaw}.as_count()) / sum:temper_cedar_evaluations_total{service:openpaw}.as_count() > 0.1",
        ),
        (
            "[OpenPaw] Request Latency Spike (P95)",
            "avg(last_15m):avg:temper_dispatch_ask_latency_ms{service:openpaw} > 5000",
        ),
        (
            "[OpenPaw] No Traffic",
            "sum(last_15m):default_zero(sum:temper_cedar_evaluations_total{service:openpaw}.as_count()) < 1",
        ),
    ];

    for (name, query) in expected {
        let monitor = by_name
            .get(name)
            .unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(monitor["type"].as_str(), Some("metric alert"));
        assert_eq!(monitor["query"].as_str(), Some(query));
        assert_eq!(monitor["options"]["notify_no_data"].as_bool(), Some(false));
    }

    for name in [
        "[OpenPaw] Error Rate Spike",
        "[OpenPaw] Request Latency Spike (P95)",
    ] {
        assert_eq!(
            by_name[name]["options"]["on_missing_data"].as_str(),
            Some("resolve"),
            "{name} should resolve when no traffic makes the rate/latency sample inapplicable"
        );
    }

    let all_queries = monitors
        .iter()
        .filter_map(|monitor| monitor["query"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !all_queries.contains("trace.custom"),
        "trace.custom is not emitted for service:openpaw and must not back monitors"
    );
}

#[test]
fn monitor_queries_use_current_runtime_metric_names_and_zero_fill_sparse_counters() {
    let monitors = load_monitors();
    let by_name = monitors_by_name(&monitors);

    assert_eq!(
        by_name["[Temper] Active Entities Drop"]["query"].as_str(),
        Some("min(last_10m):avg:temper_active_actors{service:openpaw}.rollup(avg, 60) < 1")
    );

    let sparse_counter_monitors = [
        "[Temper] Required WASM Load Failures",
        "[Temper] Session Memory Budget Exceeded",
        "[Temper] Dispatch Retry Budget Exhausted",
        "[Temper] Permanent Actor Failures",
        "[Temper] Mailbox Saturation (mailbox_full spike)",
        "[Temper] Excessive Overdue Timers on Replay",
        "[Temper] Spec Liveness Violation",
        "[Temper] Admission Deferred Spike",
        "[Temper] Unexpected Mailbox Drops (post-P4)",
        "[Temper] Profiler Upload Failures",
        "[Temper] Session Memory Externalization Spike",
        "[Temper] State Timeout Reset Rate Drop",
        "[Temper] Profiler Uploads Stalled",
        "[Temper] Integration Silent Exit (ADR-0056)",
        "[Temper] Hydration Re-arm Overdue Spike",
        "[Temper] Turso Write Retry Exhaustion",
        "[OpenPaw] Session Phase Budget Exceeded",
        "[Temper] Query Projection Update Errors",
    ];

    for name in sparse_counter_monitors {
        let monitor = by_name
            .get(name)
            .unwrap_or_else(|| panic!("{name} missing"));
        let query = monitor["query"].as_str().unwrap();
        assert!(
            query.contains("default_zero(sum:"),
            "{name} should zero-fill absent healthy sparse counter data, got {query}"
        );
        assert_eq!(monitor["options"]["notify_no_data"].as_bool(), Some(false));
    }
}
