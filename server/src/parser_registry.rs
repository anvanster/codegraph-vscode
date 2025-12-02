//! Parser Registry - Manages all language parsers implementing the CodeParser trait.

use codegraph::CodeGraph;
use codegraph_go::GoParser;
use codegraph_parser_api::{CodeParser, FileInfo, ParserConfig, ParserError, ParserMetrics};
use codegraph_python::PythonParser;
use codegraph_rust::RustParser;
use codegraph_typescript::TypeScriptParser;
use std::path::Path;
use std::sync::Arc;

/// Registry of all available language parsers.
pub struct ParserRegistry {
    python: Arc<PythonParser>,
    rust: Arc<RustParser>,
    typescript: Arc<TypeScriptParser>,
    go: Arc<GoParser>,
}

impl ParserRegistry {
    /// Create a new parser registry with default configuration.
    pub fn new() -> Self {
        Self::with_config(ParserConfig::default())
    }

    /// Create a new parser registry with custom configuration.
    pub fn with_config(config: ParserConfig) -> Self {
        Self {
            python: Arc::new(PythonParser::with_config(config.clone())),
            rust: Arc::new(RustParser::with_config(config.clone())),
            typescript: Arc::new(TypeScriptParser::with_config(config.clone())),
            go: Arc::new(GoParser::with_config(config)),
        }
    }

    /// Get parser by language identifier.
    pub fn get_parser(&self, language: &str) -> Option<Arc<dyn CodeParser>> {
        match language.to_lowercase().as_str() {
            "python" => Some(self.python.clone()),
            "rust" => Some(self.rust.clone()),
            "typescript" | "javascript" | "typescriptreact" | "javascriptreact" => {
                Some(self.typescript.clone())
            }
            "go" => Some(self.go.clone()),
            _ => None,
        }
    }

    /// Find appropriate parser for a file path.
    pub fn parser_for_path(&self, path: &Path) -> Option<Arc<dyn CodeParser>> {
        let parsers: [Arc<dyn CodeParser>; 4] = [
            self.python.clone(),
            self.rust.clone(),
            self.typescript.clone(),
            self.go.clone(),
        ];

        parsers.into_iter().find(|p| p.can_parse(path))
    }

    /// Get all supported file extensions.
    pub fn supported_extensions(&self) -> Vec<&str> {
        let mut extensions = Vec::new();
        extensions.extend(self.python.file_extensions().iter().copied());
        extensions.extend(self.rust.file_extensions().iter().copied());
        extensions.extend(self.typescript.file_extensions().iter().copied());
        extensions.extend(self.go.file_extensions().iter().copied());
        extensions
    }

    /// Get metrics from all parsers.
    pub fn all_metrics(&self) -> Vec<(&str, ParserMetrics)> {
        vec![
            ("python", self.python.metrics()),
            ("rust", self.rust.metrics()),
            ("typescript", self.typescript.metrics()),
            ("go", self.go.metrics()),
        ]
    }

    /// Check if a file path is supported by any parser.
    pub fn can_parse(&self, path: &Path) -> bool {
        self.parser_for_path(path).is_some()
    }

    /// Parse a file using the appropriate parser.
    pub fn parse_file(
        &self,
        path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<FileInfo, ParserError> {
        let parser = self
            .parser_for_path(path)
            .ok_or_else(|| ParserError::UnsupportedFeature(path.to_path_buf(), "Unsupported file type".to_string()))?;

        parser.parse_file(path, graph)
    }

    /// Parse source code string using the appropriate parser for the given path.
    pub fn parse_source(
        &self,
        source: &str,
        path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<FileInfo, ParserError> {
        let parser = self
            .parser_for_path(path)
            .ok_or_else(|| ParserError::UnsupportedFeature(path.to_path_buf(), "Unsupported file type".to_string()))?;

        parser.parse_source(source, path, graph)
    }

    /// Get language name for a file path.
    pub fn language_for_path(&self, path: &Path) -> Option<&'static str> {
        if self.python.can_parse(path) {
            Some("python")
        } else if self.rust.can_parse(path) {
            Some("rust")
        } else if self.typescript.can_parse(path) {
            // Determine if it's TypeScript or JavaScript
            if let Some(ext) = path.extension() {
                match ext.to_str() {
                    Some("ts") | Some("tsx") => Some("typescript"),
                    Some("js") | Some("jsx") => Some("javascript"),
                    _ => Some("typescript"),
                }
            } else {
                Some("typescript")
            }
        } else if self.go.can_parse(path) {
            Some("go")
        } else {
            None
        }
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parser_for_path() {
        let registry = ParserRegistry::new();

        assert!(registry.parser_for_path(&PathBuf::from("test.py")).is_some());
        assert!(registry.parser_for_path(&PathBuf::from("test.rs")).is_some());
        assert!(registry.parser_for_path(&PathBuf::from("test.ts")).is_some());
        assert!(registry.parser_for_path(&PathBuf::from("test.js")).is_some());
        assert!(registry.parser_for_path(&PathBuf::from("test.go")).is_some());
        assert!(registry.parser_for_path(&PathBuf::from("test.txt")).is_none());
    }

    #[test]
    fn test_language_for_path() {
        let registry = ParserRegistry::new();

        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.py")),
            Some("python")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.rs")),
            Some("rust")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.ts")),
            Some("typescript")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.js")),
            Some("javascript")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.go")),
            Some("go")
        );
    }
}
