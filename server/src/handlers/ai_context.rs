//! AI Context Provider - Smart context selection for AI assistants.

use crate::backend::CodeGraphBackend;
use codegraph::{Direction, EdgeType, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{Position, Range, Url};

// ==========================================
// AI Context Request Types
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIContextParams {
    pub uri: String,
    pub position: Position,
    pub context_type: String, // "explain", "modify", "debug", "test"
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryContext {
    #[serde(rename = "type")]
    pub context_type: String,
    pub name: String,
    pub code: String,
    pub language: String,
    pub location: LocationInfo,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocationInfo {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedSymbol {
    pub name: String,
    pub relationship: String,
    pub code: String,
    pub location: LocationInfo,
    pub relevance_score: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub dep_type: String,
    pub code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageExample {
    pub code: String,
    pub location: LocationInfo,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureInfo {
    pub module: String,
    pub layer: Option<String>,
    pub neighbors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMetadata {
    pub total_tokens: usize,
    pub query_time: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIContextResponse {
    pub primary_context: PrimaryContext,
    pub related_symbols: Vec<RelatedSymbol>,
    pub dependencies: Vec<DependencyInfo>,
    pub usage_examples: Option<Vec<UsageExample>>,
    pub architecture: Option<ArchitectureInfo>,
    pub metadata: ContextMetadata,
}

// ==========================================
// Token Budget Management
// ==========================================

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

/// Estimate tokens in a code string (rough approximation: ~4 chars per token).
fn estimate_tokens(code: &str) -> usize {
    code.len() / 4
}

// ==========================================
// AI Context Handler Implementation
// ==========================================

impl CodeGraphBackend {
    pub async fn handle_get_ai_context(
        &self,
        params: AIContextParams,
    ) -> Result<AIContextResponse> {
        let start_time = std::time::Instant::now();

        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let graph = self.graph.read().await;
        let max_tokens = params.max_tokens.unwrap_or(4000);

        // Find node at position
        let node_id = self
            .find_node_at_position(&graph, &path, params.position)?
            .ok_or_else(|| tower_lsp::jsonrpc::Error::invalid_params("No symbol at position"))?;

        let node = graph
            .get_node(node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

        // Get primary context
        let primary_code = self
            .get_node_source_code(node_id)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "<source not available>".to_string());

        let name = node.properties.get_string("name").unwrap_or("").to_string();
        let node_type = format!("{}", node.node_type).to_lowercase();
        let language = node
            .properties
            .get_string("language")
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.parsers.language_for_path(&path).unwrap_or("unknown").to_string());

        let location = self.node_to_location_info(&graph, node_id)?;

        let primary_context = PrimaryContext {
            context_type: node_type,
            name: name.clone(),
            code: primary_code.clone(),
            language: language.clone(),
            location,
        };

        // Calculate remaining budget
        let mut budget = TokenBudget::new(max_tokens);
        budget.consume(estimate_tokens(&primary_code));

        // Get related symbols based on context type
        let related_symbols = match params.context_type.as_str() {
            "explain" => self.get_explanation_context(&graph, node_id, &mut budget).await,
            "modify" => self.get_modification_context(&graph, node_id, &mut budget).await,
            "debug" => self.get_debug_context(&graph, node_id, &mut budget).await,
            "test" => self.get_test_context(&graph, node_id, &mut budget).await,
            _ => Vec::new(),
        };

        // Get dependencies
        let dependencies = self.get_dependencies(&graph, node_id);

        // Get architecture info
        let architecture = self.get_architecture_info(&graph, node_id);

        let query_time = start_time.elapsed().as_millis() as u64;

        Ok(AIContextResponse {
            primary_context,
            related_symbols,
            dependencies,
            usage_examples: None, // Could be implemented
            architecture,
            metadata: ContextMetadata {
                total_tokens: budget.used,
                query_time,
            },
        })
    }

    fn node_to_location_info(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
    ) -> Result<LocationInfo> {
        let location = self
            .node_to_location(graph, node_id)
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

        Ok(LocationInfo {
            uri: location.uri.to_string(),
            range: location.range,
        })
    }

    /// Get context optimized for explaining code.
    async fn get_explanation_context(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
        budget: &mut TokenBudget,
    ) -> Vec<RelatedSymbol> {
        let mut context = Vec::new();

        // Priority 1: Direct dependencies (things this symbol uses)
        let outgoing = self.get_connected_edges(graph, node_id, Direction::Outgoing);
        for (_, target, _) in outgoing.iter().take(5) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(dep_node) = graph.get_node(*target) {
                if let Some(symbol) = self.create_related_symbol(graph, *target, &dep_node, "uses", 1.0, budget).await {
                    context.push(symbol);
                }
            }
        }

        // Priority 2: Direct callers (who uses this)
        let incoming = self.get_connected_edges(graph, node_id, Direction::Incoming);
        for (source, _, edge_type) in incoming.iter().filter(|(_, _, t)| *t == EdgeType::Calls).take(3) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(caller_node) = graph.get_node(*source) {
                if let Some(symbol) = self.create_related_symbol(graph, *source, &caller_node, "called_by", 0.8, budget).await {
                    context.push(symbol);
                }
            }
        }

        // Priority 3: Parent type (for methods)
        for (source, _, edge_type) in incoming.iter().filter(|(_, _, t)| *t == EdgeType::Extends) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(parent_node) = graph.get_node(*source) {
                if let Some(symbol) = self.create_related_symbol(graph, *source, &parent_node, "inherits", 0.9, budget).await {
                    context.push(symbol);
                }
            }
        }

        context
    }

    /// Get context optimized for modifying code.
    async fn get_modification_context(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
        budget: &mut TokenBudget,
    ) -> Vec<RelatedSymbol> {
        let mut context = Vec::new();

        // Priority 1: Tests for this symbol
        let incoming = self.get_connected_edges(graph, node_id, Direction::Incoming);
        for (source, _, edge_type) in incoming.iter().filter(|(_, _, t)| *t == EdgeType::Calls).take(5) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(caller_node) = graph.get_node(*source) {
                let name = caller_node.properties.get_string("name").unwrap_or("");
                if name.starts_with("test_") || name.ends_with("_test") {
                    if let Some(symbol) = self.create_related_symbol(graph, *source, &caller_node, "tests", 1.0, budget).await {
                        context.push(symbol);
                    }
                }
            }
        }

        // Priority 2: All direct callers
        for (source, _, edge_type) in incoming.iter().filter(|(_, _, t)| *t == EdgeType::Calls).take(5) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(caller_node) = graph.get_node(*source) {
                let name = caller_node.properties.get_string("name").unwrap_or("");
                if !name.starts_with("test_") && !name.ends_with("_test") {
                    if let Some(symbol) = self.create_related_symbol(graph, *source, &caller_node, "called_by", 0.9, budget).await {
                        context.push(symbol);
                    }
                }
            }
        }

        context
    }

    /// Get context optimized for debugging.
    async fn get_debug_context(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
        budget: &mut TokenBudget,
    ) -> Vec<RelatedSymbol> {
        let mut context = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(node_id);

        // Get call chain up to entry point
        let mut current = node_id;
        let mut depth = 0;

        while depth < 5 && budget.has_budget() {
            let incoming = self.get_connected_edges(graph, current, Direction::Incoming);
            let caller = incoming
                .iter()
                .filter(|(_, _, t)| *t == EdgeType::Calls)
                .find(|(source, _, _)| !visited.contains(source));

            if let Some((source, _, _)) = caller {
                visited.insert(*source);

                if let Ok(caller_node) = graph.get_node(*source) {
                    let relevance = 1.0 - (depth as f64 * 0.1);
                    let relationship = format!("call_chain_depth_{}", depth);

                    if let Some(symbol) = self.create_related_symbol(graph, *source, &caller_node, &relationship, relevance, budget).await {
                        context.push(symbol);
                    }
                }

                current = *source;
                depth += 1;
            } else {
                break;
            }
        }

        // Add data dependencies
        let outgoing = self.get_connected_edges(graph, node_id, Direction::Outgoing);
        for (_, target, _) in outgoing.iter().take(3) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(dep_node) = graph.get_node(*target) {
                if let Some(symbol) = self.create_related_symbol(graph, *target, &dep_node, "data_flow", 0.8, budget).await {
                    context.push(symbol);
                }
            }
        }

        context
    }

    /// Get context optimized for writing tests.
    async fn get_test_context(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
        budget: &mut TokenBudget,
    ) -> Vec<RelatedSymbol> {
        let mut context = Vec::new();

        // Find existing tests that might be similar
        let incoming = self.get_connected_edges(graph, node_id, Direction::Incoming);
        for (source, _, edge_type) in incoming.iter().filter(|(_, _, t)| *t == EdgeType::Calls).take(3) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(caller_node) = graph.get_node(*source) {
                let name = caller_node.properties.get_string("name").unwrap_or("");
                if name.starts_with("test_") || name.ends_with("_test") {
                    if let Some(symbol) = self.create_related_symbol(graph, *source, &caller_node, "example_test", 0.9, budget).await {
                        context.push(symbol);
                    }
                }
            }
        }

        // Add dependencies that might need mocking
        let outgoing = self.get_connected_edges(graph, node_id, Direction::Outgoing);
        for (_, target, _) in outgoing.iter().take(3) {
            if !budget.has_budget() {
                break;
            }

            if let Ok(dep_node) = graph.get_node(*target) {
                if let Some(symbol) = self.create_related_symbol(graph, *target, &dep_node, "dependency_to_mock", 0.7, budget).await {
                    context.push(symbol);
                }
            }
        }

        context
    }

    /// Create a RelatedSymbol from a node.
    async fn create_related_symbol(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
        node: &codegraph::Node,
        relationship: &str,
        relevance: f64,
        budget: &mut TokenBudget,
    ) -> Option<RelatedSymbol> {
        let code = self.get_node_source_code(node_id).await.ok()??;
        let tokens = estimate_tokens(&code);

        if !budget.consume(tokens) {
            return None;
        }

        let name = node.properties.get_string("name").unwrap_or("").to_string();
        let location = self.node_to_location_info(graph, node_id).ok()?;

        Some(RelatedSymbol {
            name,
            relationship: relationship.to_string(),
            code,
            location,
            relevance_score: relevance,
        })
    }

    /// Get dependencies for a node.
    fn get_dependencies(&self, graph: &codegraph::CodeGraph, node_id: NodeId) -> Vec<DependencyInfo> {
        let mut deps = Vec::new();

        let outgoing = self.get_connected_edges(graph, node_id, Direction::Outgoing);

        for (_, target, edge_type) in outgoing.iter().filter(|(_, _, t)| *t == EdgeType::Imports).take(10) {
            if let Ok(dep_node) = graph.get_node(*target) {
                let name = dep_node.properties.get_string("name").unwrap_or("").to_string();
                deps.push(DependencyInfo {
                    name,
                    dep_type: "import".to_string(),
                    code: None,
                });
            }
        }

        deps
    }

    /// Get architecture information for a node.
    fn get_architecture_info(
        &self,
        graph: &codegraph::CodeGraph,
        node_id: NodeId,
    ) -> Option<ArchitectureInfo> {
        let node = graph.get_node(node_id).ok()?;
        let path = node.properties.get_string("path")?;

        // Extract module name from path
        let module = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Get neighbor modules
        let mut neighbors = HashSet::new();

        let outgoing = self.get_connected_edges(graph, node_id, Direction::Outgoing);
        let incoming = self.get_connected_edges(graph, node_id, Direction::Incoming);

        for (source, target, _) in outgoing.iter().chain(incoming.iter()) {
            let other_id = if *source == node_id { *target } else { *source };

            if let Ok(other_node) = graph.get_node(other_id) {
                if let Some(other_path) = other_node.properties.get_string("path") {
                    if let Some(other_module) = std::path::Path::new(other_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                    {
                        if other_module != module {
                            neighbors.insert(other_module.to_string());
                        }
                    }
                }
            }
        }

        Some(ArchitectureInfo {
            module,
            layer: None,
            neighbors: neighbors.into_iter().collect(),
        })
    }
}
