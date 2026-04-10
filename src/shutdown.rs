use tokio::signal;
use tokio_util::sync::CancellationToken;

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
