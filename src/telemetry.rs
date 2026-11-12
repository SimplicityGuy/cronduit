use std::sync::OnceLock;

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
    // WR-01 fix: memoize the installed PrometheusHandle via OnceLock so repeated
    // calls (e.g. multiple integration tests in the same test binary) always
    // return the same handle that is actually attached to the global
    // `metrics::` facade. The previous fallback branch (`build_recorder().handle()`)
    // silently returned a detached handle that rendered an empty body because
    // facade-routed `describe_*`/`gauge!`/`counter!`/`histogram!` calls went to
    // the already-installed global recorder, not the detached one. Memoization
    // also preserves the configured histogram buckets on every call, which the
    // old fallback silently dropped.
    static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

    HANDLE
        .get_or_init(|| {
            let handle = PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("cronduit_run_duration_seconds".to_string()),
                    &[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0],
                )
                .expect("valid bucket config")
                // Phase 20 / WH-11 / D-24 + RESEARCH §4.4: operator-tuned
                // bucket boundaries for per-attempt webhook HTTP duration.
                // 50ms..10s — top bucket matches reqwest's per-attempt
                // timeout (P18 D-18). Eight buckets cover the typical
                // homelab response-time range (LAN: 50-250ms; WAN: 1-5s;
                // DNS-lookup-blocked / firewalled: timeout at 10s).
                .set_buckets_for_metric(
                    Matcher::Full("cronduit_webhook_delivery_duration_seconds".to_string()),
                    &[0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
                )
                .expect("valid webhook bucket config")
                .install_recorder()
                .expect("metrics recorder not yet installed");

            // GAP-1: eagerly describe all cronduit metric families so the Prometheus
            // exporter renders HELP/TYPE lines from boot (not just after first observation).
            //
            // Note: in `metrics-exporter-prometheus` 0.18, `describe_*` only populates the
            // HELP/TYPE metadata table. The exporter will not render a metric family in
            // the `/metrics` body until that family has also been *registered* in the
            // underlying registry via a handle construction. We achieve that by calling
            // the `gauge!`/`counter!`/`histogram!` macros (which return a handle and
            // register the metric) paired with a zero-valued observation so the metric
            // exists in the registry from boot. Later `.set()`/`.increment()` calls in
            // sync.rs / run.rs overwrite or accumulate on top of this zero baseline.
            //
            // For metrics that normally carry labels (runs_total, run_duration_seconds,
            // run_failures_total) we register a base family with zero labels — this
            // installs the HELP/TYPE lines in the render output; labeled samples appear
            // once the first labeled observation is recorded in run.rs.
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
            metrics::describe_counter!(
                "cronduit_webhook_delivery_dropped_total",
                "Total webhook events dropped because the bounded delivery channel was \
                 saturated. Closed-cardinality (no labels in P15). Increments correlate \
                 with WARN-level events on the cronduit.webhooks tracing target. \
                 PRESERVED in P20 per D-26 — this counts CHANNEL-SATURATION drops \
                 (scheduler-side `try_send` failure under sustained backpressure) \
                 and is semantically DISTINCT from the P20 \
                 `cronduit_webhook_deliveries_total{status=\"dropped\"}` family \
                 which counts DRAIN-ON-SHUTDOWN drops at SIGTERM budget expiry."
            );
            // Phase 20 / WH-11 / D-22+D-24+D-25 — labeled webhook delivery family.
            //
            // BREAKING CHANGE: replaces the unlabeled P18 `_sent_total` and
            // `_failed_total` flat counters. Operators dashboard queries that
            // referenced those names need to migrate to:
            //   sum by (status) (cronduit_webhook_deliveries_total)
            // Documented in docs/WEBHOOKS.md § Metrics + the rc.1 release notes.
            //
            // The `status` label is a CLOSED ENUM with exactly three values:
            // {success, failed, dropped} — ALWAYS set from `&'static str`
            // literals, NEVER from response status codes or runtime strings
            // (T-20-05 cardinality mitigation, Pitfall 5). Reason granularity
            // (4xx vs 5xx vs network vs timeout) lives in the
            // `webhook_deliveries.dlq_reason` SQL audit column, NOT in metric
            // labels.
            metrics::describe_counter!(
                "cronduit_webhook_deliveries_total",
                "Total webhook delivery outcomes labeled by job + closed-enum status \
                 (success/failed/dropped). Replaces the P18 _sent_total/_failed_total \
                 unlabeled counters per Phase 20 D-22 (BREAKING). Note: \
                 `_delivery_dropped_total` (P15) is a SEPARATE counter for channel- \
                 saturation drops (D-26)."
            );
            metrics::describe_histogram!(
                "cronduit_webhook_delivery_duration_seconds",
                "Duration of a single webhook HTTP attempt in seconds (NOT the \
                 full retry chain wall time), labeled by job. Buckets cover \
                 50ms..10s; the 10s top-bucket matches reqwest's per-attempt \
                 timeout cap (P18 D-18)."
            );
            metrics::describe_gauge!(
                "cronduit_webhook_queue_depth",
                "Current depth of the webhook delivery channel, sampled by the \
                 worker on each rx.recv() boundary (D-25). Approximate under \
                 contention — mpsc::Receiver::len() is a snapshot, not a \
                 strongly-consistent count (Pitfall 7)."
            );
            metrics::describe_gauge!(
                "cronduit_docker_reachable",
                "1 if the docker daemon was reachable at last ping, 0 otherwise (Phase 8 D-12)"
            );

            // Force registration of each family in the Prometheus registry by creating a
            // handle (and, where a zero baseline is semantically safe, recording it).
            // Without this step the describe_* metadata is stored but the exporter has
            // nothing to attach HELP/TYPE lines to and the families silently disappear
            // from /metrics body until their first observation.
            metrics::gauge!("cronduit_scheduler_up").set(0.0);
            metrics::gauge!("cronduit_jobs_total").set(0.0);
            metrics::counter!("cronduit_runs_total").increment(0);
            metrics::histogram!("cronduit_run_duration_seconds").record(0.0);
            metrics::counter!("cronduit_run_failures_total").increment(0);
            // P15 channel-saturation counter PRESERVED per D-26.
            metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0);
            // Phase 20 / WH-11 / D-24 + D-25: histogram + gauge zero-baselines
            // so /metrics renders the family from boot (mirrors the existing
            // cronduit_run_duration_seconds zero-baseline pattern above).
            metrics::histogram!("cronduit_webhook_delivery_duration_seconds").record(0.0);
            metrics::gauge!("cronduit_webhook_queue_depth").set(0.0);
            metrics::gauge!("cronduit_docker_reachable").set(0.0);

            // Phase 20 / WH-11 / D-22 + D-23: closed-enum status pre-seed for
            // the labeled webhook delivery family. Only the `status` dimension
            // is seeded here (no `job` label) — per-job × per-status seeding
            // lives in src/cli/run.rs AFTER `sync_result.jobs` is in scope
            // (Plan 06 owns that wiring per RESEARCH §4.6). The intent here is
            // identical to the cronduit_runs_total status seed above:
            // dashboards keying off `cronduit_webhook_deliveries_total{status="dropped"}`
            // see the row materialize at boot, not first-shutdown-with-drain.
            for status in ["success", "failed", "dropped"] {
                metrics::counter!(
                    "cronduit_webhook_deliveries_total",
                    "status" => status.to_string(),
                )
                .increment(0);
            }

            handle
        })
        .clone()
}

