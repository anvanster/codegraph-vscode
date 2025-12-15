//! LSP Backend Implementation
//!
//! This module implements the Language Server Protocol for CodeGraph.

use crate::cache::QueryCache;
use crate::error::{LspError, LspResult};
use crate::index::SymbolIndex;
use crate::parser_registry::ParserRegistry;
use crate::watcher::FileWatcher;
use codegraph::{CodeGraph, Direction, EdgeType, NodeId, NodeType};
use codegraph_parser_api::FileInfo;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
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

    /// Workspace folders
    workspace_folders: Arc<RwLock<Vec<std::path::PathBuf>>>,

    /// File system watcher for incremental updates.
    file_watcher: Arc<Mutex<Option<FileWatcher>>>,
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
            workspace_folders: Arc::new(RwLock::new(Vec::new())),
            file_watcher: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the file watcher for the given workspace folders.
    pub async fn start_file_watcher(&self, folders: &[PathBuf]) {
        // Create the file watcher
        match FileWatcher::new(
            Arc::clone(&self.graph),
            Arc::clone(&self.parsers),
            self.client.clone(),
        ) {
            Ok(mut watcher) => {
                // Start watching each folder
                for folder in folders {
                    if let Err(e) = watcher.watch(folder) {
                        self.client
                            .log_message(
                                MessageType::WARNING,
                                format!("Failed to watch {}: {}", folder.display(), e),
                            )
                            .await;
                    } else {
                        self.client
                            .log_message(
                                MessageType::INFO,
                                format!("Watching folder: {}", folder.display()),
                            )
                            .await;
                    }
                }

                // Store the watcher
                *self.file_watcher.lock().await = Some(watcher);
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Failed to create file watcher: {e}"),
                    )
                    .await;
            }
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

    /// Index all supported files in a directory
    fn index_directory<'a>(
        &'a self,
        dir: &'a std::path::Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + Send + 'a>> {
        Box::pin(async move {
            use std::fs;

            let mut indexed_count = 0;
            let supported_extensions = self.parsers.supported_extensions();

            tracing::info!("Indexing directory: {:?}", dir);

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    // Skip hidden files and directories
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().starts_with('.') {
                            continue;
                        }
                    }

                    if path.is_dir() {
                        // Skip common directories
                        let dir_name = path.file_name().unwrap().to_string_lossy();
                        if matches!(
                            dir_name.as_ref(),
                            "node_modules" | "target" | "dist" | "build" | ".git" | "__pycache__"
                        ) {
                            continue;
                        }

                        // Recursively index subdirectories
                        indexed_count += self.index_directory(&path).await;
                    } else if path.is_file() {
                        // Check if file has supported extension
                        if let Some(ext) = path.extension() {
                            let ext_str = ext.to_string_lossy();
                            if supported_extensions
                                .iter()
                                .any(|&e| e.trim_start_matches('.') == ext_str)
                            {
                                // Parse the file using parse_file (which updates metrics)
                                if let Some(parser) = self.parsers.parser_for_path(&path) {
                                    let mut graph = self.graph.write().await;
                                    match parser.parse_file(&path, &mut graph) {
                                        Ok(file_info) => {
                                            self.symbol_index.add_file(
                                                path.clone(),
                                                &file_info,
                                                &graph,
                                            );
                                            self.file_cache.insert(
                                                Url::from_file_path(&path).unwrap(),
                                                file_info,
                                            );
                                            indexed_count += 1;
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to parse {:?}: {}", path, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            indexed_count
        })
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

        tracing::info!(
            "find_node_at_position: path={:?}, LSP position={}:{}, converted={}:{}",
            path,
            position.line,
            position.character,
            line,
            col
        );

        // Try the symbol index first (faster)
        if let Some(node_id) = self
            .symbol_index
            .find_at_position(path, line as u32, col as u32)
        {
            tracing::info!("Found node in symbol index: {:?}", node_id);
            return Ok(Some(node_id));
        }

        tracing::info!("Not found in symbol index, trying graph query");

        // Fallback: Get all nodes from the symbol index for this file and check positions
        // The graph stores line_start/line_end (not start_line/end_line)
        let file_symbols = self.symbol_index.get_file_symbols(path);

        tracing::info!(
            "Symbol index returned {} symbols for {:?}",
            file_symbols.len(),
            path
        );

        for node_id in file_symbols {
            if let Ok(node) = graph.get_node(node_id) {
                // Parsers use line_start/line_end (not start_line/end_line)
                let start_line = node.properties.get_int("line_start").unwrap_or(0);
                let end_line = node.properties.get_int("line_end").unwrap_or(0);
                // Column info not available from most parsers, default to full line
                let start_col = node.properties.get_int("col_start").unwrap_or(0);
                let end_col = node.properties.get_int("col_end").unwrap_or(i64::MAX);

                tracing::info!(
                    "Checking node {:?} '{}' at {}:{} to {}:{}",
                    node_id,
                    node.properties.get_string("name").unwrap_or(""),
                    start_line,
                    start_col,
                    end_line,
                    end_col
                );

                if line >= start_line && line <= end_line {
                    if line == start_line && col < start_col {
                        continue;
                    }
                    if line == end_line && col > end_col {
                        continue;
                    }
                    tracing::info!("Found matching node: {:?}", node_id);
                    return Ok(Some(node_id));
                }
            }
        }

        tracing::warn!("No node found at position {}:{} in {:?}", line, col, path);
        Ok(None)
    }

    /// Find the nearest symbol to a position, or the first symbol in the file.
    /// This is used as a fallback when no symbol is found at the exact cursor position.
    /// Returns (node_id, was_fallback) where was_fallback indicates if this was not an exact match.
    pub fn find_nearest_node(
        &self,
        graph: &CodeGraph,
        path: &std::path::Path,
        position: Position,
    ) -> LspResult<Option<(NodeId, bool)>> {
        // First try exact position match
        if let Some(node_id) = self.find_node_at_position(graph, path, position)? {
            return Ok(Some((node_id, false)));
        }

        // LSP positions are 0-indexed, our index stores 1-indexed
        let target_line = (position.line + 1) as i64;

        // Get all symbols in the file
        let file_symbols = self.symbol_index.get_file_symbols(path);

        if file_symbols.is_empty() {
            tracing::info!("No symbols in file {:?}", path);
            return Ok(None);
        }

        // Find the nearest symbol by line distance
        let mut best_match: Option<(NodeId, i64)> = None;

        for node_id in file_symbols {
            if let Ok(node) = graph.get_node(node_id) {
                let start_line = node.properties.get_int("line_start").unwrap_or(0);
                let end_line = node.properties.get_int("line_end").unwrap_or(0);

                // Calculate distance - prefer symbols that start after cursor (looking ahead)
                // or symbols that contain the cursor line
                let distance = if target_line >= start_line && target_line <= end_line {
                    // Cursor is within this symbol's range
                    0
                } else if start_line > target_line {
                    // Symbol starts after cursor - prefer these (looking forward)
                    start_line - target_line
                } else {
                    // Symbol ends before cursor - less preferred
                    (target_line - end_line) + 1000 // Add penalty for looking backward
                };

                if best_match.is_none() || distance < best_match.unwrap().1 {
                    best_match = Some((node_id, distance));
                }
            }
        }

        if let Some((node_id, _)) = best_match {
            if let Ok(node) = graph.get_node(node_id) {
                let name = node.properties.get_string("name").unwrap_or("unknown");
                tracing::info!(
                    "Fallback: found nearest symbol '{}' for position {}:{} in {:?}",
                    name,
                    target_line,
                    position.character,
                    path
                );
            }
            return Ok(Some((node_id, true)));
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

        // Try to get path from node properties first, fallback to symbol index
        let path_str = match node.properties.get_string("path") {
            Some(p) => p.to_string(),
            None => {
                // Fallback: look up file path from symbol index
                match self.symbol_index.find_file_for_node(node_id) {
                    Some(path_buf) => path_buf.to_string_lossy().to_string(),
                    None => {
                        let node_name = node.properties.get_string("name").unwrap_or("<unnamed>");
                        let node_type = format!("{}", node.node_type);
                        tracing::warn!(
                            "Cannot determine file path for node {}: {} '{}' (not in symbol index)",
                            node_id,
                            node_type,
                            node_name
                        );
                        return Err(LspError::NodeNotFound(format!(
                            "Cannot determine file path for {node_type} '{node_name}'"
                        )));
                    }
                }
            }
        };

        // Support both property name conventions (line_start or start_line)
        let start_line = node
            .properties
            .get_int("line_start")
            .or_else(|| node.properties.get_int("start_line"))
            .unwrap_or(1) as u32;
        let start_col = node
            .properties
            .get_int("col_start")
            .or_else(|| node.properties.get_int("start_col"))
            .unwrap_or(0) as u32;
        let end_line = node
            .properties
            .get_int("line_end")
            .or_else(|| node.properties.get_int("end_line"))
            .unwrap_or(start_line as i64) as u32;
        let end_col = node
            .properties
            .get_int("col_end")
            .or_else(|| node.properties.get_int("end_col"))
            .unwrap_or(0) as u32;

        // Convert to 0-indexed
        let start_line = start_line.saturating_sub(1);
        let end_line = end_line.saturating_sub(1);

        Ok(Location {
            uri: Url::from_file_path(&path_str)
                .map_err(|_| LspError::InvalidUri(path_str.clone()))?,
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

        // Try to get path from node properties, fallback to symbol index
        let path = match node.properties.get_string("path") {
            Some(p) => PathBuf::from(p),
            None => {
                // Fallback: look up file path from symbol index
                match self.symbol_index.find_file_for_node(node_id) {
                    Some(p) => p,
                    None => return Ok(None),
                }
            }
        };

        // Try to read from file
        if path.exists() {
            // Support both property name conventions
            let start_line = node
                .properties
                .get_int("line_start")
                .or_else(|| node.properties.get_int("start_line"))
                .unwrap_or(1) as usize;
            let end_line = node
                .properties
                .get_int("line_end")
                .or_else(|| node.properties.get_int("end_line"))
                .unwrap_or(start_line as i64) as usize;

            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let lines: Vec<&str> = content.lines().collect();
                if start_line > 0 && end_line <= lines.len() {
                    let source: String = lines[start_line - 1..end_line].join("\n");
                    return Ok(Some(source));
                }
            }
        }

        Ok(None)
    }

    /// Helper to get a string property from a node
    #[allow(dead_code)]
    fn get_node_string_property(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        key: &str,
    ) -> Option<String> {
        graph
            .get_node(node_id)
            .ok()?
            .properties
            .get_string(key)
            .map(|s| s.to_string())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for CodeGraphBackend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("Initializing CodeGraph LSP server");

        // Store workspace folders
        if let Some(folders) = params.workspace_folders {
            let mut workspace_folders = self.workspace_folders.write().await;
            for folder in folders {
                if let Ok(path) = folder.uri.to_file_path() {
                    tracing::info!("Workspace folder: {}", path.display());
                    workspace_folders.push(path);
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
                        "codegraph.findRelatedTests".to_string(),
                        "codegraph.getNodeLocation".to_string(),
                        "codegraph.getWorkspaceSymbols".to_string(),
                        "codegraph.analyzeComplexity".to_string(),
                        "codegraph.findUnusedCode".to_string(),
                        "codegraph.analyzeCoupling".to_string(),
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

        // Index workspace folders
        let folders = self.workspace_folders.read().await.clone();
        let mut total_indexed = 0;

        for folder in &folders {
            let count = self.index_directory(folder).await;
            total_indexed += count;
            self.client
                .log_message(
                    MessageType::INFO,
                    format!("Indexed {} files from {}", count, folder.display()),
                )
                .await;
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("Total files indexed: {total_indexed}"),
            )
            .await;

        // Start file watcher for incremental updates
        if !folders.is_empty() {
            self.start_file_watcher(&folders).await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down CodeGraph LSP server");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        tracing::info!("did_open called for: {}", uri);

        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("Invalid URI: {}", uri);
                return;
            }
        };

        if let Some(parser) = self.parsers.parser_for_path(&path) {
            tracing::info!("Parser found for: {:?}", path);
            let mut graph = self.graph.write().await;

            match parser.parse_source(&text, &path, &mut graph) {
                Ok(file_info) => {
                    tracing::info!("Parse succeeded for: {:?}", path);
                    // Update symbol index
                    self.symbol_index.add_file(path.clone(), &file_info, &graph);

                    // Update file cache
                    self.file_cache.insert(uri.clone(), file_info);

                    self.client
                        .log_message(MessageType::INFO, format!("Indexed: {uri}"))
                        .await;
                }
                Err(e) => {
                    tracing::error!("Parse failed for {:?}: {}", path, e);
                    self.client
                        .log_message(MessageType::ERROR, format!("Parse error in {uri}: {e}"))
                        .await;
                }
            }
        } else {
            tracing::warn!("No parser found for: {:?}", path);
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
            tracing::info!(
                "goto_definition: found reference node {} -> definition node {}",
                node_id,
                def_node_id
            );
            match self.node_to_location(&graph, def_node_id) {
                Ok(location) => return Ok(Some(GotoDefinitionResponse::Scalar(location))),
                Err(e) => {
                    // Log the error but try to return the source location as fallback
                    tracing::warn!(
                        "Failed to get definition location: {}, trying source node",
                        e
                    );
                    // Fall through to try source node
                }
            }
        }

        // Already at definition (or fallback if definition lookup failed)
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
        let signature = node
            .properties
            .get_string("signature")
            .unwrap_or("")
            .to_string();
        let doc = node.properties.get_string("doc").map(|s| s.to_string());
        let def_path = node.properties.get_string("path").unwrap_or("").to_string();

        // Count references
        let ref_count = self
            .get_connected_edges(&graph, node_id, Direction::Incoming)
            .len();

        let mut content = format!("**{kind}** `{name}`");

        if !signature.is_empty() {
            content.push_str(&format!("\n\n```\n{signature}\n```"));
        }

        if let Some(doc) = doc {
            content.push_str(&format!("\n\n{doc}"));
        }

        content.push_str(&format!(
            "\n\n---\n\n**Defined in:** {def_path}\n**References:** {ref_count}"
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
            detail: node
                .properties
                .get_string("signature")
                .map(|s| s.to_string()),
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
                                detail: node
                                    .properties
                                    .get_string("signature")
                                    .map(|s| s.to_string()),
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
                                detail: node
                                    .properties
                                    .get_string("signature")
                                    .map(|s| s.to_string()),
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

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        tracing::info!("Executing command: {}", params.command);

        match params.command.as_str() {
            "codegraph.getDependencyGraph" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::DependencyGraphParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_get_dependency_graph(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getCallGraph" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::CallGraphParams = serde_json::from_value(args.clone())
                    .map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_get_call_graph(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.analyzeImpact" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::ImpactAnalysisParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_analyze_impact(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getParserMetrics" => {
                let response = self
                    .handle_get_parser_metrics(crate::handlers::ParserMetricsParams {
                        language: None,
                    })
                    .await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.reindexWorkspace" => {
                // Clear graph and caches
                {
                    let mut graph = self.graph.write().await;
                    *graph = CodeGraph::in_memory().expect("Failed to create graph");
                }
                self.symbol_index.clear();
                self.file_cache.clear();

                self.client
                    .log_message(MessageType::INFO, "Reindexing workspace...")
                    .await;

                // Index all workspace folders
                let workspace_folders = self.workspace_folders.read().await.clone();
                let mut total_indexed = 0;

                for folder in workspace_folders {
                    tracing::info!("Indexing folder: {:?}", folder);
                    let count = self.index_directory(&folder).await;
                    total_indexed += count;
                }

                self.client
                    .log_message(
                        MessageType::INFO,
                        format!("Workspace reindexed: {total_indexed} files"),
                    )
                    .await;

                Ok(None)
            }

            "codegraph.getAIContext" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::AIContextParams = serde_json::from_value(args.clone())
                    .map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_get_ai_context(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.findRelatedTests" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::RelatedTestsParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_find_related_tests(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getNodeLocation" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::GetNodeLocationParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_get_node_location(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.getWorkspaceSymbols" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::WorkspaceSymbolsParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_get_workspace_symbols(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.analyzeComplexity" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::ComplexityParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_analyze_complexity(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.findUnusedCode" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::UnusedCodeParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_find_unused_code(params).await?;
                Ok(Some(serde_json::to_value(response).unwrap()))
            }

            "codegraph.analyzeCoupling" => {
                let args = params.arguments.first().ok_or_else(|| {
                    tower_lsp::jsonrpc::Error::invalid_params("Missing arguments")
                })?;
                let params: crate::handlers::CouplingParams =
                    serde_json::from_value(args.clone()).map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("Invalid params: {e}"))
                    })?;
                let response = self.handle_analyze_coupling(params).await?;
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
