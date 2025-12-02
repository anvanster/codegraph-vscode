//! File system watcher for incremental updates.

use crate::parser_registry::ParserRegistry;
use codegraph::CodeGraph;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;

/// File system watcher that triggers re-parsing on changes.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Create a new file watcher.
    pub fn new(
        graph: Arc<RwLock<CodeGraph>>,
        parsers: Arc<ParserRegistry>,
        client: Client,
    ) -> Result<Self, notify::Error> {
        let (tx, mut rx) = mpsc::channel::<Event>(100);

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    // Use blocking_send since this is called from a sync context
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        )?;

        // Spawn event handler task
        let graph_clone = Arc::clone(&graph);
        let parsers_clone = Arc::clone(&parsers);
        let client_clone = client.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::handle_event(&graph_clone, &parsers_clone, &client_clone, event).await;
            }
        });

        Ok(Self { _watcher: watcher })
    }

    /// Start watching a directory.
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self._watcher.watch(path, RecursiveMode::Recursive)
    }

    /// Stop watching a directory.
    pub fn unwatch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self._watcher.unwatch(path)
    }

    /// Handle a file system event.
    async fn handle_event(
        graph: &Arc<RwLock<CodeGraph>>,
        parsers: &Arc<ParserRegistry>,
        client: &Client,
        event: Event,
    ) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    // Skip non-parseable files
                    if !parsers.can_parse(&path) {
                        continue;
                    }

                    if let Err(e) = Self::handle_file_change(graph, parsers, &path).await {
                        client
                            .log_message(
                                MessageType::WARNING,
                                format!("Error processing {}: {}", path.display(), e),
                            )
                            .await;
                    } else {
                        tracing::debug!("Re-indexed: {}", path.display());
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if let Err(e) = Self::handle_file_remove(graph, &path).await {
                        client
                            .log_message(
                                MessageType::WARNING,
                                format!("Error removing {}: {}", path.display(), e),
                            )
                            .await;
                    } else {
                        tracing::debug!("Removed from index: {}", path.display());
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle a file creation or modification.
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
        Self::remove_file_nodes(&mut graph, path)?;

        // Parse and add new nodes
        parser.parse_source(&content, path, &mut graph)?;

        Ok(())
    }

    /// Handle a file removal.
    async fn handle_file_remove(
        graph: &Arc<RwLock<CodeGraph>>,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut graph = graph.write().await;
        Self::remove_file_nodes(&mut graph, path)?;
        Ok(())
    }

    /// Remove all nodes associated with a file from the graph.
    fn remove_file_nodes(
        graph: &mut CodeGraph,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path_str = path.to_string_lossy().to_string();

        // Query for all nodes with this file path
        if let Ok(nodes) = graph.query().property("path", path_str).execute() {
            for node_id in nodes {
                // Remove the node (edges are typically removed automatically)
                let _ = graph.delete_node(node_id);
            }
        }

        Ok(())
    }
}

/// Graph updater for batch operations.
pub struct GraphUpdater;

impl GraphUpdater {
    /// Batch update multiple files.
    pub async fn update_files(
        graph: &Arc<RwLock<CodeGraph>>,
        parsers: &Arc<ParserRegistry>,
        files: &[(PathBuf, String)],
    ) -> BatchUpdateResult {
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        let mut graph_guard = graph.write().await;

        for (path, content) in files {
            if let Some(parser) = parsers.parser_for_path(path) {
                // Remove old nodes
                let path_str = path.to_string_lossy().to_string();
                if let Ok(nodes) = graph_guard.query().property("path", path_str).execute() {
                    for node_id in nodes {
                        let _ = graph_guard.delete_node(node_id);
                    }
                }

                // Parse new content
                match parser.parse_source(content, path, &mut graph_guard) {
                    Ok(info) => succeeded.push((path.clone(), info)),
                    Err(e) => failed.push((path.clone(), e.to_string())),
                }
            }
        }

        BatchUpdateResult { succeeded, failed }
    }
}

/// Result of a batch update operation.
pub struct BatchUpdateResult {
    pub succeeded: Vec<(PathBuf, codegraph_parser_api::FileInfo)>,
    pub failed: Vec<(PathBuf, String)>,
}

impl BatchUpdateResult {
    /// Check if all files were updated successfully.
    pub fn all_succeeded(&self) -> bool {
        self.failed.is_empty()
    }

    /// Get the success rate.
    pub fn success_rate(&self) -> f64 {
        let total = self.succeeded.len() + self.failed.len();
        if total == 0 {
            1.0
        } else {
            self.succeeded.len() as f64 / total as f64
        }
    }
}
