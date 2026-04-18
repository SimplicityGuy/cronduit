use std::path::PathBuf;

pub mod check;
pub mod health;
pub mod run;

#[derive(clap::Parser, Debug)]
#[command(
    name = "cronduit",
    version,
    about = "Self-hosted Docker-native cron scheduler"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config file (used by `run`; `check` takes it as positional).
    #[arg(long, short = 'c', global = true)]
    pub config: Option<PathBuf>,

    /// Database URL (overrides config file value). e.g. sqlite://data/cronduit.db
    #[arg(long, global = true)]
    pub database_url: Option<String>,

    /// Bind address (overrides [server].bind). e.g. 127.0.0.1:8080
    #[arg(long, global = true)]
    pub bind: Option<String>,

    /// Log format (D-03: json is default).
    #[arg(long, global = true, default_value = "json")]
    pub log_format: LogFormatArg,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Run the cronduit daemon (loads config, migrates DB, serves web UI).
    Run,
    /// Validate a config file without touching the database.
    Check {
        /// Path to the config file to validate.
        config: PathBuf,
    },
    /// Probe the local /health endpoint and exit 0 if status="ok".
    /// Intended as a Dockerfile HEALTHCHECK target. Reuses the global
    /// `--bind` flag (default 127.0.0.1:8080). Does NOT read --config (D-04).
    Health,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum LogFormatArg {
    Json,
    Text,
}

pub async fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Command::Run => run::execute(&cli).await,
        Command::Check { config } => check::execute(config).await,
        Command::Health => health::execute(&cli).await,
    }
}
