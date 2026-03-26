pub mod tools;
pub mod notifications;

use crate::client::MatrixBridgeClient;
use crate::config::Config;
use crate::error::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Run the MCP server over stdin/stdout.
pub async fn run_server() -> Result<()> {
    let config = Config::load()?;
    let mention_pattern = config.mention_pattern();
    let mut client = MatrixBridgeClient::restore(&config).await?;

    info!("starting background sync...");
    client.start_sync().await?;

    let client = Arc::new(RwLock::new(client));

    // Register mention detection handler
    notifications::register_mention_handler(&client, mention_pattern).await;

    info!("MCP server starting on stdio");
    tools::serve(client).await?;

    Ok(())
}
