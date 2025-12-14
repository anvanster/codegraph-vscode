# Autonomous AI Agent Integration - Implementation Plan

## Goal
Enable AI agents (Claude, GitHub Copilot, etc.) to **autonomously discover and use** CodeGraph capabilities without user interaction through VS Code's Language Model Tools API.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         VS Code AI Agent                         │
│                    (Claude, Copilot, GPT-4)                      │
└─────────────────────────────────┬───────────────────────────────┘
                                  │
                    ┌─────────────▼────────────┐
                    │   Language Model API     │
                    │  vscode.lm.invokeTool()  │
                    └─────────────┬────────────┘
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        │                         │                         │
  ┌─────▼──────┐          ┌──────▼───────┐        ┌──────▼───────┐
  │   Tool 1   │          │    Tool 2    │        │    Tool 3    │
  │  get_dep   │          │  get_call    │        │   analyze    │
  │   _graph   │          │   _graph     │        │   _impact    │
  └─────┬──────┘          └──────┬───────┘        └──────┬───────┘
        │                        │                        │
        └────────────────────────┼────────────────────────┘
                                 │
                    ┌────────────▼─────────────┐
                    │  CodeGraph LSP Server    │
                    │  (Rust backend)          │
                    └──────────────────────────┘
```

## Implementation Strategy

### Phase 1: Tool Registration Framework ✅ **READY TO IMPLEMENT**

The VS Code Language Model Tools API is **stable and available** in VS Code 1.90+.

#### Step 1.1: Update Package Configuration

**File**: `package.json`

```json
{
  "engines": {
    "vscode": "^1.90.0"  // Updated from ^1.85.0
  },
  "contributes": {
    "languageModelTools": [
      {
        "name": "codegraph_get_dependency_graph",
        "displayName": "Get Dependency Graph",
        "description": "Retrieve the dependency graph for a source file showing imports and dependencies",
        "modelDescription": "Use this tool to understand what files and modules a given file depends on. Returns a graph with nodes (files/modules) and edges (dependency relationships). Useful for understanding module architecture and import chains.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI to analyze (e.g., file:///path/to/file.ts)"
            },
            "depth": {
              "type": "number",
              "description": "How many levels of dependencies to traverse (1-10, default: 3)",
              "default": 3
            },
            "includeExternal": {
              "type": "boolean",
              "description": "Whether to include external dependencies from node_modules/packages",
              "default": false
            },
            "direction": {
              "type": "string",
              "enum": ["imports", "importedBy", "both"],
              "description": "Direction to analyze: 'imports' (what this file uses), 'importedBy' (what uses this file), or 'both'",
              "default": "both"
            }
          },
          "required": ["uri"]
        }
      },
      {
        "name": "codegraph_get_call_graph",
        "displayName": "Get Call Graph",
        "description": "Retrieve the call graph for a function showing callers and callees",
        "modelDescription": "Use this tool to understand function call relationships. Shows what functions call the target function (callers) and what functions the target calls (callees). Essential for understanding code execution flow and function dependencies.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI containing the function"
            },
            "line": {
              "type": "number",
              "description": "Line number of the function (0-indexed)"
            },
            "character": {
              "type": "number",
              "description": "Character position in the line (0-indexed)",
              "default": 0
            },
            "depth": {
              "type": "number",
              "description": "How many levels deep to traverse the call graph",
              "default": 3
            },
            "direction": {
              "type": "string",
              "enum": ["callers", "callees", "both"],
              "description": "Direction: 'callers' (who calls this), 'callees' (what this calls), or 'both'",
              "default": "both"
            }
          },
          "required": ["uri", "line"]
        }
      },
      {
        "name": "codegraph_analyze_impact",
        "displayName": "Analyze Change Impact",
        "description": "Analyze the impact of modifying, deleting, or renaming a symbol",
        "modelDescription": "Use this tool before making changes to understand the blast radius. Shows all code that would be affected by changing a function, class, or variable. Returns direct impacts (immediate usages) and indirect impacts (transitive dependencies).",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI containing the symbol"
            },
            "line": {
              "type": "number",
              "description": "Line number of the symbol (0-indexed)"
            },
            "character": {
              "type": "number",
              "description": "Character position (0-indexed)",
              "default": 0
            },
            "changeType": {
              "type": "string",
              "enum": ["modify", "delete", "rename"],
              "description": "Type of change to analyze",
              "default": "modify"
            }
          },
          "required": ["uri", "line"]
        }
      },
      {
        "name": "codegraph_get_ai_context",
        "displayName": "Get AI Context",
        "description": "Get comprehensive code context optimized for AI analysis",
        "modelDescription": "Use this tool to get rich context about a code location. Returns the primary code, related code (dependencies, callers, etc.), and architectural context. Automatically selects the most relevant related code based on the intent (explain, modify, debug, or test).",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI to get context for"
            },
            "line": {
              "type": "number",
              "description": "Line number (0-indexed)"
            },
            "character": {
              "type": "number",
              "description": "Character position (0-indexed)",
              "default": 0
            },
            "intent": {
              "type": "string",
              "enum": ["explain", "modify", "debug", "test"],
              "description": "What you plan to do with the context. Affects which related code is selected.",
              "default": "explain"
            },
            "maxTokens": {
              "type": "number",
              "description": "Maximum tokens of context to return",
              "default": 4000
            }
          },
          "required": ["uri", "line"]
        }
      },
      {
        "name": "codegraph_find_related_tests",
        "displayName": "Find Related Tests",
        "description": "Find test files and test functions related to a code location",
        "modelDescription": "Use this tool to discover tests that cover a piece of code. Useful when modifying code to understand what tests need updating, or when debugging to find relevant test cases.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI to find tests for"
            },
            "line": {
              "type": "number",
              "description": "Line number (0-indexed)",
              "default": 0
            }
          },
          "required": ["uri"]
        }
      },
      {
        "name": "codegraph_get_symbol_info",
        "displayName": "Get Symbol Information",
        "description": "Get detailed information about a symbol (function, class, variable, etc.)",
        "modelDescription": "Use this tool to get metadata about a symbol: its type, signature, documentation, location, and usage statistics. Quick way to understand what a symbol is and how it's used.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "uri": {
              "type": "string",
              "description": "The file URI containing the symbol"
            },
            "line": {
              "type": "number",
              "description": "Line number of the symbol (0-indexed)"
            },
            "character": {
              "type": "number",
              "description": "Character position (0-indexed)",
              "default": 0
            }
          },
          "required": ["uri", "line"]
        }
      }
    ]
  }
}
```

#### Step 1.2: Create Tool Manager

**File**: `src/ai/toolManager.ts`

```typescript
import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
    DependencyGraphParams,
    DependencyGraphResponse,
    CallGraphParams,
    CallGraphResponse,
    ImpactAnalysisParams,
    ImpactAnalysisResponse,
    AIContextParams,
    AIContextResponse,
} from '../types';