/// Terminal `status` label values of `cronduit_runs_total{job,status}`; mirrors the
/// status strings `scheduler::run::run_job` finalizes a run with.
pub const RUN_STATUSES: [&str; 6] = [
    "success",
    "failed",
    "timeout",
    "cancelled",
    "error",
    "stopped",
];

/// Zero-seed the per-job run counters for the given job names.
///
/// Seeds `cronduit_runs_total{job,status}` for every terminal status and
/// `cronduit_run_failures_total{job,reason}` for every closed-enum reason.
/// Prometheus `increase()`/`rate()` read 0 for a series whose first sample is
/// already 1, so without a prior 0 sample the first failure of a job (and the
/// first after each restart) is invisible to `increase(...) > 0` alerts.
///
/// Called from the two places a configured job set is established: boot
/// (`cli::run`) and every config reload (`scheduler::reload`). It is idempotent
/// (`increment(0)` never changes a live value), so re-seeding on each reload is
/// safe and gives newly added jobs their series without a restart.
///
/// Removed or disabled jobs: the `metrics` facade cannot delete series, so
/// their series stay on `/metrics` frozen at their last value until restart.
/// This is harmless (a frozen counter has `increase() == 0`) and bounded by
/// the number of distinct job names seen since boot.
///
/// Must run after [`setup_metrics`] has installed the recorder.
pub fn seed_job_metrics<'a>(job_names: impl IntoIterator<Item = &'a str>) {
    for name in job_names {
        for status in RUN_STATUSES {
            metrics::counter!(
                "cronduit_runs_total",
                "job" => name.to_string(),
                "status" => status,
            )
            .increment(0);
        }
        for reason in crate::scheduler::run::FailureReason::ALL {
            metrics::counter!(
                "cronduit_run_failures_total",
                "job" => name.to_string(),
                "reason" => reason.as_label(),
            )
            .increment(0);
        }
    }
}
