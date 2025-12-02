//! Error types for the CodeGraph LSP server.

use codegraph_parser_api::ParserError;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur in the LSP server.
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

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),
}

impl From<LspError> for tower_lsp::jsonrpc::Error {
    fn from(err: LspError) -> Self {
        let code = match &err {
            LspError::SymbolNotFound => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::FileNotIndexed(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::InvalidUri(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::UnsupportedLanguage(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            LspError::NodeNotFound(_) => tower_lsp::jsonrpc::ErrorCode::InvalidParams,
            _ => tower_lsp::jsonrpc::ErrorCode::InternalError,
        };

        tower_lsp::jsonrpc::Error {
            code,
            message: err.to_string().into(),
            data: None,
        }
    }
}

/// Result type alias for LSP operations.
pub type LspResult<T> = Result<T, LspError>;
