//! Code Metrics Handler - Complexity and quality analysis for AI assistants.

use crate::backend::CodeGraphBackend;
use crate::handlers::ai_context::LocationInfo;
use codegraph::{Direction, EdgeType, NodeId, NodeType};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::Url;

// ==========================================
// Complexity Analysis Types
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityParams {
    pub uri: String,
    /// Specific line to analyze (optional, analyzes whole file if not provided)
    pub line: Option<u32>,
    /// Complexity threshold for recommendations (default: 10)
    pub threshold: Option<u32>,
    /// Include detailed metrics breakdown
    pub include_metrics: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityResponse {
    pub functions: Vec<FunctionComplexity>,
    pub file_summary: FileSummary,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionComplexity {
    pub name: String,
    pub complexity: u32,
    pub grade: char,
    pub location: LocationInfo,
    pub details: ComplexityDetails,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityDetails {
    /// Number of if/else/switch branches
    pub branches: u32,
    /// Number of for/while/loop constructs
    pub loops: u32,
    /// Number of && / || conditions
    pub conditions: u32,
    /// Maximum nesting depth
    pub nesting_depth: u32,
    /// Lines of code in the function
    pub lines_of_code: u32,
}

// LocationInfo is imported from ai_context module

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub total_functions: u32,
    pub average_complexity: f64,
    pub max_complexity: u32,
    pub functions_above_threshold: u32,
    pub overall_grade: char,
}

// ==========================================
// Dead Code Detection Types
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedCodeParams {
    /// Specific file to analyze (optional)
    pub uri: Option<String>,
    /// Scope of analysis: "file", "module", or "workspace"
    pub scope: String,
    /// Include test files in analysis
    pub include_tests: Option<bool>,
    /// Minimum confidence threshold (0.0-1.0)
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedCodeResponse {
    pub unused_items: Vec<UnusedItem>,
    pub summary: UnusedSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedItem {
    /// Type of unused item: "function", "class", "import", "export", "variable"
    pub item_type: String,
    pub name: String,
    pub location: LocationInfo,
    /// Confidence that this is truly unused (0.0-1.0)
    pub confidence: f64,
    /// Explanation of why this is considered unused
    pub reason: String,
    /// Whether it's safe to remove without breaking external consumers
    pub safe_to_remove: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedSummary {
    pub total_items: u32,
    pub by_type: UnusedByType,
    pub safe_deletions: u32,
    pub estimated_lines_removable: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedByType {
    pub functions: u32,
    pub classes: u32,
    pub imports: u32,
    pub variables: u32,
}

// ==========================================
// Coupling Analysis Types
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CouplingParams {
    pub uri: String,
    /// Include external dependencies in analysis
    pub include_external: Option<bool>,
    /// Depth of dependency analysis
    pub depth: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CouplingResponse {
    pub coupling: CouplingMetrics,
    pub cohesion: CohesionMetrics,
    pub violations: Vec<ArchViolation>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CouplingMetrics {
    /// Incoming dependencies (who depends on this module)
    pub afferent: u32,
    /// Outgoing dependencies (what this module depends on)
    pub efferent: u32,
    /// Instability: Ce / (Ca + Ce), 0 = stable, 1 = unstable
    pub instability: f64,
    /// List of modules that depend on this one
    pub dependents: Vec<String>,
    /// List of modules this one depends on
    pub dependencies: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CohesionMetrics {
    /// Cohesion score (0.0-1.0, higher is better)
    pub score: f64,
    /// Type of cohesion detected
    pub cohesion_type: String,
    /// Percentage of internal references vs external
    pub internal_reference_ratio: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchViolation {
    pub violation_type: String,
    pub severity: String,
    pub description: String,
    pub suggestion: String,
}

// ==========================================
// Complexity Calculation
// ==========================================

impl CodeGraphBackend {
    /// Calculate cyclomatic complexity grade from score
    /// Uses same thresholds as upstream codegraph-parser-api ComplexityMetrics::grade()
    fn complexity_grade(complexity: u32) -> char {
        match complexity {
            1..=5 => 'A',   // Simple, low risk
            6..=10 => 'B',  // Moderate complexity
            11..=20 => 'C', // Complex, moderate risk
            21..=50 => 'D', // Very complex, high risk
            _ => 'F',       // Untestable, very high risk
        }
    }

    /// Calculate overall file grade from average complexity
    fn file_grade(avg_complexity: f64) -> char {
        match avg_complexity as u32 {
            0..=5 => 'A',
            6..=10 => 'B',
            11..=15 => 'C',
            16..=25 => 'D',
            _ => 'F',
        }
    }

    /// Get complexity details from a function node
    /// Primary: Uses AST-based complexity from upstream codegraph parsers (v0.3.0+)
    /// Fallback: Returns base complexity of 1 if no upstream data available
    fn get_complexity_from_node(node: &codegraph::Node) -> (u32, ComplexityDetails, char) {
        let start = node.properties.get_int("line_start").unwrap_or(0) as u32;
        let end = node.properties.get_int("line_end").unwrap_or(0) as u32;
        let lines_of_code = end.saturating_sub(start) + 1;

        if let Some(parsed_complexity) = node.properties.get_int("complexity") {
            // Use upstream complexity from codegraph parsers (AST-based)
            let complexity = parsed_complexity as u32;
            let grade = node
                .properties
                .get_string("complexity_grade")
                .and_then(|s| s.chars().next())
                .unwrap_or_else(|| Self::complexity_grade(complexity));
            let details = ComplexityDetails {
                branches: node.properties.get_int("complexity_branches").unwrap_or(0) as u32,
                loops: node.properties.get_int("complexity_loops").unwrap_or(0) as u32,
                conditions: node
                    .properties
                    .get_int("complexity_logical_ops")
                    .unwrap_or(0) as u32,
                nesting_depth: node.properties.get_int("complexity_nesting").unwrap_or(0) as u32,
                lines_of_code,
            };
            (complexity, details, grade)
        } else {
            // Fallback: Base complexity of 1 (no control flow analysis available)
            // This happens for files indexed with older parser versions
            let details = ComplexityDetails {
                branches: 0,
                loops: 0,
                conditions: 0,
                nesting_depth: 0,
                lines_of_code,
            };
            (1, details, 'A')
        }
    }

    /// Analyze complexity for a file
    pub async fn handle_analyze_complexity(
        &self,
        params: ComplexityParams,
    ) -> Result<ComplexityResponse> {
        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let threshold = params.threshold.unwrap_or(10);
        let graph = self.graph.read().await;

        // Get all function nodes in this file
        let file_symbols = self.symbol_index.get_file_symbols(&path);
        let mut functions: Vec<FunctionComplexity> = Vec::new();

        for node_id in file_symbols {
            if let Ok(node) = graph.get_node(node_id) {
                // Only analyze functions
                if node.node_type != NodeType::Function {
                    continue;
                }

                // If a specific line is requested, filter to that function
                if let Some(target_line) = params.line {
                    let start_line = node.properties.get_int("line_start").unwrap_or(0) as u32;
                    let end_line = node.properties.get_int("line_end").unwrap_or(0) as u32;
                    if target_line < start_line || target_line > end_line {
                        continue;
                    }
                }

                let name = node
                    .properties
                    .get_string("name")
                    .unwrap_or("anonymous")
                    .to_string();

                // Get complexity from upstream codegraph parsers (AST-based)
                let (complexity, details, grade) = Self::get_complexity_from_node(&node);

                let location = self
                    .node_to_location(&graph, node_id)
                    .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;

                functions.push(FunctionComplexity {
                    name,
                    complexity,
                    grade,
                    location: LocationInfo {
                        uri: location.uri.to_string(),
                        range: location.range,
                    },
                    details,
                });
            }
        }

        // Sort by complexity descending
        functions.sort_by(|a, b| b.complexity.cmp(&a.complexity));

        // Calculate summary
        let total_functions = functions.len() as u32;
        let total_complexity: u32 = functions.iter().map(|f| f.complexity).sum();
        let average_complexity = if total_functions > 0 {
            total_complexity as f64 / total_functions as f64
        } else {
            0.0
        };
        let max_complexity = functions.iter().map(|f| f.complexity).max().unwrap_or(0);
        let functions_above_threshold = functions
            .iter()
            .filter(|f| f.complexity > threshold)
            .count() as u32;

        // Generate recommendations
        let mut recommendations = Vec::new();

        for func in functions.iter().filter(|f| f.complexity > threshold) {
            recommendations.push(format!(
                "Consider refactoring '{}' (complexity: {}, grade: {}). Break into smaller functions.",
                func.name, func.complexity, func.grade
            ));
        }

        if average_complexity > 15.0 {
            recommendations.push(
                "File has high average complexity. Consider splitting into multiple modules."
                    .to_string(),
            );
        }

        let high_nesting: Vec<_> = functions
            .iter()
            .filter(|f| f.details.nesting_depth > 4)
            .collect();
        if !high_nesting.is_empty() {
            recommendations.push(format!(
                "{} function(s) have deep nesting (>4 levels). Use early returns or extract methods.",
                high_nesting.len()
            ));
        }

        Ok(ComplexityResponse {
            functions,
            file_summary: FileSummary {
                total_functions,
                average_complexity,
                max_complexity,
                functions_above_threshold,
                overall_grade: Self::file_grade(average_complexity),
            },
            recommendations,
        })
    }

    /// Find unused code in the codebase
    pub async fn handle_find_unused_code(
        &self,
        params: UnusedCodeParams,
    ) -> Result<UnusedCodeResponse> {
        let graph = self.graph.read().await;
        let min_confidence = params.confidence.unwrap_or(0.7);
        let include_tests = params.include_tests.unwrap_or(false);

        let mut unused_items: Vec<UnusedItem> = Vec::new();
        let mut functions_count = 0u32;
        let mut classes_count = 0u32;
        let imports_count = 0u32;
        let variables_count = 0u32;
        let mut total_lines = 0u32;

        // Get nodes based on scope
        let node_ids: Vec<NodeId> = if let Some(uri) = &params.uri {
            let uri_parsed = Url::parse(uri)
                .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;
            let path = uri_parsed
                .to_file_path()
                .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;
            self.symbol_index.get_file_symbols(&path)
        } else {
            // Workspace scope - get all function nodes
            graph
                .query()
                .node_type(NodeType::Function)
                .execute()
                .unwrap_or_default()
        };

        for node_id in node_ids {
            if let Ok(node) = graph.get_node(node_id) {
                // Skip non-function/class nodes for now
                if node.node_type != NodeType::Function && node.node_type != NodeType::Class {
                    continue;
                }

                let name = node.properties.get_string("name").unwrap_or("").to_string();

                // Skip test functions unless requested
                if !include_tests
                    && (name.starts_with("test_")
                        || name.ends_with("_test")
                        || name.contains("Test"))
                {
                    continue;
                }

                // Check if node has any incoming calls
                let incoming = self.get_connected_edges(&graph, node_id, Direction::Incoming);
                let has_callers = incoming
                    .iter()
                    .any(|(_, _, edge_type)| *edge_type == EdgeType::Calls);

                if !has_callers {
                    // Determine if this might be exported or an entry point
                    let is_exported = node.properties.get_bool("exported").unwrap_or(false)
                        || node
                            .properties
                            .get_string("visibility")
                            .map(|v| v == "public")
                            .unwrap_or(false);

                    let is_entry = Self::is_entry_point(&name);
                    let is_handler = name.contains("handle")
                        || name.contains("Handler")
                        || name.starts_with("on");
                    let is_lifecycle = ["init", "setup", "teardown", "cleanup", "main", "run"]
                        .iter()
                        .any(|k| name.to_lowercase().contains(k));

                    // Calculate confidence
                    let confidence = if is_exported {
                        0.4 // Low confidence - might be used externally
                    } else if is_entry || is_handler || is_lifecycle {
                        0.3 // Very low - likely an entry point
                    } else {
                        0.9 // High confidence - truly unused
                    };

                    if confidence >= min_confidence {
                        let item_type = match node.node_type {
                            NodeType::Function => "function",
                            NodeType::Class => "class",
                            _ => "unknown",
                        };

                        let reason = if is_exported {
                            "Exported but no internal callers found"
                        } else if is_entry {
                            "Possible entry point with no callers"
                        } else {
                            "No callers found in codebase"
                        };

                        let start_line = node.properties.get_int("line_start").unwrap_or(0) as u32;
                        let end_line = node.properties.get_int("line_end").unwrap_or(0) as u32;
                        let lines = end_line.saturating_sub(start_line) + 1;

                        if let Ok(location) = self.node_to_location(&graph, node_id) {
                            unused_items.push(UnusedItem {
                                item_type: item_type.to_string(),
                                name: name.clone(),
                                location: LocationInfo {
                                    uri: location.uri.to_string(),
                                    range: location.range,
                                },
                                confidence,
                                reason: reason.to_string(),
                                safe_to_remove: !is_exported && !is_entry && confidence > 0.8,
                            });

                            match node.node_type {
                                NodeType::Function => functions_count += 1,
                                NodeType::Class => classes_count += 1,
                                _ => {}
                            }

                            if confidence > 0.8 {
                                total_lines += lines;
                            }
                        }
                    }
                }
            }
        }

        // Sort by confidence descending
        unused_items.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let safe_deletions = unused_items.iter().filter(|i| i.safe_to_remove).count() as u32;

        Ok(UnusedCodeResponse {
            unused_items,
            summary: UnusedSummary {
                total_items: functions_count + classes_count + imports_count + variables_count,
                by_type: UnusedByType {
                    functions: functions_count,
                    classes: classes_count,
                    imports: imports_count,
                    variables: variables_count,
                },
                safe_deletions,
                estimated_lines_removable: total_lines,
            },
        })
    }

    /// Check if a function name suggests it's an entry point
    fn is_entry_point(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower == "main"
            || lower == "run"
            || lower == "start"
            || lower == "init"
            || lower == "setup"
            || lower.starts_with("export")
            || lower.ends_with("handler")
            || lower.ends_with("listener")
            || lower.ends_with("callback")
    }

    /// Analyze module coupling and cohesion
    pub async fn handle_analyze_coupling(
        &self,
        params: CouplingParams,
    ) -> Result<CouplingResponse> {
        let uri = Url::parse(&params.uri)
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid URI"))?;

        let path = uri
            .to_file_path()
            .map_err(|_| tower_lsp::jsonrpc::Error::invalid_params("Invalid file path"))?;

        let graph = self.graph.read().await;
        let _include_external = params.include_external.unwrap_or(false);

        // Get all symbols in this file
        let file_symbols: HashSet<NodeId> = self
            .symbol_index
            .get_file_symbols(&path)
            .into_iter()
            .collect();

        let mut dependents: Vec<String> = Vec::new();
        let mut dependencies: Vec<String> = Vec::new();
        let mut internal_refs = 0u32;
        let mut external_refs = 0u32;

        // Analyze each symbol in the file
        for node_id in &file_symbols {
            // Outgoing edges (what we depend on)
            let outgoing = self.get_connected_edges(&graph, *node_id, Direction::Outgoing);
            for (_, target, edge_type) in outgoing {
                if edge_type == EdgeType::Imports || edge_type == EdgeType::Calls {
                    if file_symbols.contains(&target) {
                        internal_refs += 1;
                    } else {
                        external_refs += 1;
                        // Get the file path of the dependency
                        if let Ok(target_node) = graph.get_node(target) {
                            if let Some(dep_path) = target_node.properties.get_string("path") {
                                let dep_name = std::path::Path::new(dep_path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                if !dependencies.contains(&dep_name) {
                                    dependencies.push(dep_name);
                                }
                            }
                        }
                    }
                }
            }

            // Incoming edges (who depends on us)
            let incoming = self.get_connected_edges(&graph, *node_id, Direction::Incoming);
            for (source, _, edge_type) in incoming {
                if edge_type == EdgeType::Imports || edge_type == EdgeType::Calls {
                    if !file_symbols.contains(&source) {
                        if let Ok(source_node) = graph.get_node(source) {
                            if let Some(src_path) = source_node.properties.get_string("path") {
                                let src_name = std::path::Path::new(src_path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                if !dependents.contains(&src_name) {
                                    dependents.push(src_name);
                                }
                            }
                        }
                    }
                }
            }
        }

        let afferent = dependents.len() as u32;
        let efferent = dependencies.len() as u32;
        let instability = if afferent + efferent > 0 {
            efferent as f64 / (afferent + efferent) as f64
        } else {
            0.0
        };

        let total_refs = internal_refs + external_refs;
        let internal_ratio = if total_refs > 0 {
            internal_refs as f64 / total_refs as f64
        } else {
            1.0
        };

        // Determine cohesion type
        let cohesion_type = if internal_ratio > 0.7 {
            "functional"
        } else if internal_ratio > 0.4 {
            "sequential"
        } else {
            "coincidental"
        };

        // Generate recommendations
        let mut recommendations = Vec::new();
        let mut violations = Vec::new();

        if instability > 0.8 {
            recommendations.push(
                "High instability - this module depends on many others. Consider reducing dependencies."
                    .to_string(),
            );
        }

        if instability < 0.2 && efferent > 5 {
            violations.push(ArchViolation {
                violation_type: "stable_dependency".to_string(),
                severity: "warning".to_string(),
                description: "Stable module has many outgoing dependencies".to_string(),
                suggestion: "Consider extracting dependencies to make module more focused"
                    .to_string(),
            });
        }

        if internal_ratio < 0.3 {
            recommendations.push(
                "Low cohesion - functions in this module don't reference each other much. Consider splitting."
                    .to_string(),
            );
        }

        if afferent > 10 {
            recommendations.push(format!(
                "Many modules ({}) depend on this one. Changes here have wide impact.",
                afferent
            ));
        }

        Ok(CouplingResponse {
            coupling: CouplingMetrics {
                afferent,
                efferent,
                instability,
                dependents,
                dependencies,
            },
            cohesion: CohesionMetrics {
                score: internal_ratio,
                cohesion_type: cohesion_type.to_string(),
                internal_reference_ratio: internal_ratio,
            },
            violations,
            recommendations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_grade() {
        assert_eq!(CodeGraphBackend::complexity_grade(1), 'A');
        assert_eq!(CodeGraphBackend::complexity_grade(5), 'A');
        assert_eq!(CodeGraphBackend::complexity_grade(6), 'B');
        assert_eq!(CodeGraphBackend::complexity_grade(10), 'B');
        assert_eq!(CodeGraphBackend::complexity_grade(11), 'C');
        assert_eq!(CodeGraphBackend::complexity_grade(20), 'C');
        assert_eq!(CodeGraphBackend::complexity_grade(21), 'D');
        assert_eq!(CodeGraphBackend::complexity_grade(50), 'D');
        assert_eq!(CodeGraphBackend::complexity_grade(51), 'F');
    }

    #[test]
    fn test_file_grade() {
        assert_eq!(CodeGraphBackend::file_grade(3.0), 'A');
        assert_eq!(CodeGraphBackend::file_grade(8.0), 'B');
        assert_eq!(CodeGraphBackend::file_grade(12.0), 'C');
        assert_eq!(CodeGraphBackend::file_grade(20.0), 'D');
        assert_eq!(CodeGraphBackend::file_grade(30.0), 'F');
    }

    #[test]
    fn test_is_entry_point() {
        assert!(CodeGraphBackend::is_entry_point("main"));
        assert!(CodeGraphBackend::is_entry_point("Main"));
        assert!(CodeGraphBackend::is_entry_point("clickHandler"));
        assert!(CodeGraphBackend::is_entry_point("onSubmit"));
        assert!(!CodeGraphBackend::is_entry_point("calculateSum"));
        assert!(!CodeGraphBackend::is_entry_point("processData"));
    }
}
