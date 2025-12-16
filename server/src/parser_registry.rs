//! Parser Registry - Manages all language parsers implementing the CodeParser trait.

use codegraph::CodeGraph;
use codegraph_c::CParser;
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
    c: Arc<CParser>,
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
            go: Arc::new(GoParser::with_config(config.clone())),
            c: Arc::new(CParser::with_config(config)),
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
            "c" => Some(self.c.clone()),
            _ => None,
        }
    }

    /// Find appropriate parser for a file path.
    pub fn parser_for_path(&self, path: &Path) -> Option<Arc<dyn CodeParser>> {
        let parsers: [Arc<dyn CodeParser>; 5] = [
            self.python.clone(),
            self.rust.clone(),
            self.typescript.clone(),
            self.go.clone(),
            self.c.clone(),
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
        extensions.extend(self.c.file_extensions().iter().copied());
        extensions
    }

    /// Get metrics from all parsers.
    pub fn all_metrics(&self) -> Vec<(&str, ParserMetrics)> {
        vec![
            ("python", self.python.metrics()),
            ("rust", self.rust.metrics()),
            ("typescript", self.typescript.metrics()),
            ("go", self.go.metrics()),
            ("c", self.c.metrics()),
        ]
    }

    /// Check if a file path is supported by any parser.
    pub fn can_parse(&self, path: &Path) -> bool {
        self.parser_for_path(path).is_some()
    }

    /// Parse a file using the appropriate parser.
    pub fn parse_file(&self, path: &Path, graph: &mut CodeGraph) -> Result<FileInfo, ParserError> {
        let parser = self.parser_for_path(path).ok_or_else(|| {
            ParserError::UnsupportedFeature(path.to_path_buf(), "Unsupported file type".to_string())
        })?;

        parser.parse_file(path, graph)
    }

    /// Parse source code string using the appropriate parser for the given path.
    pub fn parse_source(
        &self,
        source: &str,
        path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<FileInfo, ParserError> {
        let parser = self.parser_for_path(path).ok_or_else(|| {
            ParserError::UnsupportedFeature(path.to_path_buf(), "Unsupported file type".to_string())
        })?;

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
        } else if self.c.can_parse(path) {
            Some("c")
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
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parser_registry_new() {
        let registry = ParserRegistry::new();
        // Should have parsers for all five languages
        assert!(registry.get_parser("python").is_some());
        assert!(registry.get_parser("rust").is_some());
        assert!(registry.get_parser("typescript").is_some());
        assert!(registry.get_parser("go").is_some());
        assert!(registry.get_parser("c").is_some());
    }

    #[test]
    fn test_parser_registry_default() {
        let registry = ParserRegistry::default();
        // Should be equivalent to new()
        assert!(registry.get_parser("python").is_some());
    }

    #[test]
    fn test_parser_registry_with_config() {
        let config = ParserConfig::default();
        let registry = ParserRegistry::with_config(config);
        assert!(registry.get_parser("python").is_some());
    }

    #[test]
    fn test_get_parser_case_insensitive() {
        let registry = ParserRegistry::new();

        assert!(registry.get_parser("Python").is_some());
        assert!(registry.get_parser("PYTHON").is_some());
        assert!(registry.get_parser("Rust").is_some());
        assert!(registry.get_parser("RUST").is_some());
        assert!(registry.get_parser("TypeScript").is_some());
        assert!(registry.get_parser("Go").is_some());
        assert!(registry.get_parser("C").is_some());
    }

    #[test]
    fn test_get_parser_javascript_variants() {
        let registry = ParserRegistry::new();

        // All JS variants should return the typescript parser
        assert!(registry.get_parser("javascript").is_some());
        assert!(registry.get_parser("typescriptreact").is_some());
        assert!(registry.get_parser("javascriptreact").is_some());
    }

    #[test]
    fn test_get_parser_unknown_language() {
        let registry = ParserRegistry::new();

        assert!(registry.get_parser("cobol").is_none());
        assert!(registry.get_parser("unknown").is_none());
        assert!(registry.get_parser("").is_none());
    }

    #[test]
    fn test_parser_for_path() {
        let registry = ParserRegistry::new();

        assert!(registry
            .parser_for_path(&PathBuf::from("test.py"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.rs"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.ts"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.js"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.go"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.c"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.h"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("test.txt"))
            .is_none());
    }

    #[test]
    fn test_parser_for_path_react_extensions() {
        let registry = ParserRegistry::new();

        assert!(registry
            .parser_for_path(&PathBuf::from("component.tsx"))
            .is_some());
        assert!(registry
            .parser_for_path(&PathBuf::from("component.jsx"))
            .is_some());
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
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.c")),
            Some("c")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("test.h")),
            Some("c")
        );
    }

    #[test]
    fn test_language_for_path_react_extensions() {
        let registry = ParserRegistry::new();

        assert_eq!(
            registry.language_for_path(&PathBuf::from("component.tsx")),
            Some("typescript")
        );
        assert_eq!(
            registry.language_for_path(&PathBuf::from("component.jsx")),
            Some("javascript")
        );
    }

    #[test]
    fn test_language_for_path_unknown() {
        let registry = ParserRegistry::new();

        assert_eq!(registry.language_for_path(&PathBuf::from("test.txt")), None);
        assert_eq!(registry.language_for_path(&PathBuf::from("Makefile")), None);
    }

    #[test]
    fn test_supported_extensions() {
        let registry = ParserRegistry::new();
        let extensions = registry.supported_extensions();

        // Check that we have extensions for all 5 languages
        // (exact extension names may vary by parser implementation)
        assert!(!extensions.is_empty());
        assert!(extensions.len() >= 5); // At least one extension per language
    }

    #[test]
    fn test_can_parse() {
        let registry = ParserRegistry::new();

        assert!(registry.can_parse(Path::new("test.py")));
        assert!(registry.can_parse(Path::new("test.rs")));
        assert!(registry.can_parse(Path::new("test.ts")));
        assert!(registry.can_parse(Path::new("test.js")));
        assert!(registry.can_parse(Path::new("test.go")));
        assert!(registry.can_parse(Path::new("test.c")));
        assert!(registry.can_parse(Path::new("test.h")));
        assert!(!registry.can_parse(Path::new("test.txt")));
        assert!(!registry.can_parse(Path::new("test.md")));
    }

    #[test]
    fn test_all_metrics() {
        let registry = ParserRegistry::new();
        let metrics = registry.all_metrics();

        assert_eq!(metrics.len(), 5);
        assert_eq!(metrics[0].0, "python");
        assert_eq!(metrics[1].0, "rust");
        assert_eq!(metrics[2].0, "typescript");
        assert_eq!(metrics[3].0, "go");
        assert_eq!(metrics[4].0, "c");
    }

    #[test]
    fn test_parse_source_python() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "def hello():\n    print('hello')\n";
        let path = Path::new("test.py");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_source_rust() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "fn hello() { println!(\"hello\"); }";
        let path = Path::new("test.rs");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_source_typescript() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "function hello(): void { console.log('hello'); }";
        let path = Path::new("test.ts");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_source_go() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "package main\n\nfunc hello() { fmt.Println(\"hello\") }";
        let path = Path::new("test.go");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_source_c() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "#include <stdio.h>\n\nvoid hello() { printf(\"hello\\n\"); }";
        let path = Path::new("test.c");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_source_unsupported() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();
        let source = "some content";
        let path = Path::new("test.txt");

        let result = registry.parse_source(source, path, &mut graph);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_file() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();

        // Create a temporary Python file
        let mut temp_file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(temp_file, "def test_function():\n    pass").unwrap();

        let result = registry.parse_file(temp_file.path(), &mut graph);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_file_unsupported() {
        let registry = ParserRegistry::new();
        let mut graph = CodeGraph::in_memory().unwrap();

        // Create a temporary text file
        let mut temp_file = NamedTempFile::with_suffix(".txt").unwrap();
        writeln!(temp_file, "some text content").unwrap();

        let result = registry.parse_file(temp_file.path(), &mut graph);
        assert!(result.is_err());
    }
}
