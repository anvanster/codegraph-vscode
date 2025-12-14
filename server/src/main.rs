//! CodeGraph LSP Server Entry Point
//!
//! This is the main entry point for the CodeGraph Language Server.
//! It initializes the server and starts listening for LSP messages on stdio.

use codegraph_lsp::CodeGraphBackend;
use tower_lsp::{LspService, Server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codegraph_lsp=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting CodeGraph LSP server");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(CodeGraphBackend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
