import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
    DependencyGraphResponse,
    CallGraphResponse,
    ImpactAnalysisResponse,
    AIContextResponse,
} from '../types';

/**
 * Manages Language Model Tool registrations for CodeGraph.
 *
 * This enables AI agents (Claude, GitHub Copilot, etc.) to autonomously
 * discover and use CodeGraph capabilities through VS Code's Language Model API.
 */
export class CodeGraphToolManager {
    private disposables: vscode.Disposable[] = [];

    constructor(private client: LanguageClient) {}

    /**
     * Register all CodeGraph tools with the Language Model API.
     *
     * Tools are automatically discoverable by all AI agents in VS Code.
     * AI agents can call these tools autonomously without user interaction.
     */
    registerTools(): void {
        console.log('[CodeGraph] Registering Language Model tools...');

        // Check if vscode.lm API exists
        if (!(vscode as any).lm) {
            console.error('[CodeGraph] vscode.lm API not available - VS Code version may be too old (need 1.90+)');
            vscode.window.showWarningMessage('CodeGraph: Language Model Tools require VS Code 1.90+. Tool registration skipped.');
            return;
        }

        if (typeof (vscode as any).lm.registerTool !== 'function') {
            console.error('[CodeGraph] vscode.lm.registerTool is not a function - API might have changed');
            vscode.window.showWarningMessage('CodeGraph: vscode.lm.registerTool not available. Tool registration skipped.');
            return;
        }

        console.log('[CodeGraph] vscode.lm API available, proceeding with tool registration...');

        // Tool 1: Get Dependency Graph
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_dependency_graph', {
                invoke: async (options, token) => {
                    const input = options.input as { uri: string; depth?: number; includeExternal?: boolean; direction?: 'imports' | 'importedBy' | 'both' };
                    const { uri, depth = 3, includeExternal = false, direction = 'both' } = input;

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
                            new vscode.LanguageModelTextPart(`Error getting dependency graph: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; depth?: number };
                    const { uri, depth } = input;
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
                    const input = options.input as { uri: string; line: number; character?: number; depth?: number; direction?: 'callers' | 'callees' | 'both' };
                    const { uri, line, character = 0, depth = 3, direction = 'both' } = input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.getCallGraph',
                            arguments: [{
                                uri,
                                position: { line, character },
                                depth,
                                direction,
                                includeExternal: false,
                            }]
                        }, token) as CallGraphResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatCallGraph(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error getting call graph: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number };
                    const { uri, line } = input;
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
                    const input = options.input as { uri: string; line: number; character?: number; changeType?: 'modify' | 'delete' | 'rename' };
                    const { uri, line, character = 0, changeType = 'modify' } = input;

                    try {
                        const response = await this.client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.analyzeImpact',
                            arguments: [{
                                uri,
                                position: { line, character },
                                analysisType: changeType,
                            }]
                        }, token) as ImpactAnalysisResponse;

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatImpactAnalysis(response))
                        ]);
                    } catch (error) {
                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(`Error analyzing impact: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; changeType?: string };
                    const { uri, line, changeType } = input;
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
                    const input = options.input as { uri: string; line: number; character?: number; intent?: 'explain' | 'modify' | 'debug' | 'test'; maxTokens?: number };
                    const { uri, line, character = 0, intent = 'explain', maxTokens = 4000 } = input;

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
                        const errorMessage = String(error);
                        let helpfulMessage = '# AI Context Unavailable\n\n';

                        if (errorMessage.includes('No symbol at position')) {
                            helpfulMessage += '❌ No code symbol found at the specified position.\n\n';
                            helpfulMessage += '**This could mean:**\n';
                            helpfulMessage += '- The position is in whitespace, comments, or imports\n';
                            helpfulMessage += '- The file has not been indexed by CodeGraph yet\n';
                            helpfulMessage += '- The specified line/character is out of bounds\n\n';
                            helpfulMessage += '**Try:**\n';
                            helpfulMessage += '- Place cursor on a function, class, or variable definition\n';
                            helpfulMessage += '- Run "CodeGraph: Reindex Workspace" to update the index\n';
                            helpfulMessage += '- Verify the file is a supported language (TypeScript, JavaScript, Python, Rust, Go)\n';
                        } else {
                            helpfulMessage += `Error: ${errorMessage}\n`;
                        }

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(helpfulMessage)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; intent?: string };
                    const { uri, line, intent } = input;
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
                    const input = options.input as { uri: string; line?: number };
                    const { uri, line = 0 } = input;

                    try {
                        // Use AI context with 'test' intent to find related tests
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
                        const errorMessage = String(error);
                        let helpfulMessage = '# Related Tests Not Found\n\n';

                        if (errorMessage.includes('No symbol at position')) {
                            helpfulMessage += '❌ No code symbol found to search for related tests.\n\n';
                            helpfulMessage += '**This could mean:**\n';
                            helpfulMessage += '- The specified position is not on a testable code element\n';
                            helpfulMessage += '- The file has not been indexed by CodeGraph yet\n';
                            helpfulMessage += '- No tests exist for this code (which might be OK)\n\n';
                            helpfulMessage += '**Try:**\n';
                            helpfulMessage += '- Specify a line with a function or class definition\n';
                            helpfulMessage += '- Run "CodeGraph: Reindex Workspace" to update the index\n';
                            helpfulMessage += '- Check if tests actually exist in your codebase\n';
                        } else {
                            helpfulMessage += `Error: ${errorMessage}\n`;
                        }

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(helpfulMessage)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string };
                    const { uri } = input;
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
                invoke: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; character?: number };
                    const { uri, line, character = 0 } = input;

