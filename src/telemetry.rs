use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    Json,
    Text,
}

pub fn init(format: LogFormat) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,cronduit=debug"));

    match format {
        LogFormat::Json => {
            let fmt_layer = fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(false)
                .with_target(true)
                .with_file(false)
                .with_line_number(false);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        LogFormat::Text => {
            let fmt_layer = fmt::layer()
                .with_target(true)
                .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()));
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
    }
}

/// Initialize the Prometheus metrics recorder with homelab-tuned histogram buckets (D-06).
///
/// Must be called once at startup, after tracing init but before any metrics macros are used.
/// Returns a `PrometheusHandle` that renders the `/metrics` endpoint response.
///
/// GAP-1 fix (06-06-PLAN.md): every cronduit metric family is eagerly described at
/// install time so the Prometheus exporter renders HELP/TYPE lines from boot, even
/// before a single sync or run has incremented the underlying counters/gauges. Without
/// describe_*, the exporter lazily registers on first observation and `cronduit_jobs_total`
/// could silently disappear from `/metrics` output despite being `.set()` in sync.rs.
pub fn setup_metrics() -> PrometheusHandle {
    let builder = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("cronduit_run_duration_seconds".to_string()),
            &[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0],
        )
        .expect("valid bucket config");

    let handle = match builder.install_recorder() {
        Ok(handle) => handle,
        Err(_) => {
            tracing::warn!("metrics recorder already installed, building detached handle");
            PrometheusBuilder::new().build_recorder().handle()
        }
    };

    // GAP-1: eagerly describe all cronduit metric families so the Prometheus
    // exporter renders HELP/TYPE lines from boot (not just after first observation).
    metrics::describe_gauge!(
        "cronduit_scheduler_up",
        "1 if the cronduit scheduler loop is running, 0 otherwise"
    );
    metrics::describe_gauge!(
        "cronduit_jobs_total",
        "Number of enabled jobs currently configured"
    );
    metrics::describe_counter!(
        "cronduit_runs_total",
        "Total job runs completed, labeled by job name and terminal status"
    );
    metrics::describe_histogram!(
        "cronduit_run_duration_seconds",
        "Duration of completed job runs in seconds, labeled by job name"
    );
    metrics::describe_counter!(
        "cronduit_run_failures_total",
        "Total job run failures, labeled by closed-enum reason"
    );

    handle
}
