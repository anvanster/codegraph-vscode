# CodeGraph VS Code Extension - Design Document

## Executive Summary

The CodeGraph VS Code Extension provides cross-language code intelligence by leveraging the codegraph crate and its unified parser system through a Language Server Protocol (LSP) implementation. The extension enables developers and AI assistants to understand, navigate, and analyze codebases with multi-language support.

**Key Features:**
- Cross-language code navigation and analysis
- Language Server Protocol (LSP) integration
- AI context provider for code assistants
- Interactive graph visualizations
- Real-time incremental updates

---

## 1. Goals and Requirements

### 1.1 Primary Goals

1. **Unified Cross-Language Intelligence**: Single tool for understanding codebases with multiple programming languages
2. **AI-First Design**: Provide rich context to AI coding assistants (GitHub Copilot, Claude, etc.)
3. **Developer Productivity**: Fast, accurate code navigation and analysis
4. **Extensibility**: Plugin architecture for adding new languages and analysis features

### 1.2 Non-Goals

- Not a replacement for language-specific LSP servers (Rust Analyzer, Pylance, etc.)
- Not providing syntax highlighting or basic completions
- Not a general-purpose IDE (focus on graph-based intelligence)

### 1.3 Requirements

**Functional:**
- Support for Python, Rust, TypeScript/JavaScript, Go initially
- Cross-language symbol references
- Dependency graph generation
- Call hierarchy across language boundaries
- AI context retrieval API

**Non-Functional:**
- Index a 100k LOC codebase in <10 seconds
- Respond to navigation requests in <100ms
- Handle incremental updates in <1 second
- Memory footprint <500MB for typical projects

---

## 2. System Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    VS Code Extension                         │
│  ┌────────────┐  ┌──────────┐  ┌──────────────────┐        │
│  │ LSP Client │  │ Commands │  │  Webview Panel   │        │
│  │            │  │ & Actions│  │ (Visualizations) │        │
│  └─────┬──────┘  └────┬─────┘  └────────┬─────────┘        │
│        │              │                 │                   │
└────────┼──────────────┼─────────────────┼───────────────────┘
         │              │                 │
         │ LSP Protocol │                 │ Custom RPC
         ▼              ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│              CodeGraph LSP Server (Rust)                     │
