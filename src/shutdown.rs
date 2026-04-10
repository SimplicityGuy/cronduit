use tokio::signal;
use tokio_util::sync::CancellationToken;

pub fn install(token: CancellationToken) {
    tokio::spawn(async move {
        let ctrl_c = async {
            let _ = signal::ctrl_c().await;
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
            _ = ctrl_c => tracing::info!("received ctrl_c, shutting down"),
            _ = term   => tracing::info!("received SIGTERM, shutting down"),
        }
        token.cancel();
    });
}