                    try {
                        // Use existing LSP hover/definition/reference providers
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
                            new vscode.LanguageModelTextPart(`Error getting symbol info: ${error}`)
                        ]);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number };
                    const { uri, line } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Getting symbol info for ${fileName}:${line + 1}...`
                    };
                }
            })
        );

        console.log(`[CodeGraph] Registered ${this.disposables.length} Language Model tools`);
    }

    /**
     * Format dependency graph for AI consumption
     */
    private formatDependencyGraph(response: DependencyGraphResponse): string {
        const { nodes, edges } = response;

        let output = '# Dependency Graph\n\n';
        output += `Found ${nodes.length} files/modules with ${edges.length} dependencies.\n\n`;

        // Group edges by type
        const imports = edges.filter(e => e.type === 'import' || e.type === 'require' || e.type === 'use');

        if (imports.length > 0) {
            output += `## Dependencies (${imports.length})\n`;
            imports.forEach(edge => {
                const fromNode = nodes.find(n => n.id === edge.from);
                const toNode = nodes.find(n => n.id === edge.to);
                output += `- ${fromNode?.label || edge.from} → ${toNode?.label || edge.to} (${edge.type})\n`;
            });
            output += '\n';
        }

        // Add node details
        output += '## Files/Modules\n';
        nodes.forEach(node => {
            output += `- **${node.label}** (${node.type}, ${node.language})\n`;
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
        const { root, nodes, edges } = response;

        let output = '# Call Graph\n\n';

        // Handle case where no function was found at the position
        if (!root) {
            output += '❌ No function found at the specified position.\n\n';
            output += 'This could mean:\n';
            output += '- The cursor is not on a function definition\n';
            output += '- The file has not been indexed yet\n';
            output += '- The position is in a comment or whitespace\n\n';
            output += 'Try:\n';
            output += '- Place cursor on a function name\n';
            output += '- Run "CodeGraph: Reindex Workspace" if the file is new\n';
            return output;
        }

        output += `Found ${nodes.length} functions with ${edges.length} call relationships.\n\n`;

        // Show root function
        output += `## Target Function\n`;
        output += `**${root.name}** (${root.signature})\n`;
        output += `Location: ${root.uri}\n`;
        if (root.metrics) {
            output += `Complexity: ${root.metrics.complexity || 'N/A'}, Lines: ${root.metrics.linesOfCode || 'N/A'}\n`;
        }
        output += '\n';

        // Group by callers vs callees
        const callers = edges.filter(e => e.to === root.id);
        const callees = edges.filter(e => e.from === root.id);

        if (callers.length > 0) {
            output += `## Callers (${callers.length})\n`;
            output += 'Functions that call this:\n';
            callers.forEach(edge => {
                const caller = nodes.find(n => n.id === edge.from);
                if (caller) {
                    output += `- **${caller.name}** at ${caller.uri}\n`;
                }
            });
            output += '\n';
        }

        if (callees.length > 0) {
            output += `## Callees (${callees.length})\n`;
            output += 'Functions that this calls:\n';
            callees.forEach(edge => {
                const callee = nodes.find(n => n.id === edge.to);
                if (callee) {
                    output += `- **${callee.name}** at ${callee.uri}\n`;
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
            output += 'Immediate usages that will be affected:\n';
            response.directImpact.forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴 BREAKING' :
                                impact.severity === 'warning' ? '🟡 WARNING' : '🔵 INFO';
                output += `${severity}: **${impact.type}** at ${impact.uri}:${impact.range.start.line + 1}\n`;
            });
            output += '\n';
        }

        if (response.indirectImpact.length > 0) {
            output += `## Indirect Impact (${response.indirectImpact.length})\n`;
            output += 'Transitive dependencies that will be affected:\n';
            response.indirectImpact.forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴' :
                                impact.severity === 'warning' ? '🟡' : '🔵';
                output += `${severity} ${impact.uri}\n`;
                output += `  Dependency path: ${impact.path.join(' → ')}\n`;
            });
            output += '\n';
        }

        if (response.affectedTests.length > 0) {
            output += `## Affected Tests (${response.affectedTests.length})\n`;
            output += 'Tests that may need updating:\n';
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
        output += `Location: ${response.primaryContext.location.uri}\n\n`;
        output += '```' + response.primaryContext.language + '\n';
        output += response.primaryContext.code + '\n';
        output += '```\n\n';

        if (response.relatedSymbols.length > 0) {
            output += `## Related Code (${response.relatedSymbols.length})\n\n`;
            response.relatedSymbols.slice(0, 5).forEach((symbol, i) => {
                output += `### ${i + 1}. ${symbol.relationship} (relevance: ${(symbol.relevanceScore * 100).toFixed(0)}%)\n`;
                output += `**${symbol.name}**\n`;
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
            s.relationship.toLowerCase().includes('test') ||
            s.name.toLowerCase().includes('test')
        );

        if (testSymbols.length === 0) {
            output += 'No related tests found in the codebase.\n';
            output += '\nThis could mean:\n';
            output += '- No tests exist for this code yet\n';
            output += '- Tests exist but are not directly connected in the dependency graph\n';
            output += '- Tests may use mocking or indirect references\n';
        } else {
            output += `Found ${testSymbols.length} related test(s):\n\n`;
            testSymbols.forEach((test, i) => {
                output += `## ${i + 1}. ${test.name}\n`;
                output += `Relationship: ${test.relationship}\n`;
                output += `Relevance: ${(test.relevanceScore * 100).toFixed(0)}%\n`;
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
            output += '## Documentation & Type Information\n';
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
                if (def && def.uri && def.range) {
                    output += `- ${def.uri.fsPath}:${def.range.start.line + 1}\n`;
                }
            });
            output += '\n';
        }

        if (data.references && data.references.length > 0) {
            output += `## References (${data.references.length} usage${data.references.length > 1 ? 's' : ''})\n`;
            // Group by file
            const byFile = new Map<string, vscode.Location[]>();
            data.references.forEach(ref => {
                if (!ref || !ref.uri) return;
                const path = ref.uri.fsPath;
                if (!byFile.has(path)) {
                    byFile.set(path, []);
                }
                byFile.get(path)!.push(ref);
            });

            byFile.forEach((refs, path) => {
                const fileName = path.split('/').pop();
                output += `- **${fileName}** (${refs.length} reference${refs.length > 1 ? 's' : ''})\n`;
                refs.slice(0, 3).forEach(ref => {
                    output += `  Line ${ref.range.start.line + 1}\n`;
                });
                if (refs.length > 3) {
                    output += `  ... and ${refs.length - 3} more\n`;
                }
            });
        }

        if (!data.hovers?.length && !data.definitions?.length && !data.references?.length) {
            output += 'No symbol information available at this location.\n';
        }

        return output;
    }

    /**
     * Dispose all tool registrations
     */
    dispose(): void {
        console.log('[CodeGraph] Disposing Language Model tools');
        this.disposables.forEach(d => d.dispose());
        this.disposables = [];
    }
}
