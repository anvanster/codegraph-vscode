//! Navigation-related helper functions.

use crate::backend::CodeGraphBackend;
use codegraph::NodeId;
use serde::{Deserialize, Serialize};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{Range, Url};

/// Request to get a node's location by ID.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNodeLocationParams {
    pub node_id: String,
}

/// Response with node location.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeLocationResponse {
    pub uri: String,
    pub range: Range,
}

impl CodeGraphBackend {
    /// Get the location of a node by its ID.
    pub async fn handle_get_node_location(
        &self,
        params: GetNodeLocationParams,
    ) -> Result<Option<NodeLocationResponse>> {
        let node_id: NodeId = params
            .node_id
            .parse()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid node ID"))?;

        let graph = self.graph.read().await;

        let node = match graph.get_node(node_id) {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };

        let path = match node.properties.get_string("path") {
            Some(p) => p,
            None => return Ok(None),
        };

        let start_line: u32 = node
            .properties
            .get_int("start_line")
            .map(|v| v as u32)
            .unwrap_or(1)
            .saturating_sub(1);

        let start_col: u32 = node
            .properties
            .get_int("start_col")
            .map(|v| v as u32)
            .unwrap_or(0);

        let end_line: u32 = node
            .properties
            .get_int("end_line")
            .map(|v| v as u32)
            .unwrap_or(start_line + 1)
            .saturating_sub(1);

        let end_col: u32 = node
            .properties
            .get_int("end_col")
            .map(|v| v as u32)
            .unwrap_or(0);

        let uri = Url::from_file_path(path)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid path"))?;

        Ok(Some(NodeLocationResponse {
            uri: uri.to_string(),
            range: Range {
                start: tower_lsp::lsp_types::Position {
                    line: start_line,
                    character: start_col,
                },
                end: tower_lsp::lsp_types::Position {
                    line: end_line,
                    character: end_col,
                },
            },
        }))
    }
}

/// Request for workspace symbols.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbolsParams {
    pub query: Option<String>,
}

/// Symbol information for tree view.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub language: String,
    pub uri: String,
    pub range: Range,
    pub children: Option<Vec<SymbolInfo>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbolsResponse {
    pub symbols: Vec<SymbolInfo>,
}

impl CodeGraphBackend {
    /// Get workspace symbols, optionally filtered by query.
    pub async fn handle_get_workspace_symbols(
        &self,
        params: WorkspaceSymbolsParams,
    ) -> Result<WorkspaceSymbolsResponse> {
        let graph = self.graph.read().await;

        let node_ids = if let Some(query) = &params.query {
            if query.is_empty() {
                // Return top-level symbols (modules, files)
                self.symbol_index.get_by_type("Module")
            } else {
                self.symbol_index.search_by_name(query)
            }
        } else {
            // Return all symbols (limited)
            let mut all = Vec::new();
            all.extend(self.symbol_index.get_by_type("Function"));
            all.extend(self.symbol_index.get_by_type("Class"));
            all.extend(self.symbol_index.get_by_type("Module"));
            all.truncate(100); // Limit results
            all
        };

        let mut symbols = Vec::new();

        for node_id in node_ids {
            if let Ok(node) = graph.get_node(node_id) {
                let name = node.properties.get_string("name").unwrap_or("").to_string();
                let kind = format!("{:?}", node.node_type);
                let language = node
                    .properties
                    .get_string("language")
                    .unwrap_or("unknown")
                    .to_string();
                let path = node.properties.get_string("path").unwrap_or("").to_string();

                let start_line: u32 = node
                    .properties
                    .get_int("start_line")
                    .map(|v| v as u32)
                    .unwrap_or(1)
                    .saturating_sub(1);

                let start_col: u32 = node
                    .properties
                    .get_int("start_col")
                    .map(|v| v as u32)
                    .unwrap_or(0);

                let end_line: u32 = node
                    .properties
                    .get_int("end_line")
                    .map(|v| v as u32)
                    .unwrap_or(start_line + 1)
                    .saturating_sub(1);

                let end_col: u32 = node
                    .properties
                    .get_int("end_col")
                    .map(|v| v as u32)
                    .unwrap_or(0);

                let uri = if !path.is_empty() {
                    Url::from_file_path(&path)
                        .map(|u| u.to_string())
                        .unwrap_or(path.clone())
                } else {
                    String::new()
                };

                symbols.push(SymbolInfo {
                    id: node_id.to_string(),
                    name,
                    kind,
                    language,
                    uri,
                    range: Range {
                        start: tower_lsp::lsp_types::Position {
                            line: start_line,
                            character: start_col,
                        },
                        end: tower_lsp::lsp_types::Position {
                            line: end_line,
                            character: end_col,
                        },
                    },
                    children: None,
                });
            }
        }

        Ok(WorkspaceSymbolsResponse { symbols })
    }
}
