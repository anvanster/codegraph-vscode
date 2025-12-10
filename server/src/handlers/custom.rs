//! Custom LSP request handlers for graph-based features.

use crate::backend::CodeGraphBackend;
use codegraph::{CodeGraph, Direction, EdgeType, Node, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{Position, Range, Url};

/// Helper to get line position from node properties.
/// Supports both property name conventions (line_start or start_line).
fn get_line_start(node: &Node) -> u32 {
    node.properties.get_int("line_start")
        .or_else(|| node.properties.get_int("start_line"))
        .map(|v| v as u32)
        .unwrap_or(1)
}

fn get_line_end(node: &Node, default: u32) -> u32 {
    node.properties.get_int("line_end")
        .or_else(|| node.properties.get_int("end_line"))
        .map(|v| v as u32)
        .unwrap_or(default)
}

fn get_col_start(node: &Node) -> u32 {
    node.properties.get_int("col_start")
        .or_else(|| node.properties.get_int("start_col"))
        .map(|v| v as u32)
        .unwrap_or(0)
}

fn get_col_end(node: &Node) -> u32 {
    node.properties.get_int("col_end")
        .or_else(|| node.properties.get_int("end_col"))
        .map(|v| v as u32)
        .unwrap_or(0)
}

// ==========================================
// Dependency Graph Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraphParams {
    pub uri: String,
    pub depth: Option<usize>,
    pub include_external: Option<bool>,
    pub direction: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyNode {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub language: String,
    pub uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraphResponse {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

impl CodeGraphBackend {
    pub async fn handle_get_dependency_graph(
        &self,
        params: DependencyGraphParams,
    ) -> Result<DependencyGraphResponse> {
        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let graph = self.graph.read().await;
        let depth = params.depth.unwrap_or(3);
        let include_external = params.include_external.unwrap_or(false);

        // Find the file node
        let path_str = path.to_string_lossy().to_string();

        let file_nodes = graph
            .query()
            .property("path", path_str.clone())
            .execute()
            .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

        if file_nodes.is_empty() {
            return Ok(DependencyGraphResponse {
                nodes: Vec::new(),
                edges: Vec::new(),
            });
        }

        let start_node = file_nodes[0];

        // BFS to collect dependency subgraph
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        queue.push_back((start_node, 0));
        visited.insert(start_node);

        while let Some((node_id, current_depth)) = queue.pop_front() {
            if current_depth > depth {
                continue;
            }

            if let Ok(node) = graph.get_node(node_id) {
                // Skip external if not requested
                let is_external = node
                    .properties
                    .get_string("external")
                    .map(|v| v == "true")
                    .unwrap_or(false);

                if is_external && !include_external {
                    continue;
                }

                let name = node.properties.get_string("name").unwrap_or("").to_string();
                let node_type = format!("{:?}", node.node_type).to_lowercase();
                let language = node
                    .properties
                    .get_string("language")
                    .unwrap_or("unknown")
                    .to_string();
                let node_path = node.properties.get_string("path").unwrap_or("").to_string();

                nodes.push(DependencyNode {
                    id: node_id.to_string(),
                    label: name,
                    node_type,
                    language,
                    uri: node_path,
                });

                // Get import edges based on direction
                let dir_param = params.direction.as_deref().unwrap_or("both");

                // Collect edges based on direction
                let collected_edges = Self::collect_edges_for_direction(
                    &graph,
                    node_id,
                    dir_param,
                    "importedBy",
                    "imports",
                );

                for (source_id, target_id, edge_type) in collected_edges {
                    if edge_type == EdgeType::Imports {
                        edges.push(DependencyEdge {
                            from: source_id.to_string(),
                            to: target_id.to_string(),
                            edge_type: "import".to_string(),
                        });

                        let next_node = if source_id == node_id {
                            target_id
                        } else {
                            source_id
                        };

                        if !visited.contains(&next_node) {
                            visited.insert(next_node);
                            queue.push_back((next_node, current_depth + 1));
                        }
                    }
                }
            }
        }

        Ok(DependencyGraphResponse { nodes, edges })
    }

    /// Helper function to collect edges based on direction parameter.
    fn collect_edges_for_direction(
        graph: &CodeGraph,
        node_id: NodeId,
        direction: &str,
        skip_outgoing: &str,
        skip_incoming: &str,
    ) -> Vec<(NodeId, NodeId, EdgeType)> {
        let mut edges = Vec::new();

        // Get outgoing edges if not skipped
        if direction != skip_outgoing {
            if let Ok(neighbors) = graph.get_neighbors(node_id, Direction::Outgoing) {
                for neighbor_id in neighbors {
                    if let Ok(edge_ids) = graph.get_edges_between(node_id, neighbor_id) {
                        for edge_id in edge_ids {
                            if let Ok(edge) = graph.get_edge(edge_id) {
                                edges.push((edge.source_id, edge.target_id, edge.edge_type));
                            }
                        }
                    }
                }
            }
        }

        // Get incoming edges if not skipped
        if direction != skip_incoming {
            if let Ok(neighbors) = graph.get_neighbors(node_id, Direction::Incoming) {
                for neighbor_id in neighbors {
                    if let Ok(edge_ids) = graph.get_edges_between(neighbor_id, node_id) {
                        for edge_id in edge_ids {
                            if let Ok(edge) = graph.get_edge(edge_id) {
                                edges.push((edge.source_id, edge.target_id, edge.edge_type));
                            }
                        }
                    }
                }
            }
        }

        edges
    }

    /// Helper function to get incoming edges for a node.
    fn get_incoming_edges(graph: &CodeGraph, node_id: NodeId) -> Vec<(NodeId, NodeId, EdgeType)> {
        let mut edges = Vec::new();
        if let Ok(neighbors) = graph.get_neighbors(node_id, Direction::Incoming) {
            for neighbor_id in neighbors {
                if let Ok(edge_ids) = graph.get_edges_between(neighbor_id, node_id) {
                    for edge_id in edge_ids {
                        if let Ok(edge) = graph.get_edge(edge_id) {
                            edges.push((edge.source_id, edge.target_id, edge.edge_type));
                        }
                    }
                }
            }
        }
        edges
    }
}

// ==========================================
// Call Graph Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallGraphParams {
    pub uri: String,
    pub position: Position,
    pub direction: Option<String>,
    pub depth: Option<usize>,
    pub include_external: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionNode {
    pub id: String,
    pub name: String,
    pub signature: String,
    pub uri: String,
    pub range: Range,
    pub language: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallEdge {
    pub from: String,
    pub to: String,
    pub call_sites: Vec<Range>,
    pub is_recursive: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallGraphResponse {
    pub root: Option<FunctionNode>,
    pub nodes: Vec<FunctionNode>,
    pub edges: Vec<CallEdge>,
}

impl CodeGraphBackend {
    pub async fn handle_get_call_graph(&self, params: CallGraphParams) -> Result<CallGraphResponse> {
        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let graph = self.graph.read().await;
        let depth = params.depth.unwrap_or(3);
        let position = params.position;

        // Find node at position
        let node_id = match self.find_node_at_position(&graph, &path, position)? {
            Some(id) => id,
            None => {
                return Ok(CallGraphResponse {
                    root: None,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                })
            }
        };

        // BFS to collect call graph
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        queue.push_back((node_id, 0));
        visited.insert(node_id);

        let mut root: Option<FunctionNode> = None;

        while let Some((current_id, current_depth)) = queue.pop_front() {
            if current_depth > depth {
                continue;
            }

            if let Ok(node) = graph.get_node(current_id) {
                let name = node.properties.get_string("name").unwrap_or("").to_string();
                let signature = node.properties.get_string("signature").unwrap_or("").to_string();
                let language = node
                    .properties
                    .get_string("language")
                    .unwrap_or("unknown")
                    .to_string();
                let node_path = node.properties.get_string("path").unwrap_or("").to_string();

                let start_line = get_line_start(&node).saturating_sub(1);
                let start_col = get_col_start(&node);
                let end_line = get_line_end(&node, start_line + 2).saturating_sub(1);
                let end_col = get_col_end(&node);

                let func_node = FunctionNode {
                    id: current_id.to_string(),
                    name,
                    signature,
                    uri: node_path,
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
                    language,
                };

                if current_id == node_id {
                    root = Some(func_node.clone());
                }

                nodes.push(func_node);

                // Get call edges based on direction
                let dir_param = params.direction.as_deref().unwrap_or("both");

                // Collect edges based on direction
                let collected_edges = Self::collect_edges_for_direction(
                    &graph,
                    current_id,
                    dir_param,
                    "callers",
                    "callees",
                );

                for (source_id, target_id, edge_type) in collected_edges {
                    if edge_type == EdgeType::Calls {
                        let is_recursive = source_id == target_id;

                        edges.push(CallEdge {
                            from: source_id.to_string(),
                            to: target_id.to_string(),
                            call_sites: Vec::new(), // Could be populated from edge properties
                            is_recursive,
                        });

                        let next_node = if source_id == current_id {
                            target_id
                        } else {
                            source_id
                        };

                        if !visited.contains(&next_node) {
                            visited.insert(next_node);
                            queue.push_back((next_node, current_depth + 1));
                        }
                    }
                }
            }
        }

        Ok(CallGraphResponse { root, nodes, edges })
    }
}

// ==========================================
// Impact Analysis Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysisParams {
    pub uri: String,
    pub position: Position,
    pub analysis_type: String, // "modify", "delete", "rename"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectImpact {
    pub uri: String,
    pub range: Range,
    #[serde(rename = "type")]
    pub impact_type: String,
    pub severity: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndirectImpact {
    pub uri: String,
    pub path: Vec<String>,
    pub severity: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedTest {
    pub uri: String,
    pub test_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactSummary {
    pub files_affected: usize,
    pub breaking_changes: usize,
    pub warnings: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysisResponse {
    pub direct_impact: Vec<DirectImpact>,
    pub indirect_impact: Vec<IndirectImpact>,
    pub affected_tests: Vec<AffectedTest>,
    pub summary: ImpactSummary,
}

impl CodeGraphBackend {
    pub async fn handle_analyze_impact(
        &self,
        params: ImpactAnalysisParams,
    ) -> Result<ImpactAnalysisResponse> {
        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let graph = self.graph.read().await;

        // Find node at position
        let node_id = match self.find_node_at_position(&graph, &path, params.position)? {
            Some(id) => id,
            None => {
                return Ok(ImpactAnalysisResponse {
                    direct_impact: Vec::new(),
                    indirect_impact: Vec::new(),
                    affected_tests: Vec::new(),
                    summary: ImpactSummary {
                        files_affected: 0,
                        breaking_changes: 0,
                        warnings: 0,
                    },
                })
            }
        };

        let mut direct_impact = Vec::new();
        let mut indirect_impact = Vec::new();
        let mut affected_tests = Vec::new();
        let mut affected_files = HashSet::new();

        // Find direct references
        let incoming_edges = Self::get_incoming_edges(&graph, node_id);

        for (source_id, _target_id, edge_type) in incoming_edges {
            if let Ok(ref_node) = graph.get_node(source_id) {
                let ref_path = ref_node.properties.get_string("path").unwrap_or("").to_string();
                let ref_name = ref_node.properties.get_string("name").unwrap_or("").to_string();

                affected_files.insert(ref_path.clone());

                let start_line = get_line_start(&ref_node).saturating_sub(1);
                let start_col = get_col_start(&ref_node);
                let end_line = get_line_end(&ref_node, start_line + 2).saturating_sub(1);
                let end_col = get_col_end(&ref_node);

                let impact_type = match edge_type {
                    EdgeType::Calls => "caller",
                    EdgeType::References => "reference",
                    EdgeType::Extends => "subclass",
                    EdgeType::Implements => "implementation",
                    _ => "reference",
                };

                let severity = match params.analysis_type.as_str() {
                    "delete" => "breaking",
                    "rename" => "breaking",
                    "modify" => "warning",
                    _ => "info",
                };

                // Check if it's a test
                let is_test = ref_name.starts_with("test_")
                    || ref_name.ends_with("_test")
                    || ref_path.contains("test");

                if is_test {
                    affected_tests.push(AffectedTest {
                        uri: ref_path.clone(),
                        test_name: ref_name,
                    });
                }

                direct_impact.push(DirectImpact {
                    uri: ref_path,
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
                    impact_type: impact_type.to_string(),
                    severity: severity.to_string(),
                });
            }
        }

        // Find indirect impact (transitive dependencies)
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(NodeId, Vec<String>)> = VecDeque::new();

        for impact in &direct_impact {
            // Get node IDs for direct impacts and trace further
            if let Ok(nodes) = graph.query().property("path", impact.uri.clone()).execute() {
                for n_id in nodes {
                    if !visited.contains(&n_id) && n_id != node_id {
                        visited.insert(n_id);
                        queue.push_back((n_id, vec![impact.uri.clone()]));
                    }
                }
            }
        }

        while let Some((current_id, path_chain)) = queue.pop_front() {
            if path_chain.len() >= 3 {
                continue; // Limit depth
            }

            let incoming_edges = Self::get_incoming_edges(&graph, current_id);
            for (source_id, _target_id, _edge_type) in incoming_edges {
                if source_id != node_id && !visited.contains(&source_id) {
                    if let Ok(ref_node) = graph.get_node(source_id) {
                        let ref_path = ref_node.properties.get_string("path").unwrap_or("").to_string();

                        if !affected_files.contains(&ref_path) {
                            let mut new_path = path_chain.clone();
                            new_path.push(ref_path.clone());

                            indirect_impact.push(IndirectImpact {
                                uri: ref_path,
                                path: new_path.clone(),
                                severity: "warning".to_string(),
                            });

                            visited.insert(source_id);
                            queue.push_back((source_id, new_path));
                        }
                    }
                }
            }
        }

        let breaking_count = direct_impact
            .iter()
            .filter(|i| i.severity == "breaking")
            .count();
        let warning_count = direct_impact
            .iter()
            .filter(|i| i.severity == "warning")
            .count()
            + indirect_impact.len();

        Ok(ImpactAnalysisResponse {
            direct_impact,
            indirect_impact,
            affected_tests,
            summary: ImpactSummary {
                files_affected: affected_files.len(),
                breaking_changes: breaking_count,
                warnings: warning_count,
            },
        })
    }
}

// ==========================================
// Parser Metrics Request
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserMetricsParams {
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserMetric {
    pub language: String,
    pub files_attempted: usize,
    pub files_succeeded: usize,
    pub files_failed: usize,
    pub total_entities: usize,
    pub total_relationships: usize,
    pub total_parse_time_ms: u64,
    pub avg_parse_time_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsTotals {
    pub files_attempted: usize,
    pub files_succeeded: usize,
    pub files_failed: usize,
    pub total_entities: usize,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserMetricsResponse {
    pub metrics: Vec<ParserMetric>,
    pub totals: MetricsTotals,
}

impl CodeGraphBackend {
    pub async fn handle_get_parser_metrics(
        &self,
        params: ParserMetricsParams,
    ) -> Result<ParserMetricsResponse> {
        let all_metrics = self.parsers.all_metrics();

        let metrics: Vec<ParserMetric> = all_metrics
            .into_iter()
            .filter(|(lang, _)| params.language.as_ref().map_or(true, |l| l == *lang))
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

        let totals = MetricsTotals {
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
}
