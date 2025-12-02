//! Custom LSP requests for CodeGraph-specific features.
//!
//! Tower-LSP handles custom requests through the request method on LanguageServer trait.

use crate::backend::CodeGraphBackend;
use crate::handlers::*;
use serde_json::Value;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::request::Request;

/// Custom request handler dispatcher
impl CodeGraphBackend {
    pub async fn handle_custom_request(&self, method: &str, params: Value) -> Result<Value> {
        match method {
            "codegraph/getDependencyGraph" => {
                let params: DependencyGraphParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_dependency_graph(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/getCallGraph" => {
                let params: CallGraphParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_call_graph(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/analyzeImpact" => {
                let params: ImpactAnalysisParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_analyze_impact(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/getParserMetrics" => {
                let params: ParserMetricsParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_parser_metrics(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/reindexWorkspace" => {
                self.handle_reindex_workspace().await?;
                Ok(Value::Null)
            }

            "codegraph/getAIContext" => {
                let params: AIContextParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_ai_context(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/getNodeLocation" => {
                let params: GetNodeLocationParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_node_location(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            "codegraph/getWorkspaceSymbols" => {
                let params: WorkspaceSymbolsParams = serde_json::from_value(params)
                    .map_err(|e| Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_workspace_symbols(params).await?;
                serde_json::to_value(response)
                    .map_err(|e| Error::internal_error())
            }

            _ => Err(Error::method_not_found()),
        }
    }

    /// Handle reindex workspace request
    async fn handle_reindex_workspace(&self) -> Result<()> {
        tracing::info!("Reindexing workspace");

        // Clear current graph and indexes
        {
            let mut graph = self.graph.write().await;
            *graph = codegraph::CodeGraph::in_memory().expect("Failed to create in-memory graph");
        }
        self.symbol_index.clear();
        self.file_cache.clear();

        self.client
            .log_message(tower_lsp::lsp_types::MessageType::INFO, "Workspace reindexed")
            .await;

        Ok(())
    }
}
