use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize logging: {error}"))?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Musify avviato (M0: workspace pronto)"
    );
    println!(
        "Musify {} — M0: workspace pronto",
        env!("CARGO_PKG_VERSION")
    );
    Ok(())
}
