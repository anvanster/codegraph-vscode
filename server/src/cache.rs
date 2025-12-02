//! Query caching for performance optimization.

use codegraph::NodeId;
use dashmap::DashMap;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Mutex;
use tower_lsp::lsp_types::{Location, Range};

/// Cache for definition lookups.
type DefinitionCache = DashMap<(PathBuf, u32, u32), NodeId>;

/// Cache for references lookups.
type ReferencesCache = DashMap<NodeId, Vec<Location>>;

/// Caches for expensive queries.
pub struct QueryCache {
    /// Fast lookup cache for definitions.
    definitions: DefinitionCache,

    /// Fast lookup cache for references.
    references: ReferencesCache,

    /// LRU cache for call hierarchy results.
    call_hierarchies: Mutex<LruCache<NodeId, CallHierarchyCache>>,

    /// LRU cache for dependency graph results.
    dependency_graphs: Mutex<LruCache<(PathBuf, usize), DependencyGraphCache>>,

    /// LRU cache for AI context results.
    ai_contexts: Mutex<LruCache<(NodeId, String), AIContextCache>>,
}

/// Cached call hierarchy data.
#[derive(Clone)]
pub struct CallHierarchyCache {
    pub incoming: Vec<(NodeId, Vec<Range>)>,
    pub outgoing: Vec<(NodeId, Vec<Range>)>,
}

/// Cached dependency graph data.
#[derive(Clone)]
pub struct DependencyGraphCache {
    pub nodes: Vec<NodeId>,
    pub edges: Vec<(NodeId, NodeId, String)>,
}

/// Cached AI context data.
#[derive(Clone)]
pub struct AIContextCache {
    pub primary_code: String,
    pub related_symbols: Vec<(NodeId, String, f64)>,
}

impl QueryCache {
    /// Create a new cache with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap());

        Self {
            definitions: DashMap::new(),
            references: DashMap::new(),
            call_hierarchies: Mutex::new(LruCache::new(capacity)),
            dependency_graphs: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity.get() / 2).unwrap_or(NonZeroUsize::new(50).unwrap()),
            )),
            ai_contexts: Mutex::new(LruCache::new(capacity)),
        }
    }

    // ==========================================
    // Definition Cache
    // ==========================================

    /// Get cached definition.
    pub fn get_definition(&self, path: &PathBuf, line: u32, character: u32) -> Option<NodeId> {
        self.definitions
            .get(&(path.clone(), line, character))
            .map(|v| *v)
    }

    /// Store definition in cache.
    pub fn set_definition(&self, path: PathBuf, line: u32, character: u32, node_id: NodeId) {
        self.definitions.insert((path, line, character), node_id);
    }

    // ==========================================
    // References Cache
    // ==========================================

    /// Get cached references.
    pub fn get_references(&self, node_id: NodeId) -> Option<Vec<Location>> {
        self.references.get(&node_id).map(|v| v.clone())
    }

    /// Store references in cache.
    pub fn set_references(&self, node_id: NodeId, locations: Vec<Location>) {
        self.references.insert(node_id, locations);
    }

    // ==========================================
    // Call Hierarchy Cache
    // ==========================================

    /// Get cached call hierarchy.
    pub fn get_call_hierarchy(&self, node_id: NodeId) -> Option<CallHierarchyCache> {
        self.call_hierarchies.lock().ok()?.get(&node_id).cloned()
    }

    /// Store call hierarchy in cache.
    pub fn set_call_hierarchy(&self, node_id: NodeId, cache: CallHierarchyCache) {
        if let Ok(mut guard) = self.call_hierarchies.lock() {
            guard.put(node_id, cache);
        }
    }

    // ==========================================
    // Dependency Graph Cache
    // ==========================================

    /// Get cached dependency graph.
    pub fn get_dependency_graph(
        &self,
        path: &PathBuf,
        depth: usize,
    ) -> Option<DependencyGraphCache> {
        self.dependency_graphs
            .lock()
            .ok()?
            .get(&(path.clone(), depth))
            .cloned()
    }

    /// Store dependency graph in cache.
    pub fn set_dependency_graph(&self, path: PathBuf, depth: usize, cache: DependencyGraphCache) {
        if let Ok(mut guard) = self.dependency_graphs.lock() {
            guard.put((path, depth), cache);
        }
    }

    // ==========================================
    // AI Context Cache
    // ==========================================

    /// Get cached AI context.
    pub fn get_ai_context(&self, node_id: NodeId, context_type: &str) -> Option<AIContextCache> {
        self.ai_contexts
            .lock()
            .ok()?
            .get(&(node_id, context_type.to_string()))
            .cloned()
    }

    /// Store AI context in cache.
    pub fn set_ai_context(&self, node_id: NodeId, context_type: String, cache: AIContextCache) {
        if let Ok(mut guard) = self.ai_contexts.lock() {
            guard.put((node_id, context_type), cache);
        }
    }

    // ==========================================
    // Invalidation
    // ==========================================

    /// Invalidate all cache entries for a file.
    pub fn invalidate_file(&self, path: &PathBuf) {
        // Remove definition entries for this file
        self.definitions
            .retain(|(p, _, _), _| p != path);

        // Clear references cache (could be more selective)
        self.references.clear();

        // Clear call hierarchies
        if let Ok(mut guard) = self.call_hierarchies.lock() {
            guard.clear();
        }

        // Remove dependency graphs for this file
        if let Ok(mut guard) = self.dependency_graphs.lock() {
            // Note: LRU doesn't have retain, so we need to work around this
            // For now, just clear if the path matches the first in key
            guard.clear();
        }
    }

    /// Invalidate entire cache.
    pub fn invalidate_all(&self) {
        self.definitions.clear();
        self.references.clear();

        if let Ok(mut guard) = self.call_hierarchies.lock() {
            guard.clear();
        }

        if let Ok(mut guard) = self.dependency_graphs.lock() {
            guard.clear();
        }

        if let Ok(mut guard) = self.ai_contexts.lock() {
            guard.clear();
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            definitions_count: self.definitions.len(),
            references_count: self.references.len(),
            call_hierarchies_count: self
                .call_hierarchies
                .lock()
                .map(|g| g.len())
                .unwrap_or(0),
            dependency_graphs_count: self
                .dependency_graphs
                .lock()
                .map(|g| g.len())
                .unwrap_or(0),
            ai_contexts_count: self.ai_contexts.lock().map(|g| g.len()).unwrap_or(0),
        }
    }
}

/// Cache statistics.
pub struct CacheStats {
    pub definitions_count: usize,
    pub references_count: usize,
    pub call_hierarchies_count: usize,
    pub dependency_graphs_count: usize,
    pub ai_contexts_count: usize,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(1000)
    }
}
