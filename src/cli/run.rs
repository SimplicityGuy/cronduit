use crate::cli::Cli;
use crate::shutdown;
use crate::web::{self, AppState};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio_util::sync::CancellationToken;

/// Run the daemon. Plan 04 replaces this with the full boot flow
/// (config parse -> DB pool -> migrate -> startup event -> serve).
pub async fn execute(cli: &Cli) -> anyhow::Result<i32> {
    let bind_str = cli.bind.as_deref().unwrap_or("127.0.0.1:8080");
    let bind = SocketAddr::from_str(bind_str)?;

    let state = AppState {
        started_at: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION"),
    };

    let cancel = CancellationToken::new();
    shutdown::install(cancel.clone());

    web::serve(bind, state, cancel).await?;
    Ok(0)
}
