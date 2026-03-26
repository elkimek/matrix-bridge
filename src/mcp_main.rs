// Thin entry point for `matrix-bridge-mcp` binary.
// Equivalent to `matrix-bridge mcp-server`.

mod cli;
mod client;
mod config;
mod error;
mod format;
mod mcp;
mod trust;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn,matrix_bridge=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    mcp::run_server().await?;
    Ok(())
}
