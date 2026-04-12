use crate::scheduler::cmd::SchedulerCmd;
use tokio::signal;
use tokio_util::sync::CancellationToken;

/// Install a SIGHUP handler that triggers config reload (RELOAD-01).
///
/// Sends `SchedulerCmd::Reload` through the provided channel on each SIGHUP.
/// The `response_tx` is a fire-and-forget oneshot (SIGHUP has no caller to
/// receive the result).
#[cfg(unix)]
pub fn install_sighup(cmd_tx: tokio::sync::mpsc::Sender<SchedulerCmd>) {
    tokio::spawn(async move {
        let mut sig = signal::unix::signal(signal::unix::SignalKind::hangup())
            .expect("install SIGHUP handler");
        loop {
            sig.recv().await;
            tracing::info!(target: "cronduit.reload", "SIGHUP received, triggering config reload");
            let (resp_tx, _resp_rx) = tokio::sync::oneshot::channel();
            if cmd_tx
                .send(SchedulerCmd::Reload {
                    response_tx: resp_tx,
                })
                .await
                .is_err()
            {
                tracing::debug!(
                    target: "cronduit.reload",
                    "scheduler channel closed, stopping SIGHUP handler"
                );
                break;
            }
        }
    });
}

#[cfg(not(unix))]
pub fn install_sighup(_cmd_tx: tokio::sync::mpsc::Sender<SchedulerCmd>) {
    tracing::warn!(
        target: "cronduit.reload",
        "SIGHUP not available on this platform; use POST /api/reload instead"
    );
}

/// Install signal handlers for graceful + force shutdown.
///
/// - First SIGINT/SIGTERM: cancels the token (scheduler begins draining)
/// - Second SIGINT/SIGTERM: calls std::process::exit(1) immediately
pub fn install(token: CancellationToken) {
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!(target: "cronduit.shutdown", "received signal, initiating graceful shutdown");
        token.cancel();

        // Wait for second signal
        wait_for_signal().await;
        tracing::warn!(target: "cronduit.shutdown", "received second signal, forcing immediate exit");
        std::process::exit(1);
    });
}

async fn wait_for_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let term = async {
        let mut sig = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        sig.recv().await;
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
}
