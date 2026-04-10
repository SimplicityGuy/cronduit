use clap::Parser;
use cronduit::cli::{self, Cli, LogFormatArg};
use cronduit::telemetry::{self, LogFormat};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_format = match cli.log_format {
        LogFormatArg::Json => LogFormat::Json,
        LogFormatArg::Text => LogFormat::Text,
    };
    telemetry::init(log_format);

    let exit_code = cli::dispatch(cli).await?;
    std::process::exit(exit_code);
}
