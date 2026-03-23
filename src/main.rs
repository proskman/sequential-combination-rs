// main.rs - Entry Point (CORRECTED - API rmcp 0.1 verified)
// FIX: Uses (stdin(), stdout()) transport instead of transport::stdio()
// All logs go to stderr to prevent JSON-RPC stdout corruption.

mod server;
mod skills_index;
mod dna_extractor;
mod config_loader;

use anyhow::Result;
use rmcp::Server;
use server::SequentialCombinationServer;
use tokio::io::{stdin, stdout};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: route ALL logs to STDERR only
    // stdout must stay clean for JSON-RPC messages (fixes VSCode restart loop)
    fmt()
        .with_env_filter(
            EnvFilter::try_from_env("RUST_LOG")
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    info!("🦀 Sequential Combination MCP (Rust) - Starting...");
    info!("📂 Initializing skills index and embedding model...");

    let server = SequentialCombinationServer::new().await?;
    info!("✅ Server ready — listening on stdio.");

    let server_task = Server::new(stdin(), stdout(), Arc::new(server));
    info!("✅ Server processing...");
    server_task.run().await?;
    
    Ok(())
}
