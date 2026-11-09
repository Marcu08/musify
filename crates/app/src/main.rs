mod backend;

use std::sync::Arc;

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    let backend = backend::AppBackend::new().context("cannot start Musify")?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        data_dir = %backend.data_dir().display(),
        "Musify avviato"
    );
    musify_ui::run(Arc::new(backend));
    Ok(())
}
