//! Symbol indexing for fast lookups.

use codegraph::{CodeGraph, NodeId, PropertyMap};
use codegraph_parser_api::FileInfo;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{Position, Range};

/// Secondary indexes for fast symbol lookups.
pub struct SymbolIndex {
    /// Name -> NodeIds (for workspace symbol search).
    by_name: DashMap<String, Vec<NodeId>>,

    /// File path -> NodeIds (for file-scoped queries).
    by_file: DashMap<PathBuf, Vec<NodeId>>,

    /// Node type -> NodeIds (for type-filtered queries).
    by_type: DashMap<String, Vec<NodeId>>,

    /// Position index for fast position lookups.
    /// Maps file path to sorted list of (range, node_id).
    by_position: DashMap<PathBuf, Vec<(IndexRange, NodeId)>>,
}

/// Internal range representation for indexing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl IndexRange {
    /// Check if this range contains the given position.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }

    /// Convert to LSP Range (0-indexed).
    pub fn to_lsp_range(&self) -> Range {
        Range {
            start: Position {
                line: self.start_line.saturating_sub(1),
                character: self.start_col,
            },
            end: Position {
                line: self.end_line.saturating_sub(1),
                character: self.end_col,
            },
        }
    }
}

impl SymbolIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            by_name: DashMap::new(),
            by_file: DashMap::new(),
            by_type: DashMap::new(),
            by_position: DashMap::new(),
        }
    }

    /// Add a file's symbols to the index.
    pub fn add_file(&self, path: PathBuf, file_info: &FileInfo, graph: &CodeGraph) {
        let mut file_nodes = Vec::new();
        let mut positions = Vec::new();

        // Index all symbols from the file
        let all_node_ids: Vec<NodeId> = file_info
            .functions
            .iter()
            .chain(file_info.classes.iter())
            .chain(file_info.traits.iter())
            .copied()
            .collect();

        for node_id in all_node_ids {
            if let Ok(node) = graph.get_node(node_id) {
                // Index by name
                if let Some(name) = node.properties.get_string("name") {
                    self.by_name
                        .entry(name.to_string())
                        .or_default()
                        .push(node_id);
                }

                // Index by type
                let type_str = format!("{}", node.node_type);
                self.by_type.entry(type_str).or_default().push(node_id);

                file_nodes.push(node_id);

                // Index by position
                if let Some(range) = extract_range(&node.properties) {
                    positions.push((range, node_id));
                }
            }
        }

        // Store file index
        self.by_file.insert(path.clone(), file_nodes);

        // Sort positions for binary search (by start line, then start col)
        positions.sort_by(|a, b| {
            a.0.start_line
                .cmp(&b.0.start_line)
                .then(a.0.start_col.cmp(&b.0.start_col))
        });
        self.by_position.insert(path, positions);
    }

    /// Remove a file's symbols from the index.
    pub fn remove_file(&self, path: &Path) {
        let path_buf = path.to_path_buf();

        // Get nodes to remove
        if let Some((_, nodes)) = self.by_file.remove(&path_buf) {
            // Remove from name index
            for &node_id in &nodes {
                self.by_name.retain(|_, v| {
                    v.retain(|&id| id != node_id);
                    !v.is_empty()
                });

                // Remove from type index
                self.by_type.retain(|_, v| {
                    v.retain(|&id| id != node_id);
                    !v.is_empty()
                });
            }
        }

        // Remove from position index
        self.by_position.remove(&path_buf);
    }

    /// Find node at the given position in a file.
    /// Position is 1-indexed (as stored in graph properties).
    pub fn find_at_position(&self, path: &Path, line: u32, col: u32) -> Option<NodeId> {
        let positions = self.by_position.get(&path.to_path_buf())?;

        // Find the smallest range containing the position
        // (innermost symbol at that position)
        let mut best_match: Option<(usize, NodeId)> = None;

        for (range, node_id) in positions.iter() {
            if range.contains(line, col) {
                let size = ((range.end_line - range.start_line) as usize)
                    * 10000
                    + (range.end_col - range.start_col) as usize;

                match &best_match {
                    Some((best_size, _)) if size < *best_size => {
                        best_match = Some((size, *node_id));
                    }
                    None => {
                        best_match = Some((size, *node_id));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(_, id)| id)
    }

    /// Search symbols by name pattern.
    pub fn search_by_name(&self, pattern: &str) -> Vec<NodeId> {
        let pattern_lower = pattern.to_lowercase();
        let mut results = Vec::new();

        for entry in self.by_name.iter() {
            if entry.key().to_lowercase().contains(&pattern_lower) {
                results.extend(entry.value().iter().copied());
            }
        }

        results
    }

    /// Get all symbols in a file.
    pub fn get_file_symbols(&self, path: &Path) -> Vec<NodeId> {
        self.by_file
            .get(&path.to_path_buf())
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get all symbols of a specific type.
    pub fn get_by_type(&self, node_type: &str) -> Vec<NodeId> {
        self.by_type
            .get(node_type)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Get the range for a node if it was indexed.
    pub fn get_node_range(&self, path: &Path, node_id: NodeId) -> Option<IndexRange> {
        let positions = self.by_position.get(&path.to_path_buf())?;

        for (range, id) in positions.iter() {
            if *id == node_id {
                return Some(range.clone());
            }
        }

        None
    }

    /// Clear all indexes.
    pub fn clear(&self) {
        self.by_name.clear();
        self.by_file.clear();
        self.by_type.clear();
        self.by_position.clear();
    }

    /// Get index statistics.
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_symbols: self.by_name.iter().map(|e| e.value().len()).sum(),
            total_files: self.by_file.len(),
            unique_names: self.by_name.len(),
        }
    }
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Index statistics.
pub struct IndexStats {
    pub total_symbols: usize,
    pub total_files: usize,
    pub unique_names: usize,
}

/// Extract range from node properties.
fn extract_range(properties: &PropertyMap) -> Option<IndexRange> {
    Some(IndexRange {
        start_line: properties.get_int("start_line")? as u32,
        start_col: properties.get_int("start_col")? as u32,
        end_line: properties.get_int("end_line")? as u32,
        end_col: properties.get_int("end_col")? as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_contains() {
        let range = IndexRange {
            start_line: 10,
            start_col: 5,
            end_line: 15,
            end_col: 10,
        };

        // Inside range
        assert!(range.contains(12, 0));
        assert!(range.contains(10, 5));
        assert!(range.contains(15, 10));

        // Outside range
        assert!(!range.contains(9, 0));
        assert!(!range.contains(16, 0));
        assert!(!range.contains(10, 4)); // Before start col on start line
        assert!(!range.contains(15, 11)); // After end col on end line
    }
}
