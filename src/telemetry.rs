use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    Json,
    Text,
}

pub fn init(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cronduit=debug"));

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
