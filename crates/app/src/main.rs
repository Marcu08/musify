mod backend;
mod logging;

use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::{BoxMakeWriter, MakeWriterExt};

fn main() -> anyhow::Result<()> {
    let data_dir = backend::data_dir();
    let log_dir = data_dir.join("logs");
    let writer: BoxMakeWriter =
        match logging::RotatingLogs::new(log_dir.clone(), logging::MAX_BYTES, logging::MAX_FILES) {
            Ok(logs) => BoxMakeWriter::new(std::io::stdout.and(logs)),
            Err(error) => {
                eprintln!(
                    "Musify: impossibile aprire i log in {}: {error}",
                    log_dir.display()
                );
                BoxMakeWriter::new(std::io::stdout)
            }
        };
    let mut filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // HTTP clients can log full request URLs at debug/trace level. Keep them
    // quiet even if RUST_LOG is set, so the Jamendo client id (a query
    // parameter) never reaches the console or the log file.
    for directive in ["reqwest=warn", "hyper=warn", "h2=warn", "rustls=warn"] {
        filter = filter.add_directive(directive.parse()?);
    }
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %data_dir.display(),
        log_dir = %log_dir.display(),
        "Musify avviato"
    );
    let backend = backend::AppBackend::new().context("cannot start Musify")?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %backend.data_dir().display(),
        "Musify pronto"
    );
    musify_ui::run(Arc::new(backend));
    Ok(())
}
