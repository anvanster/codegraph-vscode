//! LSP Backend Implementation
//!
//! This module implements the Language Server Protocol for CodeGraph.

use crate::cache::QueryCache;
use crate::error::{LspError, LspResult};
use crate::index::SymbolIndex;
use crate::parser_registry::ParserRegistry;
use codegraph::{CodeGraph, Direction, EdgeType, NodeId, NodeType};
use codegraph_parser_api::FileInfo;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// CodeGraph Language Server backend.
pub struct CodeGraphBackend {
    /// LSP client for sending notifications.
    pub client: Client,

    /// The code graph database.
    pub graph: Arc<RwLock<CodeGraph>>,

    /// Parser registry for all supported languages.
    pub parsers: Arc<ParserRegistry>,

    /// File cache: URI -> FileInfo.
    pub file_cache: Arc<DashMap<Url, FileInfo>>,

    /// Query cache for performance.
    pub query_cache: Arc<QueryCache>,

    /// Symbol index for fast lookups.
    pub symbol_index: Arc<SymbolIndex>,
}

impl CodeGraphBackend {
    /// Create a new CodeGraph backend.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            graph: Arc::new(RwLock::new(
                CodeGraph::in_memory().expect("Failed to create in-memory graph"),
            )),
            parsers: Arc::new(ParserRegistry::new()),
            file_cache: Arc::new(DashMap::new()),
            query_cache: Arc::new(QueryCache::new(1000)),
            symbol_index: Arc::new(SymbolIndex::new()),
        }
    }

    /// Remove all nodes associated with a file from the graph.
    async fn remove_file_from_graph(&self, path: &std::path::Path) {
        let mut graph = self.graph.write().await;
        let path_str = path.to_string_lossy().to_string();

        // Query for all nodes with this file path using the query builder
        if let Ok(nodes) = graph.query().property("path", path_str).execute() {
            for node_id in nodes {
                let _ = graph.delete_node(node_id);
            }
        }

        // Invalidate caches
        self.query_cache.invalidate_file(&path.to_path_buf());
        self.symbol_index.remove_file(path);
    }

    /// Find node at the given position.
    /// Position line is 0-indexed (LSP style), but we convert to 1-indexed for graph.
    pub fn find_node_at_position(
        &self,
        graph: &CodeGraph,
        path: &std::path::Path,
        position: Position,
    ) -> LspResult<Option<NodeId>> {
        // LSP positions are 0-indexed, our index stores 1-indexed
        let line = (position.line + 1) as i64;
        let col = position.character as i64;

        // Try the symbol index first (faster)
        if let Some(node_id) = self.symbol_index.find_at_position(path, line as u32, col as u32) {
            return Ok(Some(node_id));
        }

        // Fallback to graph query
        let path_str = path.to_string_lossy().to_string();
        let nodes = graph
            .query()
            .property("path", path_str)
            .execute()
            .map_err(|e| LspError::Graph(e.to_string()))?;

        for node_id in nodes {
            if let Ok(node) = graph.get_node(node_id) {
                let start_line = node.properties.get_int("start_line").unwrap_or(0);
                let end_line = node.properties.get_int("end_line").unwrap_or(0);
                let start_col = node.properties.get_int("start_col").unwrap_or(0);
                let end_col = node.properties.get_int("end_col").unwrap_or(i64::MAX);

                if line >= start_line && line <= end_line {
                    if line == start_line && col < start_col {
                        continue;
                    }
                    if line == end_line && col > end_col {
                        continue;
                    }
                    return Ok(Some(node_id));
                }
            }
        }

        Ok(None)
    }

    /// Find all edges connected to a node.
    pub fn get_connected_edges(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        direction: Direction,
    ) -> Vec<(NodeId, NodeId, EdgeType)> {
        let mut edges = Vec::new();

        // Get neighbors in the specified direction
        let neighbors = match graph.get_neighbors(node_id, direction) {
            Ok(n) => n,
            Err(_) => return edges,
        };

        for neighbor_id in neighbors {
            // Get edges between this node and the neighbor
            let (source, target) = match direction {
                Direction::Outgoing => (node_id, neighbor_id),
                Direction::Incoming => (neighbor_id, node_id),
                Direction::Both => {
                    // Try both directions
                    if let Ok(edge_ids) = graph.get_edges_between(node_id, neighbor_id) {
                        for edge_id in edge_ids {
                            if let Ok(edge) = graph.get_edge(edge_id) {
                                edges.push((edge.source_id, edge.target_id, edge.edge_type));
                            }
                        }
                    }
                    if let Ok(edge_ids) = graph.get_edges_between(neighbor_id, node_id) {
                        for edge_id in edge_ids {
                            if let Ok(edge) = graph.get_edge(edge_id) {
                                edges.push((edge.source_id, edge.target_id, edge.edge_type));
                            }
                        }
                    }
                    continue;
                }
            };

            if let Ok(edge_ids) = graph.get_edges_between(source, target) {
                for edge_id in edge_ids {
                    if let Ok(edge) = graph.get_edge(edge_id) {
                        edges.push((edge.source_id, edge.target_id, edge.edge_type));
                    }
                }
            }
        }

        edges
    }

    /// Find the definition node for a reference.
    fn find_definition_for_reference(
        &self,
        graph: &CodeGraph,
        ref_node_id: NodeId,
    ) -> LspResult<Option<NodeId>> {
        let edges = self.get_connected_edges(graph, ref_node_id, Direction::Outgoing);

        for (_, target, edge_type) in edges {
            match edge_type {
                EdgeType::Calls | EdgeType::References | EdgeType::Imports => {
                    return Ok(Some(target));
                }
                _ => continue,
            }
        }

        Ok(None)
    }

    /// Convert a node to an LSP Location.
    pub fn node_to_location(&self, graph: &CodeGraph, node_id: NodeId) -> LspResult<Location> {
        let node = graph
            .get_node(node_id)
            .map_err(|e| LspError::Graph(e.to_string()))?;

        let path = node
            .properties
            .get_string("path")
            .ok_or_else(|| LspError::NodeNotFound("Missing path property".into()))?;

        let start_line = node.properties.get_int("start_line").unwrap_or(1) as u32;
        let start_col = node.properties.get_int("start_col").unwrap_or(0) as u32;
        let end_line = node.properties.get_int("end_line").unwrap_or(start_line as i64) as u32;
        let end_col = node.properties.get_int("end_col").unwrap_or(0) as u32;

        // Convert to 0-indexed
        let start_line = start_line.saturating_sub(1);
        let end_line = end_line.saturating_sub(1);

        Ok(Location {
            uri: Url::from_file_path(path).map_err(|_| LspError::InvalidUri(path.to_string()))?,
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_col,
                },
                end: Position {
                    line: end_line,
                    character: end_col,
                },
            },
        })
    }

    /// Get the source code for a node.
    pub async fn get_node_source_code(&self, node_id: NodeId) -> LspResult<Option<String>> {
        let graph = self.graph.read().await;
        let node = graph
            .get_node(node_id)
            .map_err(|e| LspError::Graph(e.to_string()))?;

        // Try to get source from node properties first
        if let Some(source) = node.properties.get_string("source") {
            return Ok(Some(source.to_string()));
        }

        // Otherwise, try to read from file
        if let Some(path_str) = node.properties.get_string("path") {
            let path = PathBuf::from(path_str);
            if path.exists() {
                let start_line = node.properties.get_int("start_line").unwrap_or(1) as usize;
                let end_line = node.properties.get_int("end_line").unwrap_or(start_line as i64) as usize;

                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    let lines: Vec<&str> = content.lines().collect();
                    if start_line > 0 && end_line <= lines.len() {
                        let source: String = lines[start_line - 1..end_line].join("\n");
                        return Ok(Some(source));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Helper to get a string property from a node
    fn get_node_string_property(&self, graph: &CodeGraph, node_id: NodeId, key: &str) -> Option<String> {
        graph.get_node(node_id).ok()?.properties.get_string(key).map(|s| s.to_string())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for CodeGraphBackend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("Initializing CodeGraph LSP server");

        // Index workspace folders
        if let Some(folders) = params.workspace_folders {
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    tracing::info!("Workspace folder: {}", path.display());
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "codegraph.getDependencyGraph".to_string(),
                        "codegraph.getCallGraph".to_string(),
                        "codegraph.analyzeImpact".to_string(),
                        "codegraph.getParserMetrics".to_string(),
                        "codegraph.reindexWorkspace".to_string(),
                        "codegraph.getAIContext".to_string(),
                        "codegraph.getNodeLocation".to_string(),
                        "codegraph.getWorkspaceSymbols".to_string(),
                    ],
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "CodeGraph LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down CodeGraph LSP server");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("Invalid URI: {}", uri);
                return;
            }
        };

        if let Some(parser) = self.parsers.parser_for_path(&path) {
            let mut graph = self.graph.write().await;

            match parser.parse_source(&text, &path, &mut graph) {
                Ok(file_info) => {
                    tracing::info!(
                        "Parse succeeded: {} functions, {} classes, {} traits, {} imports",
                        file_info.functions.len(),
                        file_info.classes.len(),
                        file_info.traits.len(),
                        file_info.imports.len()
                    );

                    // Update symbol index
                    self.symbol_index.add_file(path.clone(), &file_info, &graph);

                    // Update file cache
                    self.file_cache.insert(uri.clone(), file_info);

                    self.client
                        .log_message(MessageType::INFO, format!("Indexed: {}", uri))
                        .await;
                }
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, format!("Parse error in {}: {}", uri, e))
                        .await;
                }
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Get the full text (assuming full sync mode)
        if let Some(change) = params.content_changes.into_iter().next() {
            if let Some(parser) = self.parsers.parser_for_path(&path) {
                // Remove old entries
                self.remove_file_from_graph(&path).await;

                // Re-parse with new content
                let mut graph = self.graph.write().await;
                if let Ok(file_info) = parser.parse_source(&change.text, &path, &mut graph) {
                    self.symbol_index.add_file(path.clone(), &file_info, &graph);
                    self.file_cache.insert(uri, file_info);
                }
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        if let Some(parser) = self.parsers.parser_for_path(&path) {
            if let Some(text) = params.text {
                self.remove_file_from_graph(&path).await;

                let mut graph = self.graph.write().await;
                if let Ok(file_info) = parser.parse_source(&text, &path, &mut graph) {
                    self.symbol_index.add_file(path.clone(), &file_info, &graph);
                    self.file_cache.insert(uri, file_info);
                }
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Keep in graph for cross-file references, but remove from file cache
        self.file_cache.remove(&params.text_document.uri);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let graph = self.graph.read().await;

        // Find node at the given position
        let node_id = match self.find_node_at_position(&graph, &path, position)? {
            Some(id) => id,
            None => return Ok(None),
        };

        // Check if this is a reference - find its definition
        if let Some(def_node_id) = self.find_definition_for_reference(&graph, node_id)? {
            let location = self.node_to_location(&graph, def_node_id)?;
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }

        // Already at definition
        let location = self.node_to_location(&graph, node_id)?;
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let graph = self.graph.read().await;

        // Find node at position
        let node_id = match self.find_node_at_position(&graph, &path, position)? {
            Some(id) => id,
            None => return Ok(None),
        };

        // Find the definition
        let def_node_id = self
            .find_definition_for_reference(&graph, node_id)?
            .unwrap_or(node_id);

        let mut locations = Vec::new();

        // Include declaration if requested
        if include_declaration {
            if let Ok(loc) = self.node_to_location(&graph, def_node_id) {
                locations.push(loc);
            }
        }

        // Find all incoming edges (references to this definition)
        let edges = self.get_connected_edges(&graph, def_node_id, Direction::Incoming);

        for (source, _, _) in edges {
            if let Ok(loc) = self.node_to_location(&graph, source) {
                locations.push(loc);
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let graph = self.graph.read().await;

        let node_id = match self.find_node_at_position(&graph, &path, position)? {
            Some(id) => id,
            None => return Ok(None),
        };

        let node = graph
            .get_node(node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

        // Build hover content
        let name = node.properties.get_string("name").unwrap_or("").to_string();
        let kind = format!("{}", node.node_type);
        let signature = node.properties.get_string("signature").unwrap_or("").to_string();
        let doc = node.properties.get_string("doc").map(|s| s.to_string());
        let def_path = node.properties.get_string("path").unwrap_or("").to_string();

        // Count references
        let ref_count = self.get_connected_edges(&graph, node_id, Direction::Incoming).len();

        let mut content = format!("**{}** `{}`", kind, name);

        if !signature.is_empty() {
            content.push_str(&format!("\n\n```\n{}\n```", signature));
        }

        if let Some(doc) = doc {
            content.push_str(&format!("\n\n{}", doc));
        }

        content.push_str(&format!(
            "\n\n---\n\n**Defined in:** {}\n**References:** {}",
            def_path, ref_count
        ));

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: None,
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let graph = self.graph.read().await;

        // Get all symbols in this file
        let node_ids = self.symbol_index.get_file_symbols(&path);

        let mut symbols = Vec::new();

        for node_id in node_ids {
            if let Ok(node) = graph.get_node(node_id) {
                let name = node.properties.get_string("name").unwrap_or("").to_string();
                let kind = match node.node_type {
                    NodeType::Function => SymbolKind::FUNCTION,
                    NodeType::Class => SymbolKind::CLASS,
                    NodeType::Interface => SymbolKind::INTERFACE,
                    NodeType::Module => SymbolKind::MODULE,
                    NodeType::Variable => SymbolKind::VARIABLE,
                    NodeType::Type => SymbolKind::TYPE_PARAMETER,
                    NodeType::CodeFile => SymbolKind::FILE,
                    NodeType::Generic => SymbolKind::VARIABLE,
                };

                if let Ok(location) = self.node_to_location(&graph, node_id) {
                    #[allow(deprecated)]
                    symbols.push(SymbolInformation {
                        name,
                        kind,
                        tags: None,
                        deprecated: None,
                        location,
                        container_name: None,
                    });
                }
            }
        }

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DocumentSymbolResponse::Flat(symbols)))
        }
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let graph = self.graph.read().await;

        let node_id = match self.find_node_at_position(&graph, &path, position)? {
            Some(id) => id,
            None => return Ok(None),
        };

        let node = graph
            .get_node(node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

        // Only functions can have call hierarchies
        if node.node_type != NodeType::Function {
            return Ok(None);
        }

        let name = node.properties.get_string("name").unwrap_or("").to_string();
        let location = self.node_to_location(&graph, node_id)?;

        Ok(Some(vec![CallHierarchyItem {
            name,
            kind: SymbolKind::FUNCTION,
            tags: None,
            detail: node.properties.get_string("signature").map(|s| s.to_string()),
            uri: location.uri,
            range: location.range,
            selection_range: location.range,
            data: Some(serde_json::json!({ "nodeId": node_id.to_string() })),
        }]))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let node_id = self.extract_node_id_from_item(&params.item)?;

        let graph = self.graph.read().await;

        let edges = self.get_connected_edges(&graph, node_id, Direction::Incoming);

        let mut calls = Vec::new();

        for (source, _, edge_type) in edges {
            if edge_type == EdgeType::Calls {
                if let Ok(node) = graph.get_node(source) {
                    let name = node.properties.get_string("name").unwrap_or("").to_string();

                    if let Ok(location) = self.node_to_location(&graph, source) {
                        calls.push(CallHierarchyIncomingCall {
                            from: CallHierarchyItem {
                                name,
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                detail: node.properties.get_string("signature").map(|s| s.to_string()),
                                uri: location.uri.clone(),
                                range: location.range,
                                selection_range: location.range,
                                data: Some(serde_json::json!({ "nodeId": source.to_string() })),
                            },
                            from_ranges: vec![location.range],
                        });
                    }
                }
            }
        }

        if calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(calls))
        }
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let node_id = self.extract_node_id_from_item(&params.item)?;

        let graph = self.graph.read().await;

        let edges = self.get_connected_edges(&graph, node_id, Direction::Outgoing);

        let mut calls = Vec::new();

        for (_, target, edge_type) in edges {
            if edge_type == EdgeType::Calls {
                if let Ok(node) = graph.get_node(target) {
                    let name = node.properties.get_string("name").unwrap_or("").to_string();

                    if let Ok(location) = self.node_to_location(&graph, target) {
                        calls.push(CallHierarchyOutgoingCall {
                            to: CallHierarchyItem {
                                name,
                                kind: SymbolKind::FUNCTION,
                                tags: None,
                                detail: node.properties.get_string("signature").map(|s| s.to_string()),
                                uri: location.uri.clone(),
                                range: location.range,
                                selection_range: location.range,
                                data: Some(serde_json::json!({ "nodeId": target.to_string() })),
                            },
                            from_ranges: vec![location.range],
                        });
                    }
                }
            }
        }

        if calls.is_empty() {
            Ok(None)
        } else {
            Ok(Some(calls))
        }
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
        tracing::info!("Executing command: {}", params.command);

        match params.command.as_str() {
            "codegraph.getDependencyGraph" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::DependencyGraphParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_dependency_graph(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getCallGraph" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::CallGraphParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_call_graph(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.analyzeImpact" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::ImpactAnalysisParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_analyze_impact(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getParserMetrics" => {
                let response = self.handle_get_parser_metrics(crate::handlers::ParserMetricsParams {
                    language: None,
                }).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.reindexWorkspace" => {
                // Clear and reindex
                {
                    let mut graph = self.graph.write().await;
                    *graph = CodeGraph::in_memory().expect("Failed to create graph");
                }
                self.symbol_index.clear();
                self.file_cache.clear();

                self.client
                    .log_message(MessageType::INFO, "Workspace reindexed")
                    .await;

                Ok(None)
            }

            "codegraph.getAIContext" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::AIContextParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_ai_context(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getNodeLocation" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::GetNodeLocationParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_node_location(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getWorkspaceSymbols" => {
                let args = params.arguments.get(0).ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::WorkspaceSymbolsParams = serde_json::from_value(args.clone())
                    .map_err(|e| tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {}", e)))?;
                let response = self.handle_get_workspace_symbols(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            _ => Err(tower_lsp::jsonrpc::Error::method_not_found()),
        }
    }
}

impl CodeGraphBackend {
    /// Extract node ID from CallHierarchyItem data.
    fn extract_node_id_from_item(&self, item: &CallHierarchyItem) -> Result<NodeId> {
        let data = item
            .data
            .as_ref()
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing data"))?;

        let node_id_str = data
            .get("nodeId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Missing nodeId"))?;

        node_id_str
            .parse::<NodeId>()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid nodeId"))
    }
}