/**
 * Manages Language Model Tool registrations for CodeGraph
 */
export class CodeGraphToolManager {
    private disposables: vscode.Disposable[] = [];

    constructor(private client: LanguageClient) {}

    /**
     * Register all CodeGraph tools with the Language Model API
     */
    registerTools(): void {
        // Tool 1: Get Dependency Graph
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_dependency_graph', {
                invoke: async (options, token) => {
                    const { uri, depth = 3, includeExternal = false, direction = 'both' } = options.input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.getDependencyGraph',
                            arguments: [{
                                uri,
                                depth,
                                includeExternal,
                                direction,
                            }]
                        }, token) as DependencyGraphResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatDependencyGraph(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri, depth } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Analyzing dependencies for ${fileName} (depth: ${depth})...`
                    };
                }
            })
        );

        // Tool 2: Get Call Graph
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_call_graph', {
                invoke: async (options, token) => {
                    const { uri, line, character = 0, depth = 3, direction = 'both' } = options.input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.getCallGraph',
                            arguments: [{
                                uri,
                                position: { line, character },
                                depth,
                                direction,
                            }]
                        }, token) as CallGraphResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatCallGraph(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri, line } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Analyzing call graph for ${fileName}:${line + 1}...`
                    };
                }
            })
        );

        // Tool 3: Analyze Impact
        this.disposables.push(
            vscode.lm.registerTool('codegraph_analyze_impact', {
                invoke: async (options, token) => {
                    const { uri, line, character = 0, changeType = 'modify' } = options.input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.analyzeImpact',
                            arguments: [{
                                uri,
                                position: { line, character },
                                changeType,
                            }]
                        }, token) as ImpactAnalysisResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatImpactAnalysis(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri, line, changeType } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Analyzing ${changeType} impact for ${fileName}:${line + 1}...`
                    };
                }
            })
        );

        // Tool 4: Get AI Context
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_ai_context', {
                invoke: async (options, token) => {
                    const { uri, line, character = 0, intent = 'explain', maxTokens = 4000 } = options.input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.getAIContext',
                            arguments: [{
                                uri,
                                position: { line, character },
                                contextType: intent,
                                maxTokens,
                            }]
                        }, token) as AIContextResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatAIContext(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri, line, intent } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Getting ${intent} context for ${fileName}:${line + 1}...`
                    };
                }
            })
        );

        // Tool 5: Find Related Tests
        this.disposables.push(
            vscode.lm.registerTool('codegraph_find_related_tests', {
                invoke: async (options, token) => {
                    const { uri, line = 0 } = options.input;

                    try {
                        // This would need a new LSP method: codegraph.findRelatedTests
                        // For now, we can use AI context with 'test' intent
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.getAIContext',
                            arguments: [{
                                uri,
                                position: { line, character: 0 },
                                contextType: 'test',
                                maxTokens: 2000,
                            }]
                        }, token) as AIContextResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatTestContext(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Finding tests related to ${fileName}...`
                    };
                }
            })
        );

        // Tool 6: Get Symbol Info
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_symbol_info', {
                invoke: async (options, token) => {
                    const { uri, line, character = 0 } = options.input;

                    try {
                        // Use existing hover/definition LSP methods
                        const doc = await vscode.workspace.openTextDocument(vscode.Uri.parse(uri));
                        const pos = new vscode.Position(line, character);

                        const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
                            'vscode.executeHoverProvider',
                            doc.uri,
                            pos
                        );

                        const definitions = await vscode.commands.executeCommand<vscode.Location[]>(
                            'vscode.executeDefinitionProvider',
                            doc.uri,
                            pos
                        );

                        const references = await vscode.commands.executeCommand<vscode.Location[]>(
                            'vscode.executeReferenceProvider',
                            doc.uri,
                            pos
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatSymbolInfo({
                                hovers,
                                definitions,
                                references,
                                uri,
                                line,
                                character
                            }))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, token) => {
                    const { uri, line } = options.input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Getting symbol info for ${fileName}:${line + 1}...`
                    };
                }
            })
        );
    }

    /**
     * Format dependency graph for AI consumption
     */
    private formatDependencyGraph(response: DependencyGraphResponse): string {
        const { nodes, edges } = response;

        let output = '# Dependency Graph\n\n';
        output += `Found ${nodes.length} files/modules with ${edges.length} dependencies.\n\n`;

        // Group by import vs importedBy
        const imports = edges.filter(e => e.type === 'import');
        const importedBy = edges.filter(e => e.type === 'importedBy');

        if (imports.length > 0) {
            output += `## Imports (${imports.length})\n`;
            imports.forEach(edge => {
                const source = nodes.find(n => n.id === edge.source);
                const target = nodes.find(n => n.id === edge.target);
                output += `- ${source?.label || edge.source} → ${target?.label || edge.target}\n`;
            });
            output += '\n';
        }

        if (importedBy.length > 0) {
            output += `## Imported By (${importedBy.length})\n`;
            importedBy.forEach(edge => {
                const source = nodes.find(n => n.id === edge.source);
                const target = nodes.find(n => n.id === edge.target);
                output += `- ${source?.label || edge.source} ← ${target?.label || edge.target}\n`;
            });
            output += '\n';
        }

        // Add node details
        output += '## Files/Modules\n';
        nodes.forEach(node => {
            output += `- **${node.label}** (${node.nodeType}, ${node.language})\n`;
            if (node.uri) {
                output += `  Path: ${node.uri}\n`;
            }
        });

        return output;
    }

    /**
     * Format call graph for AI consumption
     */
    private formatCallGraph(response: CallGraphResponse): string {
        const { nodes, edges } = response;

        let output = '# Call Graph\n\n';
        output += `Found ${nodes.length} functions with ${edges.length} call relationships.\n\n`;

        // Find the root node (the function we're analyzing)
        const rootNode = nodes.find(n => n.isRoot);

        if (rootNode) {
            output += `## Target Function\n`;
            output += `**${rootNode.label}** at ${rootNode.uri}\n\n`;
        }

        // Group by callers vs callees
        const callers = edges.filter(e => e.target === rootNode?.id);
        const callees = edges.filter(e => e.source === rootNode?.id);

        if (callers.length > 0) {
            output += `## Callers (${callers.length}) - Who calls this function\n`;
            callers.forEach(edge => {
                const caller = nodes.find(n => n.id === edge.source);
                if (caller) {
                    output += `- **${caller.label}** at ${caller.uri}\n`;
                }
            });
            output += '\n';
        }

        if (callees.length > 0) {
            output += `## Callees (${callees.length}) - What this function calls\n`;
            callees.forEach(edge => {
                const callee = nodes.find(n => n.id === edge.target);
                if (callee) {
                    output += `- **${callee.label}** at ${callee.uri}\n`;
                }
            });
            output += '\n';
        }

        return output;
    }

    /**
     * Format impact analysis for AI consumption
     */
    private formatImpactAnalysis(response: ImpactAnalysisResponse): string {
        let output = '# Impact Analysis\n\n';

        output += `## Summary\n`;
        output += `- Files Affected: ${response.summary.filesAffected}\n`;
        output += `- Breaking Changes: ${response.summary.breakingChanges}\n`;
        output += `- Warnings: ${response.summary.warnings}\n\n`;

        if (response.directImpact.length > 0) {
            output += `## Direct Impact (${response.directImpact.length})\n`;
            response.directImpact.forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴' :
                                impact.severity === 'warning' ? '🟡' : '🔵';
                output += `${severity} **${impact.type}** at ${impact.uri}:${impact.range.start.line + 1}\n`;
            });
            output += '\n';
        }

        if (response.indirectImpact.length > 0) {
            output += `## Indirect Impact (${response.indirectImpact.length})\n`;
            response.indirectImpact.forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴' :
                                impact.severity === 'warning' ? '🟡' : '🔵';
                output += `${severity} ${impact.uri}\n`;
                output += `  Path: ${impact.path.join(' → ')}\n`;
            });
            output += '\n';
        }

        if (response.affectedTests.length > 0) {
            output += `## Affected Tests (${response.affectedTests.length})\n`;
            response.affectedTests.forEach(test => {
                output += `🧪 **${test.testName}** at ${test.uri}\n`;
            });
        }

        return output;
    }

    /**
     * Format AI context for AI consumption
     */
    private formatAIContext(response: AIContextResponse): string {
        let output = '# Code Context\n\n';

        output += `## Primary Code\n`;
        output += `**${response.primaryContext.type}: ${response.primaryContext.name}**\n`;
        output += `Language: ${response.primaryContext.language}\n`;
        output += `Location: ${response.primaryContext.uri}\n\n`;
        output += '```' + response.primaryContext.language + '\n';
        output += response.primaryContext.code + '\n';
        output += '```\n\n';

        if (response.relatedSymbols.length > 0) {
            output += `## Related Code (${response.relatedSymbols.length})\n\n`;
            response.relatedSymbols.slice(0, 5).forEach((symbol, i) => {
                output += `### ${i + 1}. ${symbol.relationship} (relevance: ${(symbol.relevanceScore * 100).toFixed(0)}%)\n`;
                output += `**${symbol.symbolType}: ${symbol.name}**\n`;
                output += '```\n';
                output += symbol.code + '\n';
                output += '```\n\n';
            });
        }

        if (response.architecture) {
            output += `## Architecture Context\n`;
            output += `- Module: ${response.architecture.module}\n`;
            output += `- Neighbors: ${response.architecture.neighbors.join(', ')}\n`;
        }

        return output;
    }

    /**
     * Format test context for AI consumption
     */
    private formatTestContext(response: AIContextResponse): string {
        let output = '# Related Tests\n\n';

        const testSymbols = response.relatedSymbols.filter(s =>
            s.relationship.includes('test') || s.name.includes('test')
        );

        if (testSymbols.length === 0) {
            output += 'No related tests found.\n';
        } else {
            output += `Found ${testSymbols.length} related test(s):\n\n`;
            testSymbols.forEach((test, i) => {
                output += `## ${i + 1}. ${test.name}\n`;
                output += `Type: ${test.symbolType}\n`;
                output += `Relationship: ${test.relationship}\n`;
                output += '```\n';
                output += test.code + '\n';
                output += '```\n\n';
            });
        }

        return output;
    }

    /**
     * Format symbol info for AI consumption
     */
    private formatSymbolInfo(data: {
        hovers?: vscode.Hover[];
        definitions?: vscode.Location[];
        references?: vscode.Location[];
        uri: string;
        line: number;
        character: number;
    }): string {
        let output = '# Symbol Information\n\n';

        output += `Location: ${data.uri}:${data.line + 1}:${data.character + 1}\n\n`;

        if (data.hovers && data.hovers.length > 0) {
            output += '## Documentation\n';
            data.hovers.forEach(hover => {
                hover.contents.forEach(content => {
                    if (typeof content === 'string') {
                        output += content + '\n';
                    } else if ('value' in content) {
                        output += content.value + '\n';
                    }
                });
            });
            output += '\n';
        }

        if (data.definitions && data.definitions.length > 0) {
            output += `## Definition${data.definitions.length > 1 ? 's' : ''}\n`;
            data.definitions.forEach(def => {
                output += `- ${def.uri.fsPath}:${def.range.start.line + 1}\n`;
            });
            output += '\n';
        }

        if (data.references && data.references.length > 0) {
            output += `## References (${data.references.length})\n`;
            // Group by file
            const byFile = new Map<string, vscode.Location[]>();
            data.references.forEach(ref => {
                const path = ref.uri.fsPath;
                if (!byFile.has(path)) {
                    byFile.set(path, []);
                }
                byFile.get(path)!.push(ref);
            });

            byFile.forEach((refs, path) => {
                output += `- **${path}** (${refs.length} reference${refs.length > 1 ? 's' : ''})\n`;
            });
        }

        return output;
    }

    /**
     * Dispose all tool registrations
     */
    dispose(): void {
        this.disposables.forEach(d => d.dispose());
        this.disposables = [];
    }
}
```

#### Step 1.3: Update Extension Activation

**File**: `src/extension.ts`

```typescript
import { CodeGraphToolManager } from './ai/toolManager';