│  ┌────────────────────────────────────────────────────┐     │
│  │            Core LSP Handler                        │     │
│  │  - textDocument/definition                         │     │
│  │  - textDocument/references                         │     │
│  │  - textDocument/hover                              │     │
│  │  - textDocument/documentSymbol                     │     │
│  │  - callHierarchy/*                                 │     │
│  └────────────────┬───────────────────────────────────┘     │
│                   │                                          │
│  ┌────────────────▼───────────────────────────────────┐     │
│  │       Custom Extensions                            │     │
│  │  - codegraph/getDependencyGraph                    │     │
│  │  - codegraph/getCallGraph                          │     │
│  │  - codegraph/getAIContext                          │     │
│  │  - codegraph/analyzeImpact                         │     │
│  │  - codegraph/analyzeCoupling                       │     │
│  │  - codegraph/findSimilarCode                       │     │
│  │  - codegraph/getParserMetrics                      │     │
│  └────────────────┬───────────────────────────────────┘     │
│                   │                                          │
│  ┌────────────────▼───────────────────────────────────┐     │
│  │         Parser Registry                            │     │
│  │  ┌──────────┐ ┌──────────┐ ┌────────────┐         │     │
│  │  │ Python   │ │  Rust    │ │ TypeScript │ ...     │     │
│  │  │ Parser   │ │  Parser  │ │  Parser    │         │     │
│  │  └──────────┘ └──────────┘ └────────────┘         │     │
│  └────────────────┬───────────────────────────────────┘     │
│                   │                                          │
│  ┌────────────────▼───────────────────────────────────┐     │
│  │         CodeGraph Core Engine                      │     │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐         │     │
│  │  │  Graph   │  │  Query   │  │  Index   │         │     │
│  │  │  Store   │  │  Engine  │  │  Manager │         │     │
│  │  └──────────┘  └──────────┘  └──────────┘         │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Component Breakdown

#### 2.2.1 VS Code Extension (TypeScript)

**Extension Host (`extension.ts`)**
```typescript
import * as vscode from 'vscode';
import * as path from 'path';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

export async function activate(context: vscode.ExtensionContext) {
    // Determine server binary path
    const serverModule = context.asAbsolutePath(
        path.join('server', 'codegraph-lsp')
    );
    
    // Server options
    const serverOptions: ServerOptions = {
        command: serverModule,
        args: ['--stdio'],
    };
    
    // Client options
    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'python' },
            { scheme: 'file', language: 'rust' },
            { scheme: 'file', language: 'typescript' },
            { scheme: 'file', language: 'javascript' },
            { scheme: 'file', language: 'go' },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*'),
        },
    };
    
    const client = new LanguageClient(
        'codegraph',
        'CodeGraph Language Server',
        serverOptions,
        clientOptions
    );
    
    await client.start();
    
    // Register providers and commands
    registerCommands(context, client);
    registerTreeDataProviders(context, client);
    registerWebviewPanels(context, client);
    registerAIContextProvider(context, client);
}
```

**Key Components:**

1. **LSP Client Manager**
   - Manages lifecycle of LSP server process
   - Handles standard LSP requests/responses
   - Routes custom requests to server

2. **Command Handlers**
   - `codegraph.showDependencyGraph`
   - `codegraph.showCallGraph`
   - `codegraph.analyzeImpact`
   - `codegraph.explainCode`
   - `codegraph.findSimilarPatterns`
   - `codegraph.showMetrics`

3. **Tree View Providers**
   - Dependency tree view
   - Call hierarchy view
   - Symbol index view
   - Architecture overview

4. **Webview Panels**
   - Interactive graph visualizations (using D3.js or Cytoscape.js)
   - Dependency matrix views
   - Metrics dashboards

5. **AI Context Provider**
   - Provides code context to AI assistants
   - Integrates with GitHub Copilot Chat, Claude, etc.

#### 2.2.2 CodeGraph LSP Server (Rust)

**Dependencies (`Cargo.toml`)**
```toml
[package]
name = "codegraph-lsp"
version = "0.1.0"
edition = "2021"

[dependencies]
# LSP framework
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }

# CodeGraph ecosystem
codegraph = { workspace = true }
codegraph-parser-api = { workspace = true }
codegraph-python = { workspace = true }
codegraph-rust = { workspace = true }
codegraph-typescript = { workspace = true }
codegraph-go = { workspace = true }

# Utilities
serde = { version = "1", features = ["derive"] }
serde_json = "1"
notify = "6"
tracing = "0.1"
thiserror = "1"
dashmap = "5"
```

**Server Entry Point (`main.rs`)**
```rust
use tower_lsp::{LspService, Server};
use codegraph_lsp::CodeGraphBackend;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    
    let (service, socket) = LspService::new(|client| {
        CodeGraphBackend::new(client)
    });
    
    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
```

**Key Components:**

1. **LSP Handler (`backend.rs`)**
   - Implements `tower_lsp::LanguageServer` trait
   - Handles standard LSP requests
   - Delegates to CodeGraph engine

2. **Parser Registry (`parser_registry.rs`)**
   - Manages all language parsers
   - Routes files to appropriate parser
   - Provides unified parsing interface

3. **Custom Request Handlers**
   - Extended LSP protocol for graph operations
   - AI context generation
   - Advanced analysis features

4. **File System Watcher**
   - Watches workspace for changes
   - Triggers incremental re-parsing
   - Notifies clients of updates

5. **Graph Engine Adapter**
   - Bridges LSP types to CodeGraph types
   - Manages graph lifecycle
   - Caches query results

#### 2.2.3 Parser Registry

The Parser Registry is a critical component that manages all language parsers implementing the `CodeParser` trait.

```rust
// src/parser_registry.rs

use codegraph::CodeGraph;
use codegraph_parser_api::{CodeParser, FileInfo, ParserConfig, ParserError, ParserMetrics};
use codegraph_python::PythonParser;
use codegraph_rust::RustParser;
use codegraph_typescript::TypeScriptParser;
use codegraph_go::GoParser;
use std::path::Path;
use std::sync::Arc;

/// Registry of all available language parsers
pub struct ParserRegistry {
    python: Arc<PythonParser>,
    rust: Arc<RustParser>,
    typescript: Arc<TypeScriptParser>,
    go: Arc<GoParser>,
}

impl ParserRegistry {
    /// Create a new parser registry with default configuration
    pub fn new() -> Self {
        Self::with_config(ParserConfig::default())
    }
    
    /// Create a new parser registry with custom configuration
    pub fn with_config(config: ParserConfig) -> Self {
        Self {
            python: Arc::new(PythonParser::with_config(config.clone())),
            rust: Arc::new(RustParser::with_config(config.clone())),
            typescript: Arc::new(TypeScriptParser::with_config(config.clone())),
            go: Arc::new(GoParser::with_config(config)),
        }
    }
    
    /// Get parser by language identifier
    pub fn get_parser(&self, language: &str) -> Option<Arc<dyn CodeParser>> {
        match language {
            "python" => Some(self.python.clone()),
            "rust" => Some(self.rust.clone()),
            "typescript" | "javascript" | "typescriptreact" | "javascriptreact" => {
                Some(self.typescript.clone())
            }
            "go" => Some(self.go.clone()),
            _ => None,
        }
    }
    
    /// Find appropriate parser for a file path
    pub fn parser_for_path(&self, path: &Path) -> Option<Arc<dyn CodeParser>> {
        let parsers: [Arc<dyn CodeParser>; 4] = [
            self.python.clone(),
            self.rust.clone(),
            self.typescript.clone(),
            self.go.clone(),
        ];
        
        parsers.into_iter().find(|p| p.can_parse(path))
    }
    
    /// Get all supported file extensions
    pub fn supported_extensions(&self) -> Vec<&'static str> {
        let mut extensions = Vec::new();
        extensions.extend(self.python.file_extensions());
        extensions.extend(self.rust.file_extensions());
        extensions.extend(self.typescript.file_extensions());
        extensions.extend(self.go.file_extensions());
        extensions
    }
    
    /// Get metrics from all parsers
    pub fn all_metrics(&self) -> Vec<(&str, ParserMetrics)> {
        vec![
            ("python", self.python.metrics()),
            ("rust", self.rust.metrics()),
            ("typescript", self.typescript.metrics()),
            ("go", self.go.metrics()),
        ]
    }
    
    /// Reset metrics for all parsers
    pub fn reset_all_metrics(&self) {
        // Note: This requires mutable access, which may need interior mutability
        // in a real implementation
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 3. Language Server Protocol Integration

### 3.1 Standard LSP Features

#### 3.1.1 Text Synchronization
```rust
use tower_lsp::lsp_types::*;
use tower_lsp::jsonrpc::Result;
use codegraph::CodeGraph;
use codegraph_parser_api::CodeParser;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct CodeGraphBackend {
    client: tower_lsp::Client,
    graph: Arc<RwLock<CodeGraph>>,
    parsers: Arc<ParserRegistry>,
    // Cache: file URI -> FileInfo
    file_cache: Arc<DashMap<Url, FileInfo>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for CodeGraphBackend {
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let path = uri.to_file_path().unwrap_or_default();
        
        if let Some(parser) = self.parsers.parser_for_path(&path) {
            let mut graph = self.graph.write().await;
            
            match parser.parse_source(&text, &path, &mut graph) {
                Ok(file_info) => {
                    self.file_cache.insert(uri.clone(), file_info);
                    self.client.log_message(
                        MessageType::INFO,
                        format!("Indexed: {}", uri)
                    ).await;
                }
                Err(e) => {
                    self.client.log_message(
                        MessageType::ERROR,
                        format!("Parse error in {}: {}", uri, e)
                    ).await;
                }
            }
        }
    }
    
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let path = uri.to_file_path().unwrap_or_default();
        
        // Get the full text (assuming full sync mode)
        if let Some(change) = params.content_changes.into_iter().next() {
            if let Some(parser) = self.parsers.parser_for_path(&path) {
                let mut graph = self.graph.write().await;
                
                // Remove old entries for this file
                self.remove_file_from_graph(&mut graph, &path).await;
                
                // Re-parse with new content
                if let Ok(file_info) = parser.parse_source(&change.text, &path, &mut graph) {
                    self.file_cache.insert(uri, file_info);
                }
            }
        }
    }
    
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Optionally trigger full re-parse on save
        let uri = params.text_document.uri;
        let path = uri.to_file_path().unwrap_or_default();
        
        if let Some(parser) = self.parsers.parser_for_path(&path) {
            if let Some(text) = params.text {
                let mut graph = self.graph.write().await;
                self.remove_file_from_graph(&mut graph, &path).await;
                
                if let Ok(file_info) = parser.parse_source(&text, &path, &mut graph) {
                    self.file_cache.insert(uri, file_info);
                }
            }
        }
    }
    
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Optionally remove from cache (keep in graph for cross-file references)
        self.file_cache.remove(&params.text_document.uri);
    }
}

impl CodeGraphBackend {
    /// Remove all nodes associated with a file from the graph
    async fn remove_file_from_graph(&self, graph: &mut CodeGraph, path: &Path) {
        let path_str = path.to_string_lossy();
        
        // Query for all nodes with this file path
        if let Ok(nodes) = graph.query_nodes_by_property("path", &path_str) {
            for node_id in nodes {
                // Remove edges first, then node
                let _ = graph.remove_node_with_edges(node_id);
            }
        }
    }
}
```

#### 3.1.2 Go to Definition
```rust
async fn goto_definition(
    &self,
    params: GotoDefinitionParams
) -> Result<Option<GotoDefinitionResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let path = uri.to_file_path().map_err(|_| {
        tower_lsp::jsonrpc::Error::invalid_params("Invalid URI")
    })?;
    
    let graph = self.graph.read().await;
    
    // Find node at the given position
    let node_id = self.find_node_at_position(&graph, &path, position)?;
    
    if let Some(node_id) = node_id {
        // Check if this is a reference - find its definition
        if let Some(def_node_id) = self.find_definition_for_reference(&graph, node_id)? {
            let location = self.node_to_location(&graph, def_node_id)?;
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }
        
        // Already at definition
        let location = self.node_to_location(&graph, node_id)?;
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    } else {
        Ok(None)
    }
}

impl CodeGraphBackend {
    /// Find a node at the given position in a file
    fn find_node_at_position(
        &self,
        graph: &CodeGraph,
        path: &Path,
        position: Position,
    ) -> Result<Option<NodeId>> {
        let path_str = path.to_string_lossy();
        let line = position.line as usize + 1; // LSP is 0-indexed
        let col = position.character as usize;
        
        // Query nodes in this file
        let nodes = graph.query_nodes_by_property("path", &path_str)
            .map_err(|e| tower_lsp::jsonrpc::Error::internal_error())?;
        
        // Find node whose range contains the position
        for node_id in nodes {
            if let Ok(node) = graph.get_node(node_id) {
                let start_line: usize = node.properties
                    .get("start_line")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let end_line: usize = node.properties
                    .get("end_line")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let start_col: usize = node.properties
                    .get("start_col")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let end_col: usize = node.properties
                    .get("end_col")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(usize::MAX);
                
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
    
    /// Find the definition node for a reference
    fn find_definition_for_reference(
        &self,
        graph: &CodeGraph,
        ref_node_id: NodeId,
    ) -> Result<Option<NodeId>> {
        use codegraph::EdgeType;
        
        // Look for outgoing "References" or "Calls" edges
        let edges = graph.get_outgoing_edges(ref_node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
        
        for edge in edges {
            match edge.edge_type {
                EdgeType::Calls | EdgeType::References | EdgeType::Imports => {
                    return Ok(Some(edge.target));
                }
                _ => continue,
            }
        }
        
        Ok(None)
    }
    
    /// Convert a node to an LSP Location
    fn node_to_location(&self, graph: &CodeGraph, node_id: NodeId) -> Result<Location> {
        let node = graph.get_node(node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
        
        let path = node.properties.get("path")
            .ok_or_else(|| tower_lsp::jsonrpc::Error::internal_error())?;
        
        let start_line: u32 = node.properties
            .get("start_line")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1) - 1; // Convert to 0-indexed
        
        let start_col: u32 = node.properties
            .get("start_col")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        
        let end_line: u32 = node.properties
            .get("end_line")
            .and_then(|v| v.parse().ok())
            .unwrap_or(start_line + 1) - 1;
        
        let end_col: u32 = node.properties
            .get("end_col")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        
        Ok(Location {
            uri: Url::from_file_path(path).unwrap(),
            range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: end_line, character: end_col },
            },
        })
    }
}
```

#### 3.1.3 Find References
```rust
async fn references(
    &self,
    params: ReferenceParams
) -> Result<Option<Vec<Location>>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_declaration = params.context.include_declaration;
    let path = uri.to_file_path().map_err(|_| {
        tower_lsp::jsonrpc::Error::invalid_params("Invalid URI")
    })?;
    
    let graph = self.graph.read().await;
    
    // Find node at position
    let node_id = match self.find_node_at_position(&graph, &path, position)? {
        Some(id) => id,
        None => return Ok(None),
    };
    
    // Find the definition (might be the node itself or a referenced node)
    let def_node_id = self.find_definition_for_reference(&graph, node_id)?
        .unwrap_or(node_id);
    
    let mut locations = Vec::new();
    
    // Include declaration if requested
    if include_declaration {
        if let Ok(loc) = self.node_to_location(&graph, def_node_id) {
            locations.push(loc);
        }
    }
    
    // Find all incoming edges (references to this definition)
    let edges = graph.get_incoming_edges(def_node_id)
        .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
    
    for edge in edges {
        if let Ok(loc) = self.node_to_location(&graph, edge.source) {
            locations.push(loc);
        }
    }
    
    if locations.is_empty() {
        Ok(None)
    } else {
        Ok(Some(locations))
    }
}
```

#### 3.1.4 Hover Information
```rust
async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let path = uri.to_file_path().map_err(|_| {
        tower_lsp::jsonrpc::Error::invalid_params("Invalid URI")
    })?;
    
    let graph = self.graph.read().await;
    
    let node_id = match self.find_node_at_position(&graph, &path, position)? {
        Some(id) => id,
        None => return Ok(None),
    };
    
    let node = graph.get_node(node_id)
        .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
    
    // Build hover content
    let name = node.properties.get("name").cloned().unwrap_or_default();
    let kind = format!("{:?}", node.node_type);
    let signature = node.properties.get("signature").cloned().unwrap_or_default();
    let doc = node.properties.get("doc").cloned();
    let def_path = node.properties.get("path").cloned().unwrap_or_default();
    
    // Count references
    let ref_count = graph.get_incoming_edges(node_id)
        .map(|edges| edges.len())
        .unwrap_or(0);
    
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
```

#### 3.1.5 Call Hierarchy
```rust
async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams
) -> Result<Option<Vec<CallHierarchyItem>>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let path = uri.to_file_path().map_err(|_| {
        tower_lsp::jsonrpc::Error::invalid_params("Invalid URI")
    })?;
    
    let graph = self.graph.read().await;
    
    let node_id = match self.find_node_at_position(&graph, &path, position)? {
        Some(id) => id,
        None => return Ok(None),
    };
    
    let node = graph.get_node(node_id)
        .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
    
    // Only functions can have call hierarchies
    if !matches!(node.node_type, NodeType::Function | NodeType::Method) {
        return Ok(None);
    }
    
    Ok(Some(vec![self.node_to_call_hierarchy_item(&graph, node_id)?]))
}

async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams
) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
    let item = params.item;
    let node_id = self.call_hierarchy_item_to_node_id(&item)?;
    
    let graph = self.graph.read().await;
    
    // Find all callers (incoming "Calls" edges)
    let edges = graph.get_incoming_edges(node_id)
        .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
    
    let mut calls = Vec::new();
    
    for edge in edges {
        if edge.edge_type == EdgeType::Calls {
            if let Ok(item) = self.node_to_call_hierarchy_item(&graph, edge.source) {
                // Get call site ranges from edge properties
                let ranges = self.get_call_site_ranges(&edge);
                calls.push(CallHierarchyIncomingCall {
                    from: item,
                    from_ranges: ranges,
                });
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
    params: CallHierarchyOutgoingCallsParams
) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
    let item = params.item;
    let node_id = self.call_hierarchy_item_to_node_id(&item)?;
    
    let graph = self.graph.read().await;
    
    // Find all callees (outgoing "Calls" edges)
    let edges = graph.get_outgoing_edges(node_id)
        .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
    
    let mut calls = Vec::new();
    
    for edge in edges {
        if edge.edge_type == EdgeType::Calls {
            if let Ok(item) = self.node_to_call_hierarchy_item(&graph, edge.target) {
                let ranges = self.get_call_site_ranges(&edge);
                calls.push(CallHierarchyOutgoingCall {
                    to: item,
                    from_ranges: ranges,
                });
            }
        }
    }
    
    if calls.is_empty() {
        Ok(None)
    } else {
        Ok(Some(calls))
    }
}
```

### 3.2 Custom LSP Extensions

We extend the LSP protocol with custom requests for CodeGraph-specific features.

#### 3.2.1 Dependency Graph Request
```typescript
// Extension request
interface DependencyGraphParams {
    uri: string;              // Root file/module
    depth?: number;           // How many levels deep (default: 3)
    includeExternal?: boolean; // Include external dependencies
    direction?: 'imports' | 'importedBy' | 'both';
}

interface DependencyGraphResponse {
    nodes: Array<{
        id: string;
        label: string;
        type: 'module' | 'package' | 'file';
        language: string;
        uri: string;
        metadata?: Record<string, any>;
    }>;
    edges: Array<{
        from: string;
        to: string;
        type: 'import' | 'require' | 'use';
        metadata?: Record<string, any>;
    }>;
}
```

```rust
// Server handler
async fn handle_get_dependency_graph(
    &self,
    params: DependencyGraphParams
) -> Result<DependencyGraphResponse> {
    let path = Url::parse(&params.uri)
        .and_then(|u| u.to_file_path().ok())
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;
    
    let graph = self.graph.read().await;
    let depth = params.depth.unwrap_or(3);
    let include_external = params.include_external.unwrap_or(false);
    
    // Find the file node
    let file_node = self.find_file_node(&graph, &path)?;
    
    // BFS to collect dependency subgraph
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    
    queue.push_back((file_node, 0));
    visited.insert(file_node);
    
    while let Some((node_id, current_depth)) = queue.pop_front() {
        if current_depth > depth {
            continue;
        }
        
        let node = graph.get_node(node_id)?;
        
        // Skip external if not requested
        let is_external = node.properties.get("external")
            .map(|v| v == "true")
            .unwrap_or(false);
        
        if is_external && !include_external {
            continue;
        }
        
        nodes.push(DependencyNode {
            id: node_id.to_string(),
            label: node.properties.get("name").cloned().unwrap_or_default(),
            node_type: format!("{:?}", node.node_type),
            language: node.properties.get("language").cloned().unwrap_or_default(),
            uri: node.properties.get("path").cloned().unwrap_or_default(),
            metadata: None,
        });
        
        // Get import edges based on direction
        let import_edges = match params.direction.as_deref() {
            Some("imports") => graph.get_outgoing_edges(node_id)?,
            Some("importedBy") => graph.get_incoming_edges(node_id)?,
            _ => {
                let mut all = graph.get_outgoing_edges(node_id)?;
                all.extend(graph.get_incoming_edges(node_id)?);
                all
            }
        };
        
        for edge in import_edges {
            if edge.edge_type == EdgeType::Imports {
                edges.push(DependencyEdge {
                    from: edge.source.to_string(),
                    to: edge.target.to_string(),
                    edge_type: "import".to_string(),
                    metadata: None,
                });
                
                let next_node = if edge.source == node_id { edge.target } else { edge.source };
                if !visited.contains(&next_node) {
                    visited.insert(next_node);
                    queue.push_back((next_node, current_depth + 1));
                }
            }
        }
    }
    
    Ok(DependencyGraphResponse { nodes, edges })
}
```

#### 3.2.2 Call Graph Request
```typescript
interface CallGraphParams {
    uri: string;
    position: Position;
    direction?: 'callers' | 'callees' | 'both';
    depth?: number;
    includeExternal?: boolean;
}

interface CallGraphResponse {
    root: FunctionNode;
    nodes: FunctionNode[];
    edges: CallEdge[];
}

interface FunctionNode {
    id: string;
    name: string;
    signature: string;
    uri: string;
    range: Range;
    language: string;
    metrics?: {
        complexity?: number;
        linesOfCode?: number;
        callCount?: number;
    };
}

interface CallEdge {
    from: string;
    to: string;
    callSites: Location[];
    isRecursive?: boolean;
}
```

#### 3.2.3 AI Context Request
```typescript
interface AIContextParams {
    uri: string;
    position: Position;
    contextType: 'explain' | 'modify' | 'debug' | 'test';
    maxTokens?: number;
}

interface AIContextResponse {
    primaryContext: {
        type: 'function' | 'class' | 'module';
        name: string;
        code: string;
        language: string;
        location: Location;
    };
    
    relatedSymbols: Array<{
        name: string;
        relationship: 'calls' | 'called_by' | 'uses' | 'used_by' | 'inherits' | 'implements';
        code: string;
        location: Location;
        relevanceScore: number;
    }>;
    
    dependencies: Array<{
        name: string;
        type: 'import' | 'type_dependency';
        code?: string;
    }>;
    
    usageExamples?: Array<{
        code: string;
        location: Location;
        description?: string;
    }>;
    
    architecture?: {
        module: string;
        layer?: string;
        neighbors: string[];
    };
    
    metadata: {
        totalTokens: number;
        queryTime: number;
    };
}
```

```rust
// Server implementation
async fn handle_get_ai_context(
    &self,
    params: AIContextParams
) -> Result<AIContextResponse> {
    let path = Url::parse(&params.uri)
        .and_then(|u| u.to_file_path().ok())
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;
    
    let graph = self.graph.read().await;
    let max_tokens = params.max_tokens.unwrap_or(4000);
    
    // Find node at position
    let node_id = self.find_node_at_position(&graph, &path, params.position)?
        .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("No symbol at position"))?;
    
    let node = graph.get_node(node_id)?;
    
    // Get primary context (the symbol itself)
    let primary = self.build_primary_context(&graph, node_id)?;
    
    // Get related symbols based on context type
    let related = match params.context_type.as_str() {
        "explain" => self.get_explanation_context(&graph, node_id, max_tokens)?,
        "modify" => self.get_modification_context(&graph, node_id, max_tokens)?,
        "debug" => self.get_debug_context(&graph, node_id, max_tokens)?,
        "test" => self.get_test_context(&graph, node_id, max_tokens)?,
        _ => Vec::new(),
    };
    
    // Rank by relevance
    let ranked_related = self.rank_by_relevance(related, &params.context_type);
    
    // Build response staying within token budget
    self.build_context_response(primary, ranked_related, max_tokens)
}
```

#### 3.2.4 Impact Analysis Request
```typescript
interface ImpactAnalysisParams {
    uri: string;
    position: Position;
    analysisType: 'modify' | 'delete' | 'rename';
}

interface ImpactAnalysisResponse {
    directImpact: Array<{
        uri: string;
        range: Range;
        type: 'caller' | 'reference' | 'subclass' | 'implementation';
        severity: 'breaking' | 'warning' | 'info';
    }>;
    
    indirectImpact: Array<{
        uri: string;
        path: string[]; // Chain of dependencies
        severity: 'breaking' | 'warning' | 'info';
    }>;
    
    affectedTests: Array<{
        uri: string;
        testName: string;
    }>;
    
    summary: {
        filesAffected: number;
        breakingChanges: number;
        warnings: number;
    };
}
```

#### 3.2.5 Parser Metrics Request
```typescript
interface ParserMetricsParams {
    language?: string; // Optional: specific language, or all if omitted
}

interface ParserMetricsResponse {
    metrics: Array<{
        language: string;
        filesAttempted: number;
        filesSucceeded: number;
        filesFailed: number;
        totalEntities: number;
        totalRelationships: number;
        totalParseTimeMs: number;
        avgParseTimeMs: number;
    }>;
    
    totals: {
        filesAttempted: number;
        filesSucceeded: number;
        filesFailed: number;
        totalEntities: number;
        successRate: number;
    };
}
```

```rust
async fn handle_get_parser_metrics(
    &self,
    params: ParserMetricsParams
) -> Result<ParserMetricsResponse> {
    let all_metrics = self.parsers.all_metrics();
    
    let metrics: Vec<_> = all_metrics
        .into_iter()
        .filter(|(lang, _)| {
            params.language.as_ref().map_or(true, |l| l == *lang)
        })
        .map(|(language, m)| ParserMetric {
            language: language.to_string(),
            files_attempted: m.files_attempted,
            files_succeeded: m.files_succeeded,
            files_failed: m.files_failed,
            total_entities: m.total_entities,
            total_relationships: m.total_relationships,
            total_parse_time_ms: m.total_parse_time.as_millis() as u64,
            avg_parse_time_ms: if m.files_succeeded > 0 {
                m.total_parse_time.as_millis() as u64 / m.files_succeeded as u64
            } else {
                0
            },
        })
        .collect();
    
    let totals = Totals {
        files_attempted: metrics.iter().map(|m| m.files_attempted).sum(),
        files_succeeded: metrics.iter().map(|m| m.files_succeeded).sum(),
        files_failed: metrics.iter().map(|m| m.files_failed).sum(),
        total_entities: metrics.iter().map(|m| m.total_entities).sum(),
        success_rate: {
            let attempted: usize = metrics.iter().map(|m| m.files_attempted).sum();
            let succeeded: usize = metrics.iter().map(|m| m.files_succeeded).sum();
            if attempted > 0 {
                succeeded as f64 / attempted as f64
            } else {
                0.0
            }
        },
    };
    
    Ok(ParserMetricsResponse { metrics, totals })
}
```

### 3.3 LSP Extension Registration

```typescript
// In extension.ts
import { LanguageClient, RequestType } from 'vscode-languageclient/node';

// Define custom request types
namespace GetDependencyGraphRequest {
    export const type = new RequestType<DependencyGraphParams, DependencyGraphResponse, void>(
        'codegraph/getDependencyGraph'
    );
}

namespace GetAIContextRequest {
    export const type = new RequestType<AIContextParams, AIContextResponse, void>(
        'codegraph/getAIContext'
    );
}

namespace GetParserMetricsRequest {
    export const type = new RequestType<ParserMetricsParams, ParserMetricsResponse, void>(
        'codegraph/getParserMetrics'
    );
}

// Register custom commands
export function registerCommands(
    context: vscode.ExtensionContext,
    client: LanguageClient
) {
    context.subscriptions.push(
        vscode.commands.registerCommand(
            'codegraph.showDependencyGraph',
            async () => {
                const editor = vscode.window.activeTextEditor;
                if (!editor) return;
                
                const response = await client.sendRequest(
                    GetDependencyGraphRequest.type,
                    {
                        uri: editor.document.uri.toString(),
                        depth: 3,
                        includeExternal: false,
                        direction: 'both',
                    }
                );
                
                // Show in webview
                showGraphVisualization(context, response);
            }
        ),
        
        vscode.commands.registerCommand(
            'codegraph.showMetrics',
            async () => {
                const response = await client.sendRequest(
                    GetParserMetricsRequest.type,
                    {}
                );
                
                // Show metrics in output channel or webview
                showMetrics(response);
            }
        )
    );
}
```

---

## 4. AI Integration Architecture

### 4.1 Context Provider Interface

The extension provides code context to AI assistants through multiple channels:

#### 4.1.1 GitHub Copilot Chat Integration

```typescript
// AI Context Provider
export class CodeGraphAIProvider {
    constructor(private client: LanguageClient) {}
    
    async provideCodeContext(
        document: vscode.TextDocument,
        position: vscode.Position,
        intent: 'explain' | 'modify' | 'debug' | 'test'
    ): Promise<AIContext> {
        // Request context from LSP server
        const response = await this.client.sendRequest<AIContextResponse>(
            'codegraph/getAIContext',
            {
                uri: document.uri.toString(),
                position: {
                    line: position.line,
                    character: position.character,
                },
                contextType: intent,
                maxTokens: 4000,
            }
        );
        
        return this.formatForAI(response);
    }
    
    private formatForAI(response: AIContextResponse): AIContext {
        return {
            primary: {
                code: response.primaryContext.code,
                language: response.primaryContext.language,
                description: `${response.primaryContext.type}: ${response.primaryContext.name}`,
            },
            related: response.relatedSymbols.map(s => ({
                code: s.code,
                relationship: s.relationship,
                relevance: s.relevanceScore,
            })),
            architecture: response.architecture,
        };
    }
}

// Register with Copilot Chat (when API is available)
export function registerCopilotIntegration(
    context: vscode.ExtensionContext,
    provider: CodeGraphAIProvider
) {
    const copilotExtension = vscode.extensions.getExtension('github.copilot-chat');
    
    if (copilotExtension?.isActive) {
        // Register as context provider when Copilot exposes this API
        // This is a placeholder for future Copilot integration
        console.log('Copilot Chat integration ready');
    }
}
```

#### 4.1.2 Custom AI Chat Panel

```typescript
// Webview-based AI chat interface
export class CodeGraphChatPanel {
    private panel: vscode.WebviewPanel;
    private aiProvider: CodeGraphAIProvider;
    
    constructor(
        context: vscode.ExtensionContext,
        provider: CodeGraphAIProvider
    ) {
        this.aiProvider = provider;
        this.panel = vscode.window.createWebviewPanel(
            'codegraphChat',
            'CodeGraph AI Assistant',
            vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [
                    vscode.Uri.joinPath(context.extensionUri, 'webview')
                ],
            }
        );
        
        this.setupWebview(context);
        this.setupMessageHandlers();
    }
    
    private setupMessageHandlers() {
        this.panel.webview.onDidReceiveMessage(async (message) => {
            switch (message.type) {
                case 'chat':
                    await this.handleChatMessage(message.content);
                    break;
                case 'getContext':
                    await this.sendCurrentContext();
                    break;
            }
        });
    }
    
    private async handleChatMessage(userMessage: string) {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            this.postMessage({ type: 'error', content: 'No active editor' });
            return;
        }
        
        // Get relevant context
        const context = await this.aiProvider.provideCodeContext(
            editor.document,
            editor.selection.active,
            'explain'
        );
        
        // Build enhanced prompt
        const enhancedPrompt = this.buildPrompt(userMessage, context);
        
        // Send to AI backend (implement based on your AI service)
        // This is a placeholder - integrate with Claude API, OpenAI, etc.
        this.postMessage({
            type: 'contextReady',
            prompt: enhancedPrompt,
            context: context,
        });
    }
    
    private buildPrompt(userMessage: string, context: AIContext): string {
        return `
You are analyzing code with the following context:

## Primary Code
\`\`\`${context.primary.language}
${context.primary.code}
\`\`\`

${context.related.length > 0 ? `
## Related Code
${context.related.slice(0, 5).map(r => `
### ${r.relationship} (relevance: ${(r.relevance * 100).toFixed(0)}%)
\`\`\`
${r.code}
\`\`\`
`).join('\n')}
` : ''}

${context.architecture ? `
## Architecture Context
- Module: ${context.architecture.module}
- Neighbors: ${context.architecture.neighbors.join(', ')}
` : ''}

## User Question
${userMessage}
`;
    }
    
    private postMessage(message: any) {
        this.panel.webview.postMessage(message);
    }
}
```

### 4.2 Smart Context Selection

The AI context provider uses intelligent strategies to select relevant code:

```rust
// In codegraph-lsp/src/ai_context.rs

impl CodeGraphBackend {
    /// Get context optimized for explaining code
    fn get_explanation_context(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        max_tokens: usize
    ) -> Result<Vec<RelatedSymbol>> {
        let mut budget = TokenBudget::new(max_tokens);
        let mut context = Vec::new();
        
        // Priority 1: Direct dependencies (imports, types used)
        let deps = self.get_direct_dependencies(graph, node_id)?;
        for (dep_id, relationship) in deps.iter().take(5) {
            if let Some(code) = self.get_node_source_code(graph, *dep_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: *dep_id,
                        relationship: relationship.clone(),
                        code,
                        relevance: 1.0,
                    });
                }
            }
        }
        
        // Priority 2: Direct callers (who uses this?)
        let callers = self.get_callers(graph, node_id)?;
        for caller_id in callers.iter().take(3) {
            if let Some(code) = self.get_node_source_code(graph, *caller_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: *caller_id,
                        relationship: "called_by".to_string(),
                        code,
                        relevance: 0.8,
                    });
                }
            }
        }
        
        // Priority 3: Type hierarchy (for classes/interfaces)
        if let Some(parent_id) = self.get_parent_type(graph, node_id)? {
            if let Some(code) = self.get_node_source_code(graph, parent_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: parent_id,
                        relationship: "inherits".to_string(),
                        code,
                        relevance: 0.9,
                    });
                }
            }
        }
        
        // Priority 4: Sibling functions in same module
        let siblings = self.get_sibling_symbols(graph, node_id)?;
        for sibling_id in siblings.iter().take(2) {
            if budget.has_budget() {
                if let Some(code) = self.get_node_source_code(graph, *sibling_id)? {
                    let tokens = estimate_tokens(&code);
                    if budget.consume(tokens) {
                        context.push(RelatedSymbol {
                            node_id: *sibling_id,
                            relationship: "sibling".to_string(),
                            code,
                            relevance: 0.5,
                        });
                    }
                }
            }
        }
        
        Ok(context)
    }
    
    /// Get context optimized for modifying code
    fn get_modification_context(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        max_tokens: usize
    ) -> Result<Vec<RelatedSymbol>> {
        let mut budget = TokenBudget::new(max_tokens);
        let mut context = Vec::new();
        
        // Priority 1: Tests for this symbol
        let tests = self.find_tests_for(graph, node_id)?;
        for test_id in tests {
            if let Some(code) = self.get_node_source_code(graph, test_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: test_id,
                        relationship: "tests".to_string(),
                        code,
                        relevance: 1.0,
                    });
                }
            }
        }
        
        // Priority 2: All direct callers (breaking changes concern)
        let callers = self.get_callers(graph, node_id)?;
        for caller_id in callers.iter().take(5) {
            if let Some(code) = self.get_node_source_code(graph, *caller_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: *caller_id,
                        relationship: "called_by".to_string(),
                        code,
                        relevance: 0.9,
                    });
                }
            }
        }
        
        // Priority 3: Similar functions (for consistency)
        let similar = self.find_similar_functions(graph, node_id, 0.7)?;
        for (sim_id, similarity) in similar.iter().take(2) {
            if budget.has_budget() {
                if let Some(code) = self.get_node_source_code(graph, *sim_id)? {
                    let tokens = estimate_tokens(&code);
                    if budget.consume(tokens) {
                        context.push(RelatedSymbol {
                            node_id: *sim_id,
                            relationship: "similar".to_string(),
                            code,
                            relevance: *similarity,
                        });
                    }
                }
            }
        }
        
        Ok(context)
    }
    
    /// Get context optimized for debugging
    fn get_debug_context(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        max_tokens: usize
    ) -> Result<Vec<RelatedSymbol>> {
        let mut budget = TokenBudget::new(max_tokens);
        let mut context = Vec::new();
        
        // Priority 1: Full call chain up to entry point
        let call_chain = self.get_call_chain_to_entry(graph, node_id)?;
        for (chain_node_id, depth) in call_chain {
            if let Some(code) = self.get_node_source_code(graph, chain_node_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: chain_node_id,
                        relationship: format!("call_chain_depth_{}", depth),
                        code,
                        relevance: 1.0 - (depth as f64 * 0.1),
                    });
                } else {
                    break; // Stop if we run out of budget
                }
            }
        }
        
        // Priority 2: Data flow dependencies
        let data_deps = self.get_data_dependencies(graph, node_id)?;
        for dep_id in data_deps.iter().take(3) {
            if budget.has_budget() {
                if let Some(code) = self.get_node_source_code(graph, *dep_id)? {
                    let tokens = estimate_tokens(&code);
                    if budget.consume(tokens) {
                        context.push(RelatedSymbol {
                            node_id: *dep_id,
                            relationship: "data_flow".to_string(),
                            code,
                            relevance: 0.8,
                        });
                    }
                }
            }
        }
        
        Ok(context)
    }
    
    /// Get context optimized for writing tests
    fn get_test_context(
        &self,
        graph: &CodeGraph,
        node_id: NodeId,
        max_tokens: usize
    ) -> Result<Vec<RelatedSymbol>> {
        let mut budget = TokenBudget::new(max_tokens);
        let mut context = Vec::new();
        
        // Priority 1: Existing tests for similar functions
        let similar = self.find_similar_functions(graph, node_id, 0.5)?;
        for (sim_id, _) in similar.iter().take(3) {
            let tests = self.find_tests_for(graph, *sim_id)?;
            for test_id in tests.iter().take(2) {
                if let Some(code) = self.get_node_source_code(graph, *test_id)? {
                    let tokens = estimate_tokens(&code);
                    if budget.consume(tokens) {
                        context.push(RelatedSymbol {
                            node_id: *test_id,
                            relationship: "example_test".to_string(),
                            code,
                            relevance: 0.9,
                        });
                    }
                }
            }
        }
        
        // Priority 2: Dependencies to mock
        let deps = self.get_direct_dependencies(graph, node_id)?;
        for (dep_id, _) in deps.iter().take(3) {
            if let Some(code) = self.get_node_source_code(graph, *dep_id)? {
                let tokens = estimate_tokens(&code);
                if budget.consume(tokens) {
                    context.push(RelatedSymbol {
                        node_id: *dep_id,
                        relationship: "dependency_to_mock".to_string(),
                        code,
                        relevance: 0.7,
                    });
                }
            }
        }
        
        Ok(context)
    }
}

/// Token budget manager
struct TokenBudget {
    total: usize,
    used: usize,
}

impl TokenBudget {
    fn new(total: usize) -> Self {
        Self { total, used: 0 }
    }
    
    fn consume(&mut self, tokens: usize) -> bool {
        if self.used + tokens <= self.total {
            self.used += tokens;
            true
        } else {
            false
        }
    }
    
    fn has_budget(&self) -> bool {
        self.used < self.total
    }
    
    fn remaining(&self) -> usize {
        self.total.saturating_sub(self.used)
    }
}

/// Estimate tokens in a code string (rough approximation)
fn estimate_tokens(code: &str) -> usize {
    // Rough estimate: ~4 characters per token
    code.len() / 4
}
```

### 4.3 Context Ranking Algorithm

```rust
impl CodeGraphBackend {
    fn rank_by_relevance(
        &self,
        mut symbols: Vec<RelatedSymbol>,
        context_type: &str
    ) -> Vec<RelatedSymbol> {
        symbols.sort_by(|a, b| {
            let score_a = self.calculate_relevance_score(a, context_type);
            let score_b = self.calculate_relevance_score(b, context_type);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        symbols
    }
    
    fn calculate_relevance_score(
        &self,
        symbol: &RelatedSymbol,
        context_type: &str
    ) -> f64 {
        let mut score = symbol.relevance;
        
        // Boost based on relationship type and context
        match (context_type, symbol.relationship.as_str()) {
            // For explanation, prioritize dependencies and parents
            ("explain", "uses") => score *= 1.2,
            ("explain", "inherits") => score *= 1.3,
            
            // For modification, prioritize callers and tests
            ("modify", "called_by") => score *= 1.5,
            ("modify", "tests") => score *= 1.8,
            
            // For debugging, prioritize call chain and data flow
            ("debug", r) if r.starts_with("call_chain") => score *= 1.6,
            ("debug", "data_flow") => score *= 1.4,
            
            // For testing, prioritize example tests
            ("test", "example_test") => score *= 1.7,
            ("test", "dependency_to_mock") => score *= 1.3,
            
            _ => {},
        }
        
        // Penalty for very large symbols (too much context)
        let lines = symbol.code.lines().count();
        if lines > 100 {
            score *= 0.7;
        } else if lines > 50 {
            score *= 0.85;
        }
        
        score
    }
}
```

---

## 5. Graph Visualization

### 5.1 Webview Architecture

```typescript
// src/views/graphPanel.ts

import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';

export class GraphVisualizationPanel {
    public static currentPanel: GraphVisualizationPanel | undefined;
    private readonly panel: vscode.WebviewPanel;
    private readonly extensionUri: vscode.Uri;
    private disposables: vscode.Disposable[] = [];
    
    private constructor(
        panel: vscode.WebviewPanel,
        extensionUri: vscode.Uri,
        private client: LanguageClient
    ) {
        this.panel = panel;
        this.extensionUri = extensionUri;
        
        this.panel.webview.html = this.getWebviewContent();
        this.setupMessageHandlers();
        
        this.panel.onDidDispose(() => this.dispose(), null, this.disposables);
    }
    
    public static createOrShow(
        extensionUri: vscode.Uri,
        client: LanguageClient
    ) {
        const column = vscode.window.activeTextEditor
            ? vscode.window.activeTextEditor.viewColumn
            : undefined;
        
        if (GraphVisualizationPanel.currentPanel) {
            GraphVisualizationPanel.currentPanel.panel.reveal(column);
            return GraphVisualizationPanel.currentPanel;
        }
        
        const panel = vscode.window.createWebviewPanel(
            'codegraphVisualization',
            'CodeGraph Visualization',
            column || vscode.ViewColumn.One,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [
                    vscode.Uri.joinPath(extensionUri, 'webview', 'dist')
                ],
            }
        );
        
        GraphVisualizationPanel.currentPanel = new GraphVisualizationPanel(
            panel,
            extensionUri,
            client
        );
        
        return GraphVisualizationPanel.currentPanel;
    }
    
    private setupMessageHandlers() {
        this.panel.webview.onDidReceiveMessage(
            async (message) => {
                switch (message.command) {
                    case 'loadDependencyGraph':
                        await this.loadDependencyGraph(message.params);
                        break;
                    case 'loadCallGraph':
                        await this.loadCallGraph(message.params);
                        break;
                    case 'nodeClick':
                        await this.handleNodeClick(message.nodeId);
                        break;
                    case 'expandNode':
                        await this.expandNode(message.nodeId);
                        break;
                }
            },
            null,
            this.disposables
        );
    }
    
    private async loadDependencyGraph(params: DependencyGraphParams) {
        const graph = await this.client.sendRequest<DependencyGraphResponse>(
            'codegraph/getDependencyGraph',
            params
        );
        
        this.panel.webview.postMessage({
            command: 'renderGraph',
            graphType: 'dependency',
            data: graph,
        });
    }
    
    private async loadCallGraph(params: CallGraphParams) {
        const graph = await this.client.sendRequest<CallGraphResponse>(
            'codegraph/getCallGraph',
            params
        );
        
        this.panel.webview.postMessage({
            command: 'renderGraph',
            graphType: 'call',
            data: graph,
        });
    }
    
    private async handleNodeClick(nodeId: string) {
        // Navigate to node in editor
        const location = await this.client.sendRequest<Location>(
            'codegraph/getNodeLocation',
            { nodeId }
        );
        
        if (location) {
            const uri = vscode.Uri.parse(location.uri);
            const range = new vscode.Range(
                location.range.start.line,
                location.range.start.character,
                location.range.end.line,
                location.range.end.character
            );
            
            vscode.window.showTextDocument(uri, { selection: range });
        }
    }
    
    private getWebviewContent(): string {
        const scriptUri = this.panel.webview.asWebviewUri(
            vscode.Uri.joinPath(this.extensionUri, 'webview', 'dist', 'bundle.js')
        );
        
        const styleUri = this.panel.webview.asWebviewUri(
            vscode.Uri.joinPath(this.extensionUri, 'webview', 'dist', 'styles.css')
        );
        
        const nonce = getNonce();
        
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${this.panel.webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
    <link href="${styleUri}" rel="stylesheet">
    <title>CodeGraph Visualization</title>
</head>
<body>
    <div id="root"></div>
    <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
    }
    
    private dispose() {
        GraphVisualizationPanel.currentPanel = undefined;
        this.panel.dispose();
        while (this.disposables.length) {
            const x = this.disposables.pop();
            if (x) x.dispose();
        }
    }
}

function getNonce(): string {
    let text = '';
    const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    for (let i = 0; i < 32; i++) {
        text += possible.charAt(Math.floor(Math.random() * possible.length));
    }
    return text;
}
```

### 5.2 Graph Rendering (React + D3)

```typescript
// webview/src/GraphView.tsx

import React, { useEffect, useRef, useState } from 'react';
import * as d3 from 'd3';

interface GraphNode {
    id: string;
    label: string;
    type: string;
    language?: string;
    x?: number;
    y?: number;
}

interface GraphEdge {
    from: string;
    to: string;
    type: string;
    source?: GraphNode;
    target?: GraphNode;
}

interface GraphViewProps {
    nodes: GraphNode[];
    edges: GraphEdge[];
    graphType: 'dependency' | 'call';
    onNodeClick: (nodeId: string) => void;
    onExpandNode: (nodeId: string) => void;
}

export const GraphView: React.FC<GraphViewProps> = ({
    nodes,
    edges,
    graphType,
    onNodeClick,
    onExpandNode
}) => {
    const svgRef = useRef<SVGSVGElement>(null);
    const [dimensions, setDimensions] = useState({ width: 800, height: 600 });
    
    useEffect(() => {
        const handleResize = () => {
            if (svgRef.current?.parentElement) {
                setDimensions({
                    width: svgRef.current.parentElement.clientWidth,
                    height: svgRef.current.parentElement.clientHeight
                });
            }
        };
        
        handleResize();
        window.addEventListener('resize', handleResize);
        return () => window.removeEventListener('resize', handleResize);
    }, []);
    
    useEffect(() => {
        if (!svgRef.current || nodes.length === 0) return;
        
        const { width, height } = dimensions;
        
        // Prepare data
        const nodeMap = new Map(nodes.map(n => [n.id, { ...n }]));
        const links = edges.map(e => ({
            ...e,
            source: nodeMap.get(e.from)!,
            target: nodeMap.get(e.to)!
        })).filter(l => l.source && l.target);
        
        // Create force simulation
        const simulation = d3.forceSimulation(Array.from(nodeMap.values()))
            .force('link', d3.forceLink(links)
                .id((d: any) => d.id)
                .distance(120))
            .force('charge', d3.forceManyBody().strength(-400))
            .force('center', d3.forceCenter(width / 2, height / 2))
            .force('collision', d3.forceCollide().radius(60));
        
        const svg = d3.select(svgRef.current);
        svg.selectAll('*').remove();
        
        // Add zoom behavior
        const g = svg.append('g');
        svg.call(d3.zoom<SVGSVGElement, unknown>()
            .scaleExtent([0.1, 4])
            .on('zoom', (event) => g.attr('transform', event.transform)));
        
        // Create arrow markers
        const defs = g.append('defs');
        ['import', 'call', 'extends', 'implements'].forEach(type => {
            defs.append('marker')
                .attr('id', `arrow-${type}`)
                .attr('viewBox', '0 -5 10 10')
                .attr('refX', 25)
                .attr('refY', 0)
                .attr('markerWidth', 6)
                .attr('markerHeight', 6)
                .attr('orient', 'auto')
                .append('path')
                .attr('d', 'M0,-5L10,0L0,5')
                .attr('fill', getEdgeColor(type));
        });
        
        // Draw edges
        const linkElements = g.append('g')
            .selectAll('line')
            .data(links)
            .enter().append('line')
            .attr('stroke', d => getEdgeColor(d.type))
            .attr('stroke-width', 2)
            .attr('stroke-opacity', 0.6)
            .attr('marker-end', d => `url(#arrow-${d.type})`);
        
        // Draw nodes
        const nodeElements = g.append('g')
            .selectAll('g')
            .data(Array.from(nodeMap.values()))
            .enter().append('g')
            .call(d3.drag<any, GraphNode>()
                .on('start', (event, d) => {
                    if (!event.active) simulation.alphaTarget(0.3).restart();
                    d.fx = d.x;
                    d.fy = d.y;
                })
                .on('drag', (event, d) => {
                    d.fx = event.x;
                    d.fy = event.y;
                })
                .on('end', (event, d) => {
                    if (!event.active) simulation.alphaTarget(0);
                    d.fx = null;
                    d.fy = null;
                }));
        
        // Node circles
        nodeElements.append('circle')
            .attr('r', d => getNodeRadius(d.type))
            .attr('fill', d => getNodeColor(d.type, d.language))
            .attr('stroke', '#fff')
            .attr('stroke-width', 2)
            .style('cursor', 'pointer')
            .on('click', (event, d) => onNodeClick(d.id))
            .on('dblclick', (event, d) => onExpandNode(d.id));
        
        // Node labels
        nodeElements.append('text')
            .text(d => truncateLabel(d.label, 20))
            .attr('x', 0)
            .attr('y', d => getNodeRadius(d.type) + 15)
            .attr('text-anchor', 'middle')
            .attr('font-size', '12px')
            .attr('fill', 'var(--vscode-foreground)');
        
        // Language badge
        nodeElements.filter(d => d.language)
            .append('text')
            .text(d => d.language?.substring(0, 2).toUpperCase() || '')
            .attr('x', 0)
            .attr('y', 4)
            .attr('text-anchor', 'middle')
            .attr('font-size', '10px')
            .attr('font-weight', 'bold')
            .attr('fill', '#fff');
        
        // Update positions on simulation tick
        simulation.on('tick', () => {
            linkElements
                .attr('x1', d => (d.source as any).x)
                .attr('y1', d => (d.source as any).y)
                .attr('x2', d => (d.target as any).x)
                .attr('y2', d => (d.target as any).y);
            
            nodeElements.attr('transform', d => `translate(${d.x},${d.y})`);
        });
        
        return () => simulation.stop();
    }, [nodes, edges, dimensions, onNodeClick, onExpandNode]);
    
    return (
        <svg 
            ref={svgRef} 
            width={dimensions.width} 
            height={dimensions.height}
            style={{ background: 'var(--vscode-editor-background)' }}
        />
    );
};

function getNodeColor(type: string, language?: string): string {
    // Language-based colors
    if (language) {
        const languageColors: Record<string, string> = {
            python: '#3776AB',
            rust: '#DEA584',
            typescript: '#3178C6',
            javascript: '#F7DF1E',
            go: '#00ADD8',
        };
        if (languageColors[language]) return languageColors[language];
    }
    
    // Type-based colors
    const typeColors: Record<string, string> = {
        module: '#4CAF50',
        file: '#2196F3',
        function: '#9C27B0',
        class: '#FF9800',
        trait: '#E91E63',
        interface: '#00BCD4',
    };
    
    return typeColors[type] || '#9E9E9E';
}

function getNodeRadius(type: string): number {
    const radii: Record<string, number> = {
        module: 25,
        file: 20,
        function: 15,
        class: 22,
        trait: 18,
    };
    return radii[type] || 15;
}

function getEdgeColor(type: string): string {
    const colors: Record<string, string> = {
        import: '#4CAF50',
        call: '#2196F3',
        extends: '#FF9800',
        implements: '#9C27B0',
    };
    return colors[type] || '#999';
}

function truncateLabel(label: string, maxLength: number): string {
    return label.length > maxLength 
        ? label.substring(0, maxLength - 3) + '...' 
        : label;
}
```

---

## 6. Incremental Updates

### 6.1 File System Watching

```rust
// src/watcher.rs

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use codegraph::CodeGraph;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    graph: Arc<RwLock<CodeGraph>>,
    parsers: Arc<ParserRegistry>,
    client: tower_lsp::Client,
}

impl FileWatcher {
    pub fn new(
        graph: Arc<RwLock<CodeGraph>>,
        parsers: Arc<ParserRegistry>,
        client: tower_lsp::Client,
    ) -> Result<Self, notify::Error> {
        let (tx, mut rx) = mpsc::channel(100);
        
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        )?;
        
        // Spawn event handler
        let graph_clone = Arc::clone(&graph);
        let parsers_clone = Arc::clone(&parsers);
        let client_clone = client.clone();
        
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::handle_event(
                    &graph_clone,
                    &parsers_clone,
                    &client_clone,
                    event,
                ).await;
            }
        });
        
        Ok(Self { watcher, graph, parsers, client })
    }
    
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.watcher.watch(path, RecursiveMode::Recursive)
    }
    
    async fn handle_event(
        graph: &Arc<RwLock<CodeGraph>>,
        parsers: &Arc<ParserRegistry>,
        client: &tower_lsp::Client,
        event: Event,
    ) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if let Err(e) = Self::handle_file_change(graph, parsers, &path).await {
                        client.log_message(
                            tower_lsp::lsp_types::MessageType::WARNING,
                            format!("Error processing {}: {}", path.display(), e)
                        ).await;
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if let Err(e) = Self::handle_file_remove(graph, &path).await {
                        client.log_message(
                            tower_lsp::lsp_types::MessageType::WARNING,
                            format!("Error removing {}: {}", path.display(), e)
                        ).await;
                    }
                }
            }
            _ => {}
        }
    }
    
    async fn handle_file_change(
        graph: &Arc<RwLock<CodeGraph>>,
        parsers: &Arc<ParserRegistry>,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Skip non-parseable files
        let parser = match parsers.parser_for_path(path) {
            Some(p) => p,
            None => return Ok(()),
        };
        
        // Read file content
        let content = tokio::fs::read_to_string(path).await?;
        
        // Remove old entries and re-parse
        let mut graph = graph.write().await;
        
        // Remove existing nodes for this file
        let path_str = path.to_string_lossy();
        if let Ok(nodes) = graph.query_nodes_by_property("path", &path_str) {
            for node_id in nodes {
                let _ = graph.remove_node_with_edges(node_id);
            }
        }
        
        // Parse and add new nodes
        parser.parse_source(&content, path, &mut graph)?;
        
        Ok(())
    }
    
    async fn handle_file_remove(
        graph: &Arc<RwLock<CodeGraph>>,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut graph = graph.write().await;
        
        let path_str = path.to_string_lossy();
        if let Ok(nodes) = graph.query_nodes_by_property("path", &path_str) {
            for node_id in nodes {
                let _ = graph.remove_node_with_edges(node_id);
            }
        }
        
        Ok(())
    }
}
```

### 6.2 Incremental Graph Updates

Since the CodeGraph doesn't have a built-in `update_file` method, we use a remove-and-reparse strategy:

```rust
// src/graph_updater.rs

use codegraph::{CodeGraph, NodeId, NodeType};
use codegraph_parser_api::{CodeParser, FileInfo};
use std::collections::HashSet;
use std::path::Path;

pub struct GraphUpdater<'a> {
    graph: &'a mut CodeGraph,
}

impl<'a> GraphUpdater<'a> {
    pub fn new(graph: &'a mut CodeGraph) -> Self {
        Self { graph }
    }
    
    /// Update graph for a single file
    /// 
    /// Strategy: Remove all nodes associated with the file, then re-parse
    pub fn update_file(
        &mut self,
        path: &Path,
        content: &str,
        parser: &dyn CodeParser,
    ) -> Result<FileInfo, UpdateError> {
        // Step 1: Find and remove existing nodes for this file
        let removed_nodes = self.remove_file_nodes(path)?;
        
        // Step 2: Re-parse the file
        let file_info = parser.parse_source(content, path, self.graph)
            .map_err(|e| UpdateError::ParseError(e.to_string()))?;
        
        // Step 3: Log the update
        tracing::info!(
            "Updated {}: removed {} nodes, added {} new",
            path.display(),
            removed_nodes,
            file_info.functions.len() + file_info.classes.len()
        );
        
        Ok(file_info)
    }
    
    /// Remove all nodes associated with a file path
    fn remove_file_nodes(&mut self, path: &Path) -> Result<usize, UpdateError> {
        let path_str = path.to_string_lossy();
        
        let nodes = self.graph
            .query_nodes_by_property("path", &path_str)
            .map_err(|e| UpdateError::GraphError(e.to_string()))?;
        
        let count = nodes.len();
        
        for node_id in nodes {
            // Remove edges first (both incoming and outgoing)
            if let Ok(outgoing) = self.graph.get_outgoing_edges(node_id) {
                for edge in outgoing {
                    let _ = self.graph.remove_edge(edge.id);
                }
            }
            
            if let Ok(incoming) = self.graph.get_incoming_edges(node_id) {
                for edge in incoming {
                    let _ = self.graph.remove_edge(edge.id);
                }
            }
            
            // Remove the node
            let _ = self.graph.remove_node(node_id);
        }
        
        Ok(count)
    }
    
    /// Batch update multiple files
    pub fn update_files(
        &mut self,
        files: &[(PathBuf, String)],
        parsers: &ParserRegistry,
    ) -> BatchUpdateResult {
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        
        for (path, content) in files {
            if let Some(parser) = parsers.parser_for_path(path) {
                match self.update_file(path, content, parser.as_ref()) {
                    Ok(info) => succeeded.push((path.clone(), info)),
                    Err(e) => failed.push((path.clone(), e.to_string())),
                }
            }
        }
        
        BatchUpdateResult { succeeded, failed }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Graph error: {0}")]
    GraphError(String),
    
    #[error("IO error: {0}")]
    IoError(String),
}

pub struct BatchUpdateResult {
    pub succeeded: Vec<(PathBuf, FileInfo)>,
    pub failed: Vec<(PathBuf, String)>,
}
```

---

## 7. Implementation Plan

### Phase 1: Foundation (Weeks 1-3)
- Set up project structure (Rust workspace + TypeScript extension)
- Implement basic LSP server with `tower-lsp`
- Integrate `codegraph-parser-api` and create `ParserRegistry`
- Implement standard LSP features:
  - Text synchronization (did_open, did_change, did_save, did_close)
  - Go to definition
  - Find references
- Basic VS Code extension with LSP client
- File system watching for incremental updates

**Deliverable**: Working extension with basic navigation for one language (Python)

### Phase 2: Multi-Language Support (Weeks 4-6)
- Integrate all available parsers (Python, Rust, TypeScript, Go)
- Add call hierarchy support
- Implement hover with cross-language info
- Add document symbols
- Test cross-language navigation

**Deliverable**: Cross-language navigation working

### Phase 3: Custom Extensions (Weeks 7-9)
- Implement custom LSP requests:
  - `codegraph/getDependencyGraph`
  - `codegraph/getCallGraph`
  - `codegraph/analyzeImpact`
  - `codegraph/getParserMetrics`
- Add tree view providers in extension
- Implement basic webview visualization with D3.js

**Deliverable**: Graph visualization working

### Phase 4: AI Integration (Weeks 10-12)
- Implement `codegraph/getAIContext` request
- Add smart context selection algorithms
- Implement token budgeting system
- Build custom AI chat panel webview
- Add context ranking algorithm
- Prepare for GitHub Copilot integration (when API available)

**Deliverable**: AI can use codegraph context

### Phase 5: Polish & Performance (Weeks 13-14)
- Performance optimization (caching, lazy loading)
- Query result caching
- Error handling improvements
- Documentation (README, API docs, user guide)
- Comprehensive testing
- Package for VS Code Marketplace

**Deliverable**: Production-ready extension

---

## 8. Technical Considerations

### 8.1 Performance

**Caching Strategy:**
```rust
use dashmap::DashMap;
use lru::LruCache;
use std::sync::Mutex;

pub struct QueryCache {
    // Fast lookup caches
    definitions: DashMap<(PathBuf, Position), NodeId>,
    references: DashMap<NodeId, Vec<Location>>,
    
    // LRU caches for expensive queries
    call_hierarchies: Mutex<LruCache<NodeId, CallHierarchy>>,
    dependency_graphs: Mutex<LruCache<(PathBuf, usize), DependencyGraph>>,
    ai_contexts: Mutex<LruCache<(NodeId, String), AIContextResponse>>,
}

impl QueryCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            definitions: DashMap::new(),
            references: DashMap::new(),
            call_hierarchies: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(capacity).unwrap()
            )),
            dependency_graphs: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(capacity / 2).unwrap()
            )),
            ai_contexts: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(capacity).unwrap()
            )),
        }
    }
    
    /// Invalidate all cache entries for a file
    pub fn invalidate_file(&self, path: &Path) {
        // Remove definition entries for this file
        self.definitions.retain(|(p, _), _| p != path);
        
        // Clear other caches (could be more selective)
        self.references.clear();
        self.call_hierarchies.lock().unwrap().clear();
        // Dependency graphs might still be valid for other files
    }
    
    /// Invalidate entire cache
    pub fn invalidate_all(&self) {
        self.definitions.clear();
        self.references.clear();
        self.call_hierarchies.lock().unwrap().clear();
        self.dependency_graphs.lock().unwrap().clear();
        self.ai_contexts.lock().unwrap().clear();
    }
}
```

**Lazy Loading:**
- Only parse files when opened or referenced
- Load dependency graphs on-demand
- Paginate large result sets

**Indexing:**
```rust
use codegraph::NodeId;
use std::collections::HashMap;
use dashmap::DashMap;

/// Secondary indexes for fast lookups
pub struct SymbolIndex {
    /// Name -> NodeIds (for workspace symbol search)
    by_name: DashMap<String, Vec<NodeId>>,
    
    /// File path -> NodeIds (for file-scoped queries)
    by_file: DashMap<PathBuf, Vec<NodeId>>,
    
    /// Node type -> NodeIds (for type-filtered queries)
    by_type: DashMap<NodeType, Vec<NodeId>>,
    
    /// Position index for fast position lookups
    by_position: DashMap<PathBuf, Vec<(Range, NodeId)>>,
}

impl SymbolIndex {
    pub fn add_file(&self, path: PathBuf, file_info: &FileInfo, graph: &CodeGraph) {
        let mut file_nodes = Vec::new();
        let mut positions = Vec::new();
        
        for &node_id in file_info.functions.iter()
            .chain(file_info.classes.iter())
            .chain(file_info.traits.iter())
        {
            if let Ok(node) = graph.get_node(node_id) {
                // Index by name
                if let Some(name) = node.properties.get("name") {
                    self.by_name.entry(name.clone())
                        .or_default()
                        .push(node_id);
                }
                
                // Index by type
                self.by_type.entry(node.node_type)
                    .or_default()
                    .push(node_id);
                
                file_nodes.push(node_id);
                
                // Index by position
                if let Some(range) = extract_range(&node.properties) {
                    positions.push((range, node_id));
                }
            }
        }
        
        self.by_file.insert(path.clone(), file_nodes);
        
        // Sort positions for binary search
        positions.sort_by_key(|(r, _)| (r.start.line, r.start.character));
        self.by_position.insert(path, positions);
    }
    
    pub fn remove_file(&self, path: &Path) {
        if let Some((_, nodes)) = self.by_file.remove(path) {
            // Remove from other indexes
            for node_id in nodes {
                // This is expensive - consider lazy cleanup
                self.by_name.retain(|_, v| {
                    v.retain(|&id| id != node_id);
                    !v.is_empty()
                });
            }
        }
        self.by_position.remove(path);
    }
    
    /// Fast position-based lookup
    pub fn find_at_position(&self, path: &Path, position: Position) -> Option<NodeId> {
        let positions = self.by_position.get(path)?;
        
        // Binary search for the range containing position
        positions.iter()
            .find(|(range, _)| contains_position(range, position))
            .map(|(_, node_id)| *node_id)
    }
}
```

### 8.2 Error Handling

```rust
use thiserror::Error;
use codegraph_parser_api::ParserError;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("Symbol not found at position")]
    SymbolNotFound,
    
    #[error("File not indexed: {0}")]
    FileNotIndexed(PathBuf),
    
    #[error("Parser error: {0}")]
    Parser(#[from] ParserError),
    
    #[error("Graph error: {0}")]
    Graph(String),
    
    #[error("Invalid URI: {0}")]
    InvalidUri(String),
    
    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
    
    #[error("Cache error: {0}")]
    Cache(String),
}

// Convert to LSP errors
impl From<LspError> for tower_lsp::jsonrpc::Error {
    fn from(err: LspError) -> Self {
        let code = match &err {
            LspError::SymbolNotFound => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::FileNotIndexed(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::InvalidUri(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::UnsupportedLanguage(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            _ => tower_lsp::jsonrpc::ErrorCode::InternalError,
        };
        
        tower_lsp::jsonrpc::Error {
            code,
            message: err.to_string().into(),
            data: None,
        }
    }
}
```

### 8.3 Configuration

```json
// VS Code settings schema (package.json contributes.configuration)
{
    "codegraph.enabled": {
        "type": "boolean",
        "default": true,
        "description": "Enable CodeGraph extension"
    },
    "codegraph.languages": {
        "type": "array",
        "items": { "type": "string" },
        "default": ["python", "rust", "typescript", "javascript", "go"],
        "description": "Languages to index"
    },
    "codegraph.indexOnStartup": {
        "type": "boolean",
        "default": true,
        "description": "Index workspace on startup"
    },
    "codegraph.maxFileSizeKB": {
        "type": "number",
        "default": 1024,
        "description": "Maximum file size to index (KB)"
    },
    "codegraph.excludePatterns": {
        "type": "array",
        "items": { "type": "string" },
        "default": [
            "**/node_modules/**",
            "**/target/**",
            "**/__pycache__/**",
            "**/dist/**",
            "**/build/**",
            "**/.git/**"
        ],
        "description": "Glob patterns for files to exclude"
    },
    "codegraph.includePrivate": {
        "type": "boolean",
        "default": true,
        "description": "Include private/internal symbols"
    },
    "codegraph.includeTests": {
        "type": "boolean",
        "default": true,
        "description": "Include test files and functions"
    },
    "codegraph.ai.maxContextTokens": {
        "type": "number",
        "default": 4000,
        "description": "Maximum tokens for AI context"
    },
    "codegraph.ai.contextStrategy": {
        "type": "string",
        "enum": ["minimal", "smart", "maximum"],
        "default": "smart",
        "description": "AI context selection strategy"
    },
    "codegraph.visualization.defaultDepth": {
        "type": "number",
        "default": 3,
        "description": "Default depth for graph visualizations"
    },
    "codegraph.cache.enabled": {
        "type": "boolean",
        "default": true,
        "description": "Enable query caching"
    },
    "codegraph.cache.maxSizeMB": {
        "type": "number",
        "default": 500,
        "description": "Maximum cache size (MB)"
    },
    "codegraph.parallelParsing": {
        "type": "boolean",
        "default": true,
        "description": "Enable parallel file parsing"
    }
}
```

**Configuration Mapping (TypeScript to Rust):**
```typescript
// In extension.ts
function getParserConfig(): ParserConfig {
    const config = vscode.workspace.getConfiguration('codegraph');
    
    return {
        include_private: config.get('includePrivate', true),
        include_tests: config.get('includeTests', true),
        parse_docs: true,
        max_file_size: config.get('maxFileSizeKB', 1024) * 1024,
        follow_modules: true,
        file_extensions: [], // Set by individual parsers
        exclude_dirs: config.get('excludePatterns', [])
            .filter((p: string) => !p.includes('*'))
            .map((p: string) => p.replace(/\*\*\//g, '')),
        parallel: config.get('parallelParsing', true),
        num_threads: undefined, // Use default (num_cpus)
    };
}
```

### 8.4 Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::*;
    use codegraph::CodeGraph;
    use tempfile::TempDir;
    
    async fn create_test_backend() -> (CodeGraphBackend, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let graph = CodeGraph::in_memory().unwrap();
        let parsers = ParserRegistry::new();
        
        // Create a mock client
        let (service, _) = tower_lsp::LspService::new(|client| {
            CodeGraphBackend {
                client,
                graph: Arc::new(RwLock::new(graph)),
                parsers: Arc::new(parsers),
                file_cache: Arc::new(DashMap::new()),
                query_cache: Arc::new(QueryCache::new(100)),
            }
        });
        
        // ... setup
        (backend, temp_dir)
    }
    
    #[tokio::test]
    async fn test_goto_definition_python() {
        let (backend, _temp) = create_test_backend().await;
        
        // Index test file
        let source = r#"
def foo():
    pass

def bar():
    foo()  # Line 5, col 4
"#;
        
        {
            let mut graph = backend.graph.write().await;
            backend.parsers.get_parser("python").unwrap()
                .parse_source(source, Path::new("test.py"), &mut graph)
                .unwrap();
        }
        
        // Request definition at foo() call
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path("/test.py").unwrap(),
                },
                position: Position { line: 5, character: 4 },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        
        let result = backend.goto_definition(params).await.unwrap();
        
        assert!(result.is_some());
        if let Some(GotoDefinitionResponse::Scalar(location)) = result {
            assert_eq!(location.range.start.line, 1); // foo defined on line 1
        }
    }
    
    #[tokio::test]
    async fn test_cross_language_references() {
        let (backend, temp) = create_test_backend().await;
        
        // Create Python file that imports from a Rust module (via PyO3)
        let python_source = r#"
from my_rust_lib import process_data

def main():
    result = process_data(42)
"#;
        
        // This tests the graph's ability to track cross-language refs
        // In practice, this requires semantic analysis of FFI boundaries
    }
    
    #[tokio::test]
    async fn test_ai_context_generation() {
        let (backend, _temp) = create_test_backend().await;
        
        let source = r#"
class Calculator:
    def add(self, a: int, b: int) -> int:
        return a + b
    
    def subtract(self, a: int, b: int) -> int:
        return a - b

def test_calculator():
    calc = Calculator()
    assert calc.add(2, 3) == 5
"#;
        
        {
            let mut graph = backend.graph.write().await;
            backend.parsers.get_parser("python").unwrap()
                .parse_source(source, Path::new("test.py"), &mut graph)
                .unwrap();
        }
        
        // Request AI context for the add method
        let response = backend.handle_get_ai_context(AIContextParams {
            uri: "file:///test.py".to_string(),
            position: Position { line: 2, character: 8 },
            context_type: "modify".to_string(),
            max_tokens: Some(2000),
        }).await.unwrap();
        
        // Should include the test as related context
        assert!(response.related_symbols.iter()
            .any(|s| s.relationship == "tests"));
    }
}
```

---

## 9. Deployment & Distribution

### 9.1 Packaging

```json
// package.json
{
    "name": "codegraph",
    "displayName": "CodeGraph",
    "description": "Cross-language code intelligence powered by graph analysis",
    "version": "0.1.0",
    "publisher": "codegraph",
    "license": "Apache-2.0",
    "repository": {
        "type": "git",
        "url": "https://github.com/codegraph/codegraph-vscode"
    },
    "engines": {
        "vscode": "^1.85.0"
    },
    "categories": [
        "Programming Languages",
        "Linters",
        "Other"
    ],
    "keywords": [
        "code intelligence",
        "cross-language",
        "code graph",
        "navigation",
        "AI"
    ],
    "activationEvents": [
        "onLanguage:python",
        "onLanguage:rust",
        "onLanguage:typescript",
        "onLanguage:javascript",
        "onLanguage:go"
    ],
    "main": "./out/extension.js",
    "contributes": {
        "commands": [
            {
                "command": "codegraph.showDependencyGraph",
                "title": "Show Dependency Graph",
                "category": "CodeGraph"
            },
            {
                "command": "codegraph.showCallGraph",
                "title": "Show Call Graph",
                "category": "CodeGraph"
            },
            {
                "command": "codegraph.analyzeImpact",
                "title": "Analyze Impact",
                "category": "CodeGraph"
            },
            {
                "command": "codegraph.showMetrics",
                "title": "Show Parser Metrics",
                "category": "CodeGraph"
            },
            {
                "command": "codegraph.openAIChat",
                "title": "Open AI Assistant",
                "category": "CodeGraph"
            }
        ],
        "configuration": {
            "title": "CodeGraph",
            "properties": {
                "codegraph.enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable CodeGraph extension"
                }
            }
        },
        "menus": {
            "editor/context": [
                {
                    "command": "codegraph.showCallGraph",
                    "group": "codegraph",
                    "when": "editorTextFocus"
                },
                {
                    "command": "codegraph.analyzeImpact",
                    "group": "codegraph",
                    "when": "editorTextFocus"
                }
            ]
        },
        "views": {
            "explorer": [
                {
                    "id": "codegraphSymbols",
                    "name": "CodeGraph Symbols",
                    "when": "codegraph.enabled"
                }
            ]
        }
    },
    "scripts": {
        "vscode:prepublish": "npm run compile && npm run build-server",
        "compile": "tsc -p ./",
        "watch": "tsc -watch -p ./",
        "build-server": "cd server && cargo build --release",
        "build-webview": "cd webview && npm run build",
        "package": "vsce package",
        "test": "npm run compile && node ./out/test/runTest.js"
    },
    "devDependencies": {
        "@types/node": "^20",
        "@types/vscode": "^1.85.0",
        "@vscode/vsce": "^2.22.0",
        "typescript": "^5.3.0"
    },
    "dependencies": {
        "vscode-languageclient": "^9.0.1"
    }
}
```

### 9.2 Binary Distribution

The extension ships pre-compiled LSP server binaries for major platforms:

```
codegraph-vscode/
├── package.json
├── out/                          # Compiled TypeScript
├── server/
│   ├── codegraph-lsp-linux-x64   # Linux binary
│   ├── codegraph-lsp-darwin-x64  # macOS Intel binary
│   ├── codegraph-lsp-darwin-arm64 # macOS Apple Silicon binary
│   └── codegraph-lsp-win32-x64.exe # Windows binary
└── webview/
    └── dist/                     # Bundled webview assets
```

**Build script:**
```bash
#!/bin/bash
# scripts/build-binaries.sh

set -e

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

for target in "${TARGETS[@]}"; do
    echo "Building for $target..."
    cross build --release --target "$target" -p codegraph-lsp
    
    # Copy to server directory with appropriate name
    case "$target" in
        *linux*)
            cp "target/$target/release/codegraph-lsp" "server/codegraph-lsp-linux-x64"
            ;;
        *darwin*x86*)
            cp "target/$target/release/codegraph-lsp" "server/codegraph-lsp-darwin-x64"
            ;;
        *darwin*aarch*)
            cp "target/$target/release/codegraph-lsp" "server/codegraph-lsp-darwin-arm64"
            ;;
        *windows*)
            cp "target/$target/release/codegraph-lsp.exe" "server/codegraph-lsp-win32-x64.exe"
            ;;
    esac
done

echo "All binaries built successfully!"
```

### 9.3 Platform Detection

```typescript
// src/server.ts
import * as os from 'os';
import * as path from 'path';

export function getServerPath(context: vscode.ExtensionContext): string {
    const platform = os.platform();
    const arch = os.arch();
    
    let binaryName: string;
    
    switch (platform) {
        case 'linux':
            binaryName = 'codegraph-lsp-linux-x64';
            break;
        case 'darwin':
            binaryName = arch === 'arm64' 
                ? 'codegraph-lsp-darwin-arm64'
                : 'codegraph-lsp-darwin-x64';
            break;
        case 'win32':
            binaryName = 'codegraph-lsp-win32-x64.exe';
            break;
        default:
            throw new Error(`Unsupported platform: ${platform}`);
    }
    
    return context.asAbsolutePath(path.join('server', binaryName));
}
```

---

## 10. Future Enhancements

### 10.1 Short-term
- Additional language support (Java, C#, C++)
- More sophisticated graph algorithms (PageRank for importance)
- Code smell detection based on graph patterns
- Refactoring suggestions (extract method, move function)
- Workspace symbol search

### 10.2 Long-term
- Cloud-based indexing for very large codebases
- Team collaboration features (shared annotations)
- Integration with CI/CD for architecture validation
- ML-based code similarity detection
- Architecture evolution tracking over time
- HSG v3 (Hierarchical Semantic Graph) integration

---

## 11. Success Metrics

- **Performance**: Index 100k LOC in <10s, respond to queries in <100ms
- **Accuracy**: >95% accuracy in cross-language references
- **Adoption**: >1000 active users within 6 months
- **AI Integration**: Demonstrable improvement in AI-assisted coding tasks
- **Reliability**: <1% crash rate, graceful degradation on parse errors

---

## Appendix A: LSP Protocol Reference

### Standard LSP Methods Implemented

| Method | Description |
|--------|-------------|
| `initialize` / `initialized` | Server initialization |
| `textDocument/didOpen` | File opened |
| `textDocument/didChange` | File changed |
| `textDocument/didSave` | File saved |
| `textDocument/didClose` | File closed |
| `textDocument/definition` | Go to definition |
| `textDocument/references` | Find all references |
| `textDocument/hover` | Hover information |
| `textDocument/documentSymbol` | Document symbols |
| `callHierarchy/prepareCallHierarchy` | Prepare call hierarchy |
| `callHierarchy/incomingCalls` | Incoming calls |
| `callHierarchy/outgoingCalls` | Outgoing calls |

### Custom Methods

| Method | Description |
|--------|-------------|
| `codegraph/getDependencyGraph` | Get module dependency graph |
| `codegraph/getCallGraph` | Get function call graph |
| `codegraph/getAIContext` | Get AI-optimized code context |
| `codegraph/analyzeImpact` | Analyze change impact |
| `codegraph/analyzeCoupling` | Analyze module coupling |
| `codegraph/findSimilarCode` | Find similar code patterns |
| `codegraph/getParserMetrics` | Get parser statistics |
| `codegraph/getNodeLocation` | Get location for a node ID |

---

## Appendix B: File Structure

```
codegraph-vscode/
├── Cargo.toml                    # Rust workspace root
├── package.json                  # VS Code extension manifest
├── tsconfig.json
├── README.md
├── CHANGELOG.md
├── LICENSE
│
├── server/                       # Rust LSP server
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # Entry point
│   │   ├── lib.rs               # Library exports
│   │   ├── backend.rs           # LSP handler implementation
│   │   ├── parser_registry.rs   # Multi-language parser management
│   │   ├── handlers/
│   │   │   ├── mod.rs
│   │   │   ├── navigation.rs    # goto_definition, references
│   │   │   ├── hierarchy.rs     # call hierarchy
│   │   │   ├── custom.rs        # custom graph requests
│   │   │   └── ai_context.rs    # AI context provider
│   │   ├── graph_adapter.rs     # Bridges LSP <-> CodeGraph
│   │   ├── cache.rs             # Query caching
│   │   ├── index.rs             # Symbol indexing
│   │   ├── watcher.rs           # File system watcher
│   │   └── error.rs             # Error types
│   └── tests/
│       ├── integration/
│       └── fixtures/
│
├── src/                          # TypeScript extension
│   ├── extension.ts             # Entry point
│   ├── client.ts                # LSP client setup
│   ├── config.ts                # Configuration handling
│   ├── commands/
│   │   ├── index.ts
│   │   ├── analyze.ts
│   │   ├── navigate.ts
│   │   └── visualize.ts
│   ├── views/
│   │   ├── dependencyTree.ts
│   │   ├── callGraph.ts
│   │   └── symbolIndex.ts
│   ├── webview/
│   │   └── graphPanel.ts
│   └── ai/
│       ├── contextProvider.ts
│       └── chatPanel.ts
│
├── webview/                      # React app for visualization
│   ├── package.json
│   ├── tsconfig.json
│   ├── webpack.config.js
│   └── src/
│       ├── index.tsx
│       ├── App.tsx
│       ├── components/
│       │   ├── GraphView.tsx
│       │   ├── DependencyMatrix.tsx
│       │   └── MetricsDashboard.tsx
│       └── styles/
│           └── main.css
│
├── scripts/
│   ├── build-binaries.sh
│   └── package.sh
│
└── docs/
    ├── ARCHITECTURE.md
    ├── API.md
    ├── DEVELOPMENT.md
    └── USER_GUIDE.md
```

---

## Appendix C: Parser Integration Reference

### CodeParser Trait (from codegraph-parser-api)

```rust
pub trait CodeParser: Send + Sync {
    /// Returns the language identifier (lowercase)
    fn language(&self) -> &str;

    /// Returns supported file extensions
    fn file_extensions(&self) -> &[&str];

    /// Parse a single file
    fn parse_file(&self, path: &Path, graph: &mut CodeGraph) -> Result<FileInfo, ParserError>;

    /// Parse source from string
    fn parse_source(&self, source: &str, file_path: &Path, graph: &mut CodeGraph) 
        -> Result<FileInfo, ParserError>;

    /// Check if this parser can handle a file
    fn can_parse(&self, path: &Path) -> bool;

    /// Get current configuration
    fn config(&self) -> &ParserConfig;

    /// Get parser metrics
    fn metrics(&self) -> ParserMetrics;

    /// Reset metrics
    fn reset_metrics(&mut self);

    /// Parse multiple files (default: sequential)
    fn parse_files(&self, paths: &[PathBuf], graph: &mut CodeGraph) 
        -> Result<ProjectInfo, ParserError>;

    /// Parse directory recursively
    fn parse_directory(&self, dir: &Path, graph: &mut CodeGraph) 
        -> Result<ProjectInfo, ParserError>;
}
```

### FileInfo Structure

```rust
pub struct FileInfo {
    pub file_path: PathBuf,
    pub file_id: NodeId,
    pub functions: Vec<NodeId>,
    pub classes: Vec<NodeId>,
    pub traits: Vec<NodeId>,
    pub imports: Vec<NodeId>,
    pub parse_time: Duration,
    pub line_count: usize,
    pub byte_count: usize,
}
```

### Available Parsers

| Crate | Language | Extensions |
|-------|----------|------------|
| `codegraph-python` | Python | `.py`, `.pyw` |
| `codegraph-rust` | Rust | `.rs` |
| `codegraph-typescript` | TypeScript/JavaScript | `.ts`, `.tsx`, `.js`, `.jsx` |
| `codegraph-go` | Go | `.go` |

---

**Document Version**: 2.0  
**Last Updated**: 2025-01-XX  
**Status**: Revised Draft  
**Changes from v1.0**:
- Added ParserRegistry component
- Aligned with actual CodeParser trait API
- Fixed method signatures throughout
- Added security considerations (CSP for webviews)
- Added parser metrics endpoint
- Updated file structure to match codebase
- Added configuration mapping
- Expanded testing strategy
- Updated to notify v6 API
