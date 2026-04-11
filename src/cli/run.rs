use crate::cli::Cli;
use crate::config;
use crate::db::{DbBackend, DbPool, strip_db_credentials};
use crate::shutdown;
use crate::web::{self, AppState};
use secrecy::ExposeSecret;
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
    let resolved_db_url: String = match &cli.database_url {
        Some(flag) => {
            tracing::info!(
                field = "database_url",
                from_config = "<redacted>",
                from_cli = "<redacted>",
                "CLI flag overrides config file"
            );
            flag.clone()
        }
        None => cfg.server.database_url.expose_secret().to_string(),
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
    let pool = DbPool::connect(&resolved_db_url).await?;
    pool.migrate().await?;

    // 5. Sync config to DB and parse timezone.
    let tz: chrono_tz::Tz = cfg
        .server
        .timezone
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid timezone: {}", cfg.server.timezone))?;

    let sync_result = crate::scheduler::sync::sync_config_to_db(&pool, &parsed.config).await?;

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
        database_url = %strip_db_credentials(&resolved_db_url),
        config_path = %config_path.display(),
        timezone = %cfg.server.timezone,
        job_count = sync_result.jobs.len(),
        disabled_job_count = sync_result.disabled,
        bind_warning,
        "cronduit starting"
    );

    // 7. Wire graceful shutdown + spawn scheduler + serve.
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<crate::scheduler::cmd::SchedulerCmd>(32);

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION"),
        pool: pool.clone(),
        cmd_tx,
        config_path: config_path.clone(),
    };
    let cancel = CancellationToken::new();
    shutdown::install(cancel.clone());

    // Spawn the scheduler loop.
    let scheduler_handle = crate::scheduler::spawn(
        pool.clone(),
        sync_result.jobs,
        tz,
        cancel.clone(),
        cfg.server.shutdown_grace,
        cmd_rx,
    );

    // Serve web (blocks until cancel).
    let serve_result = web::serve(resolved_bind, state, cancel).await;

    // 8. Wait for scheduler to drain (Plan 04 will add timeout).
    let _ = scheduler_handle.await;

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
