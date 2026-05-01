use crate::cli::Cli;
use crate::config;
use crate::db::{DbBackend, DbPool, queries, strip_db_credentials};
use crate::shutdown;
use crate::web::{self, AppState};
use secrecy::{ExposeSecret, SecretString};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use tokio_util::sync::CancellationToken;

pub async fn execute(cli: &Cli) -> anyhow::Result<i32> {
    // 1. Resolve config path (CLI override -> env default).
    let config_path: PathBuf = cli
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from("/etc/cronduit/config.toml"));

    // 2. Parse + validate (shared pipeline; does not touch DB).
    let parsed = match config::parse_and_validate(&config_path) {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                eprintln!("{e}");
            }
            eprintln!();
            eprintln!("{} error(s)", errors.len());
            return Ok(1);
        }
    };
    let cfg = &parsed.config;

    // 3. Apply CLI overrides with info-level tracing.
    let resolved_db_url: SecretString = match &cli.database_url {
        Some(flag) => {
            tracing::info!(
                field = "database_url",
                from_config = "<redacted>",
                from_cli = "<redacted>",
                "CLI flag overrides config file"
            );
            SecretString::from(flag.clone())
        }
        None => cfg.server.database_url.clone(),
    };

    let resolved_bind_str: String = match &cli.bind {
        Some(flag) => {
            tracing::info!(
                field = "bind",
                from_config = %cfg.server.bind,
                from_cli = %flag,
                "CLI flag overrides config file"
            );
            flag.clone()
        }
        None => cfg.server.bind.clone(),
    };
    let resolved_bind: SocketAddr = SocketAddr::from_str(&resolved_bind_str)?;

    // 4. Open DB pool and run migrations (idempotent per DB-03).
    let pool = DbPool::connect(resolved_db_url.expose_secret()).await?;
    pool.migrate().await?;

    // Phase 11 D-15 (verbatim from CONTEXT.md + ROADMAP): assert post-migration
    // that every job_runs row has a non-null job_run_number. In production this
    // can never fire (D-12 two-phase startup + D-14 fail-fast on migration error
    // + file-3 NOT NULL constraint all enforce it). In tests it guards against
    // future regressions that let the scheduler spawn against unbackfilled rows.
    //
    // Locked decision: CONTEXT.md D-15 says literally "Panic with a clear
    // message if not." — we use panic!(), NOT anyhow::bail, to honor that
    // wording. The message identifies the NULL count and the recovery path
    // (restart is recoverable because backfill is idempotent).
    let null_count = queries::count_job_runs_with_null_run_number(&pool)
        .await
        .expect("count_job_runs_with_null_run_number query must succeed");
    if null_count > 0 {
        panic!(
            "Phase 11 backfill invariant violated: {} job_runs rows have NULL \
             job_run_number after migration. Aborting scheduler startup to \
             prevent inconsistent state. Re-run cronduit to retry backfill — \
             file 2 (backfill) is idempotent on WHERE job_run_number IS NULL.",
            null_count
        );
    }

    // 5. Sync config to DB and parse timezone.
    let tz: chrono_tz::Tz = cfg
        .server
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid timezone: {}", cfg.server.timezone))?;

    let random_min_gap = cfg
        .defaults
        .as_ref()
        .and_then(|d| d.random_min_gap)
        .unwrap_or(std::time::Duration::from_secs(0));
    let sync_result =
        crate::scheduler::sync::sync_config_to_db(&pool, &parsed.config, random_min_gap).await?;

    // 6. Emit startup event (D-23) + bind warning (D-24) BEFORE serve.
    let backend = match pool.backend() {
        DbBackend::Sqlite => "sqlite",
        DbBackend::Postgres => "postgres",
    };
    let bind_warning = !is_loopback(&resolved_bind);
    if bind_warning {
        tracing::warn!(
            target: "cronduit.startup",
            bind = %resolved_bind,
            "web UI bound to non-loopback address — v1 ships without authentication; \
             see README SECURITY and THREAT_MODEL.md. Put cronduit behind a reverse proxy \
             with auth, or keep it on 127.0.0.1."
        );
    }

    tracing::info!(
        target: "cronduit.startup",
        version = env!("CARGO_PKG_VERSION"),
        bind = %resolved_bind,
        database_backend = backend,
        database_url = %strip_db_credentials(resolved_db_url.expose_secret()),
        config_path = %config_path.display(),
        timezone = %cfg.server.timezone,
        job_count = sync_result.jobs.len(),
        disabled_job_count = sync_result.disabled,
        bind_warning,
        "cronduit starting"
    );

    // 7. Wire graceful shutdown + spawn scheduler + serve.
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<crate::scheduler::cmd::SchedulerCmd>(32);

    let cancel = CancellationToken::new();
    shutdown::install(cancel.clone());
    shutdown::install_sighup(cmd_tx.clone());

    // File watcher for automatic config reload (D-10, RELOAD-03).
    if cfg.server.watch_config
        && let Err(e) =
            crate::scheduler::reload::spawn_file_watcher(config_path.clone(), cmd_tx.clone())
    {
        tracing::warn!(
            target: "cronduit.startup",
            error = %e,
            "failed to start config file watcher -- file-based reload unavailable"
        );
    }

    // Initialize Prometheus metrics recorder (OPS-02).
    let metrics_handle = crate::telemetry::setup_metrics();
    metrics::gauge!("cronduit_scheduler_up").set(1.0);

    let active_runs =
        std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION"),
        pool: pool.clone(),
        cmd_tx,
        config_path: config_path.clone(),
        tz,
        last_reload: std::sync::Arc::new(std::sync::Mutex::new(None)),
        metrics_handle,
        watch_config: cfg.server.watch_config,
        active_runs: active_runs.clone(),
    };

    // Create Docker client (non-fatal if unavailable).
    //
    // Uses `connect_with_defaults()` (NOT `connect_with_local_defaults()`)
    // because the latter only honors `DOCKER_HOST` when it starts with
    // `unix://` — TCP URIs like `tcp://dockerproxy:2375` are silently
    // ignored and bollard falls back to `/var/run/docker.sock`, which is
    // not mounted in `examples/docker-compose.secure.yml`'s cronduit
    // container. `connect_with_defaults()` routes on the URL scheme and
    // honors unix/tcp/http/ssh uniformly, matching the docker CLI.
    //
    // Historical note: this was regressed by the Phase 8 secure-compose CI
    // matrix (08-04) which exposed the bug — the CI found that every
    // docker-type job errored with "docker executor unavailable" on the
    // secure axis because the proxy-sidecar DOCKER_HOST was ignored.
    let docker = match bollard::Docker::connect_with_defaults() {
        Ok(d) => {
            tracing::info!(target: "cronduit.startup", "Docker client connected");
            Some(d)
        }
        Err(e) => {
            tracing::warn!(
                target: "cronduit.startup",
                error = %e,
                "Docker client unavailable -- docker-type jobs will fail"
            );
            None
        }
    };

    // Phase 8 D-11: run the daemon ping once at startup so the operator sees a
    // loud WARN if bollard cannot reach the Docker socket, and so the
    // cronduit_docker_reachable gauge reflects startup state. Non-fatal — cronduit
    // keeps booting regardless so command/script-only configs still work.
    //
    // Note: `bollard::Docker::connect_with_defaults()` (above) reads the
    // `DOCKER_HOST` environment variable on EVERY scheme (unix://, tcp://,
    // http://, ssh://). This is how `examples/docker-compose.secure.yml`
    // routes bollard to the `docker-socket-proxy` sidecar via
    // `DOCKER_HOST=tcp://dockerproxy:2375` — the connect function matches
    // the docker CLI's resolution semantics.
    crate::scheduler::docker_daemon::preflight_ping(docker.as_ref()).await;

    // Orphan reconciliation before scheduler starts (SCHED-08).
    if let Some(ref docker_client) = docker {
        match crate::scheduler::docker_orphan::reconcile_orphans(docker_client, &pool).await {
            Ok(count) if count > 0 => {
                tracing::info!(
                    target: "cronduit.startup",
                    orphans = count,
                    "orphan reconciliation complete"
                );
            }
            Ok(_) => {} // No orphans found -- no log needed
            Err(e) => {
                tracing::error!(
                    target: "cronduit.startup",
                    error = %e,
                    "orphan reconciliation failed"
                );
            }
        }
    }

    // Spawn the daily retention pruner (DB-08).
    tokio::spawn(crate::scheduler::retention::retention_pruner(
        pool.clone(),
        cfg.server.log_retention,
        cancel.clone(),
    ));

    // Phase 15 / WH-02 / D-03 — always-on webhook delivery worker.
    // NoopDispatcher in P15; P18 swaps in HttpDispatcher against the same
    // trait. The worker's lifetime is owned by this bin layer: scheduler
    // shutdown fires the cancel token, and we await the worker's JoinHandle
    // AFTER the scheduler finishes draining. Order matters — awaiting the
    // worker before the scheduler drains would race finalize_run's last
    // try_send calls with the worker's exit and produce noisy
    // TrySendError::Closed errors in production logs.

    // Phase 18 / WH-01..09 — build the per-job webhook map from validated config.
    // The map is keyed by post-sync DbJob.id; values are clones of the
    // (already-defaults-merged) WebhookConfig from JobConfig.webhook. Empty map
    // means no webhook is configured anywhere — keep NoopDispatcher to avoid
    // spinning a reqwest::Client. Per RESEARCH Open Q 1, this map is the
    // SOLE source of webhook config at delivery time; no DB round-trip
    // through `config_json` (5-layer parity exempt — Plan 02 must-haves).
    //
    // ALIGNMENT (T-18-36 mitigation): Building the map by NAME-keyed lookup
    // rather than blind `zip(cfg.jobs, sync_result.jobs)` because index-aligning
    // those two slices is fragile — if `sync_config_to_db` ever reorders or
    // filters (e.g., to skip a job), a single-job test cannot detect the
    // mis-wiring. Name lookup makes the alignment explicit.
    let webhooks: std::collections::HashMap<i64, crate::config::WebhookConfig> = {
        // First: build a per-name view of cfg.jobs[].webhook (only the entries
        // that actually have a webhook configured).
        let by_name: std::collections::HashMap<&str, &crate::config::WebhookConfig> = cfg
            .jobs
            .iter()
            .filter_map(|j| j.webhook.as_ref().map(|wh| (j.name.as_str(), wh)))
            .collect();
        // Then: for each post-sync DbJob, resolve its WebhookConfig by name.
        // (Names are unique post-validation; cfg-validators reject duplicate
        // job names at LOAD time, and sync preserves the same uniqueness.)
        sync_result
            .jobs
            .iter()
            .filter_map(|db_job| {
                by_name
                    .get(db_job.name.as_str())
                    .map(|wh| (db_job.id, (*wh).clone()))
            })
            .collect()
    };

    let dispatcher: std::sync::Arc<dyn crate::webhooks::WebhookDispatcher> = if webhooks.is_empty()
    {
        // Zero webhooks configured anywhere — keep NoopDispatcher.
        // No reqwest::Client is built; no rustls TLS handshake setup overhead.
        std::sync::Arc::new(crate::webhooks::NoopDispatcher)
    } else {
        // At least one job has a webhook — build HttpDispatcher.
        let http =
            crate::webhooks::HttpDispatcher::new(pool.clone(), std::sync::Arc::new(webhooks))
                .map_err(|e| anyhow::anyhow!("HttpDispatcher init failed: {e}"))?;
        std::sync::Arc::new(http)
    };

    let (webhook_tx, webhook_rx) = crate::webhooks::channel();
    // Phase 20 / WH-10 / D-15: webhook worker drain grace on SIGTERM. Plan 06
    // owns the proper bin-layer wiring (`cfg.server.webhook_drain_grace`).
    // Until then, a 30-second default matches the locked
    // `webhook_drain_grace = "30s"` value from the spec so the runtime
    // behavior is correct out of the box; Plan 06 will replace this hardcode
    // with `cfg.server.webhook_drain_grace`.
    let webhook_drain_grace = std::time::Duration::from_secs(30);
    let webhook_worker_handle = crate::webhooks::spawn_worker(
        webhook_rx,
        dispatcher,
        cancel.child_token(),
        webhook_drain_grace,
    );

    // Spawn the scheduler loop.
    let scheduler_handle = crate::scheduler::spawn(
        pool.clone(),
        docker,
        sync_result.jobs,
        tz,
        cancel.clone(),
        cfg.server.shutdown_grace,
        cmd_rx,
        config_path.to_path_buf(),
        active_runs,
        webhook_tx,
    );

    // Serve web (blocks until cancel).
    let serve_result = web::serve(resolved_bind, state, cancel).await;

    // 8. Wait for scheduler to drain (Plan 04 will add timeout).
    let _ = scheduler_handle.await;

    // Phase 15 / WH-02 — drain the webhook worker AFTER the scheduler
    // finishes. The worker exits cleanly when the cancel token fires
    // (because cancel.child_token() above) AND when the last Sender drops
    // (which happens when SchedulerLoop is dropped at scheduler_handle
    // completion). Either path triggers the worker_loop's break.
    let _ = webhook_worker_handle.await;

    // 9. Drain pools before returning.
    pool.close().await;

    serve_result?;
    Ok(0)
}

fn is_loopback(addr: &SocketAddr) -> bool {
    match addr.ip() {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn loopback_detection() {
        assert!(is_loopback(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(127, 0, 0, 1),
            8080
        ))));
        assert!(!is_loopback(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(192, 168, 1, 10),
            8080
        ))));
        assert!(!is_loopback(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(0, 0, 0, 0),
            8080
        ))));
    }
}
