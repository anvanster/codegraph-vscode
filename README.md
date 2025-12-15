# CodeGraph

**Cross-language code intelligence powered by graph analysis.**

[![VS Code](https://img.shields.io/badge/VS%20Code-1.90+-blue.svg)](https://code.visualstudio.com/)
[![License](https://img.shields.io/badge/License-Apache%202.0-green.svg)](LICENSE)

CodeGraph analyzes your codebase to understand relationships between files, functions, and modules. It provides **dependency graphs**, **call graphs**, **impact analysis**, and **AI-optimized context** — all accessible through VS Code commands, chat participants, and Language Model Tools.

---

## Why CodeGraph?

Traditional code exploration requires multiple grep searches, file reads, and manual analysis. CodeGraph provides **semantic understanding** of your code:

| Task | Traditional Approach | With CodeGraph |
|------|---------------------|----------------|
| "What does this function call?" | 5-7 grep/read operations | 1 call graph query |
| "What breaks if I change this?" | Manual tracing across files | 1 impact analysis |
| "Show me dependencies" | Parse imports manually | 1 dependency graph |
| "Get context for AI" | Read multiple related files | 1 context request |

**Benchmarks show 75-80% reduction in tool calls and tokens** when AI agents use CodeGraph tools vs. traditional file operations.

---

## Features

### 🔍 Dependency Graph Visualization
See what files and modules your code depends on — and what depends on it.

### 📞 Call Graph Analysis
Understand function call relationships: who calls what, and how deep the call chain goes.

### ⚡ Impact Analysis
Before you refactor, know exactly what will break. See direct impacts, indirect impacts, and affected tests.

### 🤖 AI Integration
CodeGraph provides tools for AI assistants (GitHub Copilot, Claude, etc.) to understand your code more efficiently:
- **@codegraph chat participant** — Ask questions in any AI chat
- **Language Model Tools** — AI agents can autonomously query code structure
- **Intent-aware context** — Get relevant code based on what you're trying to do (explain, modify, debug, test)

### 🌐 Multi-Language Support
Works with **TypeScript**, **JavaScript**, **Python**, **Rust**, and **Go** in the same project.

---

## Quick Start

1. Install CodeGraph from the VS Code Marketplace
2. Open a project with supported languages
3. Wait for initial indexing (status bar shows progress)
4. Use commands from the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)

### Commands

| Command | Description |
|---------|-------------|
| `CodeGraph: Show Dependency Graph` | Visualize module dependencies |
| `CodeGraph: Show Call Graph` | Show function call relationships |
| `CodeGraph: Analyze Impact` | Analyze impact of modifying current symbol |
| `CodeGraph: Show Parser Metrics` | View parsing statistics |
| `CodeGraph: Reindex Workspace` | Force re-indexing of all files |

---

## AI Integration

### Chat Participant (@codegraph)

In any VS Code AI chat, mention `@codegraph` to get code intelligence:

```
@codegraph what are the dependencies of this file?
@codegraph show me the call graph for this function
@codegraph what would break if I change this?
@codegraph find tests related to this code
```

### Language Model Tools

CodeGraph registers 9 tools that AI agents can use autonomously:

| Tool | Purpose |
|------|---------|
| `codegraph_get_dependency_graph` | Analyze file/module dependencies |
| `codegraph_get_call_graph` | Map function call relationships |
| `codegraph_analyze_impact` | Assess change impact before refactoring |
| `codegraph_get_ai_context` | Get intent-aware code context |
| `codegraph_find_related_tests` | Discover tests for a code location |
| `codegraph_get_symbol_info` | Get symbol metadata and usage |
| `codegraph_analyze_complexity` | Measure cyclomatic and cognitive complexity |
| `codegraph_find_unused_code` | Detect dead code for cleanup |
| `codegraph_analyze_coupling` | Analyze module coupling and cohesion |

See [AI_TOOL_EXAMPLES.md](docs/AI_TOOL_EXAMPLES.md) for detailed usage examples and best practices.

---

## Configuration

| Setting | Description | Default |
|---------|-------------|---------|
| `codegraph.enabled` | Enable/disable the extension | `true` |
| `codegraph.languages` | Languages to index | `["python", "rust", "typescript", "javascript", "go"]` |
| `codegraph.indexOnStartup` | Index workspace on startup | `true` |
| `codegraph.maxFileSizeKB` | Maximum file size to index (KB) | `1024` |
| `codegraph.excludePatterns` | Glob patterns to exclude | `["**/node_modules/**", "**/target/**", ...]` |
| `codegraph.ai.maxContextTokens` | Max tokens for AI context | `4000` |
| `codegraph.ai.contextStrategy` | Context selection strategy | `"smart"` |
| `codegraph.visualization.defaultDepth` | Default graph depth | `3` |
| `codegraph.cache.enabled` | Enable query caching | `true` |
| `codegraph.parallelParsing` | Enable parallel file parsing | `true` |

---

## Requirements

- **VS Code 1.90+** (required for Language Model Tools API)
- **Rust toolchain** (for building the LSP server from source)

---

## How It Works

CodeGraph consists of two main components:

### VS Code Extension (TypeScript)
The extension provides the user interface: commands, visualizations, chat participant, and Language Model Tools registration. It communicates with the LSP server over the standard Language Server Protocol.

### Rust LSP Server
A high-performance LSP server built with [tower-lsp](https://github.com/ebkalderon/tower-lsp) that handles all code analysis. It uses the **[codegraph](https://crates.io/crates/codegraph) Rust ecosystem** for parsing and graph construction:

| Crate | Purpose |
|-------|---------|
| `codegraph` | Core graph data structures and query engine |
| `codegraph-parser-api` | Unified parser trait for all languages |
| `codegraph-typescript` | TypeScript/JavaScript parser (tree-sitter) |
| `codegraph-python` | Python parser (rustpython-parser) |
| `codegraph-rust` | Rust parser (syn + tree-sitter) |
| `codegraph-go` | Go parser (tree-sitter) |

```
┌─────────────────────────────────────────────────────────────┐
│                   VS Code Extension (TS)                     │
├─────────────────────────────────────────────────────────────┤
│  Commands    │  Chat Participant  │  Language Model Tools   │
│  & Views     │  (@codegraph)      │  (AI Agent Access)      │
└──────────────┴───────────────────┴─────────────────────────┘
                            │
                      LSP Protocol
                            │
┌─────────────────────────────────────────────────────────────┐
│                  Rust LSP Server (tower-lsp)                 │
├─────────────────────────────────────────────────────────────┤
│                     Parser Registry                          │
│  ┌────────────┐ ┌────────┐ ┌──────┐ ┌────┐                  │
│  │ TypeScript │ │ Python │ │ Rust │ │ Go │                  │
│  └────────────┘ └────────┘ └──────┘ └────┘                  │
├─────────────────────────────────────────────────────────────┤
│  CodeGraph Core: Semantic Graph │ Query Engine │ Caching    │
└─────────────────────────────────┴─────────────┴─────────────┘
```

The Rust server provides sub-100ms response times for navigation queries and can index a 100k LOC codebase in under 10 seconds.

---

## Building from Source

```bash
# Clone the repository
git clone https://github.com/anvanster/codegraph-vscode
cd codegraph-vscode

# Install dependencies
npm install

# Build the Rust LSP server
cd server && cargo build --release && cd ..

# Build the extension
npm run compile

# Package the extension
npm run package
```

### Development

```bash
# Watch mode for TypeScript
npm run watch

# Run tests
npm test

# Launch Extension Development Host
# Press F5 in VS Code
```

---

## License

Apache-2.0

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Reporting Issues

Found a bug or have a feature request? [Open an issue](https://github.com/anvanster/codegraph-vscode/issues).
