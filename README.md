# CodeGraph VS Code Extension

Cross-language code intelligence powered by graph analysis.

## Features

- **Cross-Language Navigation**: Navigate definitions and references across Python, Rust, TypeScript, JavaScript, and Go
- **Dependency Graph Visualization**: Interactive visualization of module dependencies
- **Call Graph Analysis**: View call hierarchies across language boundaries
- **AI Context Provider**: Smart context selection for AI coding assistants
- **Impact Analysis**: Understand the impact of code changes before making them

## Installation

### From VS Code Marketplace

Search for "CodeGraph" in the VS Code Extensions view.

### From Source

1. Clone this repository
2. Install dependencies:
   ```bash
   npm install
   ```
3. Build the LSP server:
   ```bash
   cd server && cargo build --release
   ```
4. Build the extension:
   ```bash
   npm run compile
   ```
5. Press F5 in VS Code to launch the Extension Development Host

## Commands

- **CodeGraph: Show Dependency Graph** - Visualize module dependencies
- **CodeGraph: Show Call Graph** - Show function call relationships
- **CodeGraph: Analyze Impact** - Analyze impact of modifying current symbol
- **CodeGraph: Show Parser Metrics** - View parsing statistics
- **CodeGraph: Open AI Assistant** - Get AI-optimized code context
- **CodeGraph: Reindex Workspace** - Force re-indexing of all files

## Configuration

| Setting | Description | Default |
|---------|-------------|---------|
| `codegraph.enabled` | Enable/disable the extension | `true` |
| `codegraph.languages` | Languages to index | `["python", "rust", "typescript", "javascript", "go"]` |
| `codegraph.indexOnStartup` | Index workspace on startup | `true` |
| `codegraph.maxFileSizeKB` | Maximum file size to index (KB) | `1024` |
| `codegraph.excludePatterns` | Glob patterns for files to exclude | `["**/node_modules/**", ...]` |
| `codegraph.ai.maxContextTokens` | Maximum tokens for AI context | `4000` |
| `codegraph.visualization.defaultDepth` | Default depth for graph visualizations | `3` |

## Architecture

```
codegraph-vscode/
├── src/                    # TypeScript extension
│   ├── extension.ts        # Entry point
│   ├── commands/           # Command handlers
│   ├── views/              # Tree views and webviews
│   └── ai/                 # AI context provider
├── server/                 # Rust LSP server
│   └── src/
│       ├── main.rs         # Server entry point
│       ├── backend.rs      # LSP handler
│       ├── parser_registry.rs  # Multi-language parser management
│       └── handlers/       # Custom LSP request handlers
└── webview/               # Graph visualization (React + D3)
```

## Development

### Prerequisites

- Node.js 18+
- Rust 1.70+
- VS Code 1.85+

### Building

```bash
# Install dependencies
npm install

# Build everything
npm run compile
npm run build-server

# Watch mode for TypeScript
npm run watch
```

### Testing

```bash
npm test
```

## LSP Protocol Extensions

The extension implements custom LSP methods:

| Method | Description |
|--------|-------------|
| `codegraph/getDependencyGraph` | Get module dependency graph |
| `codegraph/getCallGraph` | Get function call graph |
| `codegraph/getAIContext` | Get AI-optimized code context |
| `codegraph/analyzeImpact` | Analyze change impact |
| `codegraph/getParserMetrics` | Get parser statistics |

## License

Apache-2.0

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