let toolManager: CodeGraphToolManager;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    // ... existing code ...

    // Start the client
    await client.start();

    // Create AI context provider
    aiProvider = new CodeGraphAIProvider(client);

    // 🆕 Register Language Model Tools for autonomous AI agent access
    toolManager = new CodeGraphToolManager(client);
    toolManager.registerTools();

    vscode.window.showInformationMessage('CodeGraph: AI tools registered and ready');

    // Register commands, tree providers, etc.
    registerCommands(context, client, aiProvider);
    registerTreeDataProviders(context, client);

    // Add to disposables
    context.subscriptions.push(client, toolManager);
}
```

### Phase 2: Testing & Validation

#### Test Scenarios

1. **Manual Tool Invocation**
   ```typescript
   // Test from extension development host
   const tools = vscode.lm.tools;
   console.log('Available CodeGraph tools:', tools.filter(t => t.name.startsWith('codegraph_')));

   // Invoke a tool
   const result = await vscode.lm.invokeTool(
       'codegraph_get_dependency_graph',
       { uri: 'file:///path/to/file.ts', depth: 2 },
       new vscode.CancellationTokenSource().token
   );
   console.log('Result:', result);
   ```

2. **AI Agent Discovery**
   - Open Claude Code or GitHub Copilot
   - Ask: "What tools are available to analyze code dependencies?"
   - Expected: AI should list CodeGraph tools
   - Ask: "Show me the dependency graph for the current file"
   - Expected: AI automatically calls `codegraph_get_dependency_graph`

3. **Autonomous Workflow**
   ```
   User: "I want to refactor this function. What will break?"

   AI Thinking:
   1. Calls codegraph_get_symbol_info to understand the function
   2. Calls codegraph_analyze_impact with changeType='modify'
   3. Calls codegraph_find_related_tests to identify affected tests
   4. Synthesizes response with full impact analysis

   AI Response: "Refactoring this function will affect:
   - 5 direct callers in 3 files
   - 12 indirect impacts through the call chain
   - 3 test files that need updating
   [Shows detailed breakdown from tools]"
   ```

### Phase 3: Advanced Features

1. **Tool Chaining Intelligence**
   - AI learns to chain tools effectively
   - Example: `get_symbol_info` → `get_call_graph` → `analyze_impact`

2. **Context-Aware Tool Selection**
   - Based on user intent, AI picks optimal tool
   - "Explain this" → `get_ai_context` with intent='explain'
   - "Will this break?" → `analyze_impact`

3. **Streaming Results**
   - For large graphs, stream partial results
   - AI can process incrementally

## Benefits of This Approach

### For Users
✅ **Zero friction** - No commands to remember, no manual context copying
✅ **Natural language** - Just describe what you want to understand
✅ **Intelligent** - AI picks the right tools automatically
✅ **Fast** - Direct tool calls, no UI interaction needed

### For AI Agents
✅ **Discoverable** - Tools show up in `vscode.lm.tools` automatically
✅ **Well-documented** - `modelDescription` tells AI when to use each tool
✅ **Type-safe** - JSON schema validates inputs
✅ **Reliable** - Standardized API, predictable results

### For Developers
✅ **Simple API** - Register once, works with all AI agents
✅ **Observable** - `prepareInvocation` shows progress
✅ **Cancellable** - Respects cancellation tokens
✅ **Maintainable** - Clean separation of concerns

## Implementation Timeline

- **Week 1**: Package.json updates, tool manager scaffolding
- **Week 2**: Implement all 6 tools, format outputs for AI
- **Week 3**: Testing with Claude Code and GitHub Copilot
- **Week 4**: Documentation, demos, user feedback

## Success Metrics

- ✅ All 6 tools appear in `vscode.lm.tools`
- ✅ AI agents can call tools without errors
- ✅ Tool results are properly formatted for AI consumption
- ✅ Users can ask natural language questions and get graph-backed answers
- ✅ No manual command invocation needed

## Next Steps

1. Update `package.json` engine version to `^1.90.0`
2. Add `languageModelTools` contribution point
3. Create `src/ai/toolManager.ts`
4. Update `src/extension.ts` to register tools
5. Test with Claude Code
6. Iterate based on real AI agent usage patterns
