# Changelog

All notable changes to the CodeGraph extension will be documented in this file.

## [0.2.0] - 2024-12-30

### Added

- **C Language Support**: Full parsing support for C source files using `codegraph-c` parser
  - Tolerant kernel mode for parsing Linux kernel drivers and system code without `compile_commands.json`
  - Handles kernel macros (`__init`, `__exit`, `likely()`, etc.) and GCC extensions
  - For full semantic analysis, generate a compilation database using `bear -- make`
- Extension icon for VS Code marketplace
- 3 additional Language Model Tools (now 9 total):
  - `codegraph_analyze_complexity` - Measure cyclomatic and cognitive complexity
  - `codegraph_find_unused_code` - Detect dead code for cleanup
  - `codegraph_analyze_coupling` - Analyze module coupling and cohesion

### Changed

- Improved symbol index with O(1) node-to-file reverse lookup
- Enhanced path resolution in handlers with fallback to symbol index
- Updated build script with `--all-platforms` and `--sync-binaries` options

## [0.1.0] - 2024-12-11

### Added

- Initial release
- Cross-language code intelligence for TypeScript, JavaScript, Python, Rust, and Go
- Dependency graph visualization
- Call graph analysis
- Impact analysis for refactoring
- AI integration via Language Model Tools (6 tools for AI agents)
- `@codegraph` chat participant for VS Code AI chats
- Rust LSP server powered by codegraph crates
