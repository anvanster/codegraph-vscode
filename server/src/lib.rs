//! CodeGraph LSP Server Library
//!
//! This crate implements a Language Server Protocol (LSP) server for CodeGraph,
//! providing cross-language code intelligence through graph-based analysis.

pub mod backend;
pub mod cache;
pub mod custom_requests;
pub mod error;
pub mod handlers;
pub mod index;
pub mod parser_registry;
pub mod watcher;

pub use backend::CodeGraphBackend;
pub use error::LspError;
pub use parser_registry::ParserRegistry;
