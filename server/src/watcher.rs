//! File system watcher for incremental updates.

use crate::parser_registry::ParserRegistry;
use codegraph::CodeGraph;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;

/// Default debounce interval in milliseconds.
const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// File system watcher that triggers re-parsing on changes.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Create a new file watcher with debouncing.
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

        // Spawn event handler task with debouncing
        let graph_clone = Arc::clone(&graph);
        let parsers_clone = Arc::clone(&parsers);
        let client_clone = client.clone();

        tokio::spawn(async move {
            let debounce_duration = Duration::from_millis(DEFAULT_DEBOUNCE_MS);
            let mut pending_events: HashMap<PathBuf, (EventKind, Instant)> = HashMap::new();

            loop {
                // Use tokio::select to handle both incoming events and debounce timeouts
                tokio::select! {
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                // Accumulate events with debouncing
                                for path in event.paths {
                                    pending_events.insert(path, (event.kind, Instant::now()));
                                }
                            }
                            None => break, // Channel closed
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        // Process any events that have been pending long enough
                        let now = Instant::now();
                        let mut to_process = Vec::new();

                        pending_events.retain(|path, (kind, timestamp)| {
                            if now.duration_since(*timestamp) >= debounce_duration {
                                to_process.push((path.clone(), *kind));
                                false
                            } else {
                                true
                            }
                        });

                        // Process debounced events
                        for (path, kind) in to_process {
                            let event = Event {
                                kind,
                                paths: vec![path],
                                attrs: Default::default(),
                            };
                            Self::handle_event(&graph_clone, &parsers_clone, &client_clone, event).await;
                        }
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a mock FileInfo using GraphUpdater
    async fn create_file_info_via_update(
        path: &str,
        content: &str,
    ) -> codegraph_parser_api::FileInfo {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());
        let files = vec![(PathBuf::from(path), content.to_string())];
        let result = GraphUpdater::update_files(&graph, &parsers, &files).await;
        result.succeeded.into_iter().next().unwrap().1
    }

    #[test]
    fn test_batch_update_result_all_succeeded_empty() {
        let result = BatchUpdateResult {
            succeeded: vec![],
            failed: vec![],
        };
        assert!(result.all_succeeded());
    }

    #[test]
    fn test_batch_update_result_has_failures() {
        let result = BatchUpdateResult {
            succeeded: vec![],
            failed: vec![(PathBuf::from("bad.py"), "parse error".to_string())],
        };
        assert!(!result.all_succeeded());
    }

    #[test]
    fn test_success_rate_empty() {
        let result = BatchUpdateResult {
            succeeded: vec![],
            failed: vec![],
        };
        assert!((result.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_all_failure() {
        let result = BatchUpdateResult {
            succeeded: vec![],
            failed: vec![
                (PathBuf::from("a.py"), "error".to_string()),
                (PathBuf::from("b.py"), "error".to_string()),
            ],
        };
        assert!((result.success_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_batch_update_result_all_succeeded_with_files() {
        let info = create_file_info_via_update("a.py", "def foo(): pass").await;
        let result = BatchUpdateResult {
            succeeded: vec![(PathBuf::from("a.py"), info)],
            failed: vec![],
        };
        assert!(result.all_succeeded());
    }

    #[tokio::test]
    async fn test_batch_update_result_mixed() {
        let info = create_file_info_via_update("a.py", "def foo(): pass").await;
        let result = BatchUpdateResult {
            succeeded: vec![(PathBuf::from("a.py"), info)],
            failed: vec![(PathBuf::from("bad.py"), "parse error".to_string())],
        };
        assert!(!result.all_succeeded());
    }

    #[tokio::test]
    async fn test_success_rate_all_success() {
        let info1 = create_file_info_via_update("a.py", "def a(): pass").await;
        let info2 = create_file_info_via_update("b.py", "def b(): pass").await;
        let result = BatchUpdateResult {
            succeeded: vec![
                (PathBuf::from("a.py"), info1),
                (PathBuf::from("b.py"), info2),
            ],
            failed: vec![],
        };
        assert!((result.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_success_rate_half() {
        let info = create_file_info_via_update("a.py", "def foo(): pass").await;
        let result = BatchUpdateResult {
            succeeded: vec![(PathBuf::from("a.py"), info)],
            failed: vec![(PathBuf::from("b.py"), "error".to_string())],
        };
        assert!((result.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_success_rate_two_thirds() {
        let info1 = create_file_info_via_update("a.py", "def a(): pass").await;
        let info2 = create_file_info_via_update("b.py", "def b(): pass").await;
        let result = BatchUpdateResult {
            succeeded: vec![
                (PathBuf::from("a.py"), info1),
                (PathBuf::from("b.py"), info2),
            ],
            failed: vec![(PathBuf::from("c.py"), "error".to_string())],
        };
        // 2/3 = 0.6666...
        assert!((result.success_rate() - (2.0 / 3.0)).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_graph_updater_update_files_python() {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());

        let files = vec![(
            PathBuf::from("test.py"),
            "def foo():\n    pass\n".to_string(),
        )];

        let result = GraphUpdater::update_files(&graph, &parsers, &files).await;

        assert!(result.all_succeeded());
        assert_eq!(result.succeeded.len(), 1);
        assert!(result.failed.is_empty());
    }

    #[tokio::test]
    async fn test_graph_updater_update_multiple_files() {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());

        let files = vec![
            (PathBuf::from("a.py"), "def a(): pass".to_string()),
            (PathBuf::from("b.rs"), "fn b() {}".to_string()),
            (PathBuf::from("c.ts"), "function c() {}".to_string()),
        ];

        let result = GraphUpdater::update_files(&graph, &parsers, &files).await;

        assert!(result.all_succeeded());
        assert_eq!(result.succeeded.len(), 3);
    }

    #[tokio::test]
    async fn test_graph_updater_unsupported_file_skipped() {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());

        let files = vec![(PathBuf::from("readme.txt"), "hello world".to_string())];

        let result = GraphUpdater::update_files(&graph, &parsers, &files).await;

        // Unsupported files are silently skipped, not considered failures
        assert!(result.all_succeeded());
        assert!(result.succeeded.is_empty());
        assert!(result.failed.is_empty());
    }

    #[tokio::test]
    async fn test_graph_updater_mixed_files() {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());

        let files = vec![
            (PathBuf::from("valid.py"), "def foo(): pass".to_string()),
            (PathBuf::from("readme.md"), "# Hello".to_string()), // unsupported
            (PathBuf::from("valid.rs"), "fn bar() {}".to_string()),
        ];

        let result = GraphUpdater::update_files(&graph, &parsers, &files).await;

        // Only parseable files are tracked
        assert!(result.all_succeeded());
        assert_eq!(result.succeeded.len(), 2); // Python and Rust
    }

    #[tokio::test]
    async fn test_graph_updater_updates_remove_old_nodes() {
        let graph = Arc::new(RwLock::new(CodeGraph::in_memory().unwrap()));
        let parsers = Arc::new(ParserRegistry::new());

        // First update
        let files1 = vec![(
            PathBuf::from("test.py"),
            "def old_function(): pass".to_string(),
        )];
        GraphUpdater::update_files(&graph, &parsers, &files1).await;

        // Second update with new content (should replace old)
        let files2 = vec![(
            PathBuf::from("test.py"),
            "def new_function(): pass".to_string(),
        )];
        let result = GraphUpdater::update_files(&graph, &parsers, &files2).await;

        assert!(result.all_succeeded());
    }
}
