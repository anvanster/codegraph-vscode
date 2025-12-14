import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import {
    DependencyGraphResponse,
    CallGraphResponse,
    ImpactAnalysisResponse,
    AIContextResponse,
    RelatedTestsResponse,
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
     * Execute an LSP command with a small retry/backoff to smooth over transient timeouts.
     */
    private async sendRequestWithRetry<T>(
        command: string,
        args: unknown,
        token: vscode.CancellationToken,
        options: { retries?: number; delayMs?: number; backoffFactor?: number } = {}
    ): Promise<T> {
        const retries = options.retries ?? 1;
        let delay = options.delayMs ?? 250;
        const backoffFactor = options.backoffFactor ?? 2;

        for (let attempt = 0; attempt <= retries; attempt++) {
            if (token.isCancellationRequested) {
                throw new Error('cancelled');
            }

            try {
                return await this.client.sendRequest(
                    'workspace/executeCommand',
                    { command, arguments: [args] },
                    token
                ) as T;
            } catch (error) {
                const isLastAttempt = attempt === retries;
                if (isLastAttempt || !this.isRetryableError(error)) {
                    throw error;
                }

                await new Promise(resolve => setTimeout(resolve, delay));
                delay *= backoffFactor;
            }
        }

        throw new Error('Request failed after retries');
    }

    private isRetryableError(error: unknown): boolean {
        const message = String(error).toLowerCase();
        return (
            message.includes('timeout') ||
            message.includes('timed out') ||
            message.includes('temporarily unavailable') ||
            message.includes('requestcancelled') ||
            message.includes('cancelled') ||
            message.includes('canceled')
        );
    }

    /**
     * Handle errors from tool invocations, including cancellation.
     * Returns a user-friendly message appropriate for AI agents.
     */
    private handleToolError(error: unknown, toolName: string, token?: vscode.CancellationToken): vscode.LanguageModelToolResult {
        // Check if operation was cancelled
        if (token?.isCancellationRequested) {
            return new vscode.LanguageModelToolResult([
                new vscode.LanguageModelTextPart(`Operation cancelled: ${toolName} was stopped by user request.`)
            ]);
        }

        // Check for cancellation error patterns
        const errorMessage = String(error);
        if (errorMessage.includes('cancelled') || errorMessage.includes('canceled') || errorMessage.includes('RequestCancelled')) {
            return new vscode.LanguageModelToolResult([
                new vscode.LanguageModelTextPart(`Operation cancelled: ${toolName} was stopped.`)
            ]);
        }

        // Return generic error with context
        return new vscode.LanguageModelToolResult([
            new vscode.LanguageModelTextPart(`Error in ${toolName}: ${errorMessage}`)
        ]);
    }

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
                    const input = options.input as { uri: string; depth?: number; includeExternal?: boolean; direction?: 'imports' | 'importedBy' | 'both'; summary?: boolean };
                    const { uri, depth = 3, includeExternal = false, direction = 'both', summary = false } = input;

                    try {
                        const response = await this.sendRequestWithRetry<DependencyGraphResponse>(
                            'codegraph.getDependencyGraph',
                            { uri, depth, includeExternal, direction },
                            token,
                            { retries: 1 }
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatDependencyGraph(response, summary))
                        ]);
                    } catch (error) {
                        return this.handleToolError(error, 'dependency graph', token);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; depth?: number; direction?: string };
                    const { uri, depth = 3, direction = 'both' } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();
                    const directionLabel = direction === 'both' ? 'imports and dependents' : direction === 'importedBy' ? 'dependents' : 'imports';

                    return {
                        invocationMessage: `Analyzing ${directionLabel} for ${fileName} (depth: ${depth})...`
                    };
                }
            })
        );

        // Tool 2: Get Call Graph
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_call_graph', {
                invoke: async (options, token) => {
                    const input = options.input as { uri: string; line: number; character?: number; depth?: number; direction?: 'callers' | 'callees' | 'both'; summary?: boolean };
                    const { uri, line, character = 0, depth = 3, direction = 'both', summary = false } = input;

                    try {
                        const response = await this.sendRequestWithRetry<CallGraphResponse>(
                            'codegraph.getCallGraph',
                            {
                                uri,
                                position: { line, character },
                                depth,
                                direction,
                                includeExternal: false,
                            },
                            token,
                            { retries: 1 }
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatCallGraph(response, summary))
                        ]);
                    } catch (error) {
                        return this.handleToolError(error, 'call graph', token);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; depth?: number; direction?: string };
                    const { uri, line, depth = 3, direction = 'both' } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();
                    const directionLabel = direction === 'both' ? 'callers and callees' : direction;

                    return {
                        invocationMessage: `Analyzing ${directionLabel} for ${fileName}:${line + 1} (depth: ${depth})...`,
                        confirmationMessages: depth > 5 ? {
                            title: 'Deep Call Graph Analysis',
                            message: `Analyzing call graph with depth ${depth} may take longer on large codebases.`
                        } : undefined
                    };
                }
            })
        );

        // Tool 3: Analyze Impact
        this.disposables.push(
            vscode.lm.registerTool('codegraph_analyze_impact', {
                invoke: async (options, token) => {
                    const input = options.input as { uri: string; line: number; character?: number; changeType?: 'modify' | 'delete' | 'rename'; summary?: boolean };
                    const { uri, line, character = 0, changeType = 'modify', summary = false } = input;

                    try {
                        const response = await this.sendRequestWithRetry<ImpactAnalysisResponse>(
                            'codegraph.analyzeImpact',
                            {
                                uri,
                                position: { line, character },
                                analysisType: changeType,
                            },
                            token,
                            { retries: 1 }
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatImpactAnalysis(response, summary))
                        ]);
                    } catch (error) {
                        return this.handleToolError(error, 'impact analysis', token);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; changeType?: string };
                    const { uri, line, changeType = 'modify' } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();

                    return {
                        invocationMessage: `Analyzing ${changeType} impact for ${fileName}:${line + 1}...`,
                        confirmationMessages: changeType === 'delete' ? {
                            title: 'Delete Impact Analysis',
                            message: 'Analyzing what would break if this symbol is deleted.'
                        } : undefined
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
                        const response = await this.sendRequestWithRetry<AIContextResponse>(
                            'codegraph.getAIContext',
                            {
                                uri,
                                position: { line, character },
                                contextType: intent,
                                maxTokens,
                            },
                            token,
                            { retries: 1 }
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatAIContext(response))
                        ]);
                    } catch (error) {
                        // Check for cancellation first
                        if (token.isCancellationRequested) {
                            return this.handleToolError(error, 'AI context', token);
                        }

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
                        } else if (errorMessage.includes('cancelled') || errorMessage.includes('canceled')) {
                            return this.handleToolError(error, 'AI context', token);
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
                    const { uri, line, intent = 'explain' } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();
                    const intentLabels: Record<string, string> = {
                        explain: 'explanation',
                        modify: 'modification',
                        debug: 'debugging',
                        test: 'testing'
                    };

                    return {
                        invocationMessage: `Getting ${intentLabels[intent] || intent} context for ${fileName}:${line + 1}...`
                    };
                }
            })
        );

        // Tool 5: Find Related Tests
        this.disposables.push(
            vscode.lm.registerTool('codegraph_find_related_tests', {
                invoke: async (options, token) => {
                    const input = options.input as { uri: string; line?: number; limit?: number };
                    const { uri, line = 0, limit = 10 } = input;

                    try {
                        const response = await this.sendRequestWithRetry<RelatedTestsResponse>(
                            'codegraph.findRelatedTests',
                            {
                                uri,
                                position: { line, character: 0 },
                                limit,
                            },
                            token,
                            { retries: 1 }
                        );

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatRelatedTests(response))
                        ]);
                    } catch (error) {
                        // Check for cancellation first
                        if (token.isCancellationRequested) {
                            return this.handleToolError(error, 'find related tests', token);
                        }

                        const errorMessage = String(error);
                        if (errorMessage.includes('cancelled') || errorMessage.includes('canceled')) {
                            return this.handleToolError(error, 'find related tests', token);
                        }

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
                    const input = options.input as { uri: string; line?: number };
                    const { uri, line } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();
                    const lineInfo = line !== undefined ? `:${line + 1}` : '';

                    return {
                        invocationMessage: `Finding tests related to ${fileName}${lineInfo}...`
                    };
                }
            })
        );

        // Tool 6: Get Symbol Info
        this.disposables.push(
            vscode.lm.registerTool('codegraph_get_symbol_info', {
                invoke: async (options, token) => {
                    const input = options.input as { uri: string; line: number; character?: number; includeReferences?: boolean };
                    const { uri, line, character = 0, includeReferences = false } = input;

                    try {
                        // Check for cancellation before starting
                        if (token.isCancellationRequested) {
                            return this.handleToolError(new Error('cancelled'), 'symbol info', token);
                        }

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

                        // Only fetch references if explicitly requested (can be slow)
                        let references: vscode.Location[] | undefined;
                        if (includeReferences) {
                            // Check for cancellation before expensive operation
                            if (token.isCancellationRequested) {
                                return this.handleToolError(new Error('cancelled'), 'symbol info', token);
                            }

                            // Use a timeout for reference search (5 seconds)
                            const timeoutPromise = new Promise<vscode.Location[]>((_, reject) => {
                                setTimeout(() => reject(new Error('Reference search timed out after 5s')), 5000);
                            });

                            const refPromise = vscode.commands.executeCommand<vscode.Location[]>(
                                'vscode.executeReferenceProvider',
                                doc.uri,
                                pos
                            );

                            try {
                                references = await Promise.race([refPromise, timeoutPromise]);
                            } catch (timeoutErr) {
                                // Reference search timed out, continue without references
                                console.log('[CodeGraph] Reference search timed out, returning partial results');
                            }
                        }

                        return new vscode.LanguageModelToolResult([
                            new vscode.LanguageModelTextPart(this.formatSymbolInfo({
                                hovers,
                                definitions,
                                references,
                                uri,
                                line,
                                character,
                                referencesIncluded: includeReferences,
                                referencesTimedOut: includeReferences && !references
                            }))
                        ]);
                    } catch (error) {
                        return this.handleToolError(error, 'symbol info', token);
                    }
                },
                prepareInvocation: async (options, _token) => {
                    const input = options.input as { uri: string; line: number; includeReferences?: boolean };
                    const { uri, line, includeReferences = false } = input;
                    const fileName = vscode.Uri.parse(uri).path.split('/').pop();
                    const refNote = includeReferences ? ' (including references - may be slow)' : '';

                    return {
                        invocationMessage: `Getting symbol info for ${fileName}:${line + 1}${refNote}...`,
                        confirmationMessages: includeReferences ? {
                            title: 'Reference Search',
                            message: 'Including references can be slow on large workspaces. Consider using without references for faster results.'
                        } : undefined
                    };
                }
            })
        );

        console.log(`[CodeGraph] Registered ${this.disposables.length} Language Model tools`);
    }

    /**
     * Format dependency graph for AI consumption
     */
    private formatDependencyGraph(response: DependencyGraphResponse, summary = false): string {
        const { nodes, edges } = response;
        const shouldSummarize = summary || nodes.length > 50 || edges.length > 80;

        let output = shouldSummarize ? '# Dependency Graph (summary)\n\n' : '# Dependency Graph\n\n';
        output += `Found ${nodes.length} files/modules with ${edges.length} dependencies.\n\n`;

        const imports = edges.filter(e => e.type === 'import' || e.type === 'require' || e.type === 'use');

        const edgeLimit = shouldSummarize ? 15 : imports.length;
        if (imports.length > 0) {
            output += `## Dependencies (${imports.length})\n`;
            imports.slice(0, edgeLimit).forEach(edge => {
                const fromNode = nodes.find(n => n.id === edge.from);
                const toNode = nodes.find(n => n.id === edge.to);
                output += `- ${fromNode?.label || edge.from} → ${toNode?.label || edge.to} (${edge.type})\n`;
            });
            if (imports.length > edgeLimit) {
                output += `... and ${imports.length - edgeLimit} more\n`;
            }
            output += '\n';
        }

        const nodeLimit = shouldSummarize ? 15 : nodes.length;
        output += `## Files/Modules${shouldSummarize ? ' (sample)' : ''}\n`;
        nodes.slice(0, nodeLimit).forEach(node => {
            output += `- **${node.label}** (${node.type}, ${node.language})\n`;
            if (node.uri) {
                output += `  Path: ${node.uri}\n`;
            }
        });
        if (nodes.length > nodeLimit) {
            output += `... and ${nodes.length - nodeLimit} more\n`;
        }

        return output;
    }

    /**
     * Format call graph for AI consumption
     */
    private formatCallGraph(response: CallGraphResponse, summary = false): string {
        const { root, nodes, edges } = response;

        let output = summary ? '# Call Graph (summary)\n\n' : '# Call Graph\n\n';

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

        const shouldSummarize = summary || nodes.length > 50 || edges.length > 80;
        output += `Found ${nodes.length} functions with ${edges.length} call relationships.\n\n`;

        output += `## Target Function\n`;
        output += `**${root.name}** (${root.signature})\n`;
        output += `Location: ${root.uri}\n`;
        if (root.metrics) {
            output += `Complexity: ${root.metrics.complexity || 'N/A'}, Lines: ${root.metrics.linesOfCode || 'N/A'}\n`;
        }
        output += '\n';

        const callers = edges.filter(e => e.to === root.id);
        const callees = edges.filter(e => e.from === root.id);

        const callerLimit = shouldSummarize ? 15 : callers.length;
        if (callers.length > 0) {
            output += `## Callers (${callers.length})\n`;
            output += 'Functions that call this:\n';
            callers.slice(0, callerLimit).forEach(edge => {
                const caller = nodes.find(n => n.id === edge.from);
                if (caller) {
                    output += `- **${caller.name}** at ${caller.uri}\n`;
                }
            });
            if (callers.length > callerLimit) {
                output += `... and ${callers.length - callerLimit} more\n`;
            }
            output += '\n';
        }

        const calleeLimit = shouldSummarize ? 15 : callees.length;
        if (callees.length > 0) {
            output += `## Callees (${callees.length})\n`;
            output += 'Functions that this calls:\n';
            callees.slice(0, calleeLimit).forEach(edge => {
                const callee = nodes.find(n => n.id === edge.to);
                if (callee) {
                    output += `- **${callee.name}** at ${callee.uri}\n`;
                }
            });
            if (callees.length > calleeLimit) {
                output += `... and ${callees.length - calleeLimit} more\n`;
            }
            output += '\n';
        }

        return output;
    }

    /**
     * Format impact analysis for AI consumption
     */
    private formatImpactAnalysis(response: ImpactAnalysisResponse, summary = false): string {
        const shouldSummarize = summary || response.directImpact.length > 50 || response.indirectImpact.length > 50;

        let output = shouldSummarize ? '# Impact Analysis (summary)\n\n' : '# Impact Analysis\n\n';

        output += `## Summary\n`;
        output += `- Files Affected: ${response.summary.filesAffected}\n`;
        output += `- Breaking Changes: ${response.summary.breakingChanges}\n`;
        output += `- Warnings: ${response.summary.warnings}\n\n`;

        const directLimit = shouldSummarize ? 20 : response.directImpact.length;
        if (response.directImpact.length > 0) {
            output += `## Direct Impact (${response.directImpact.length})\n`;
            output += 'Immediate usages that will be affected:\n';
            response.directImpact.slice(0, directLimit).forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴 BREAKING' :
                                impact.severity === 'warning' ? '🟡 WARNING' : '🔵 INFO';
                output += `${severity}: **${impact.type}** at ${impact.uri}:${impact.range.start.line + 1}\n`;
            });
            if (response.directImpact.length > directLimit) {
                output += `... and ${response.directImpact.length - directLimit} more\n`;
            }
            output += '\n';
        }

        const indirectLimit = shouldSummarize ? 15 : response.indirectImpact.length;
        if (response.indirectImpact.length > 0) {
            output += `## Indirect Impact (${response.indirectImpact.length})\n`;
            output += 'Transitive dependencies that will be affected:\n';
            response.indirectImpact.slice(0, indirectLimit).forEach(impact => {
                const severity = impact.severity === 'breaking' ? '🔴' :
                                impact.severity === 'warning' ? '🟡' : '🔵';
                output += `${severity} ${impact.uri}\n`;
                output += `  Dependency path: ${impact.path.join(' → ')}\n`;
            });
            if (response.indirectImpact.length > indirectLimit) {
                output += `... and ${response.indirectImpact.length - indirectLimit} more\n`;
            }
            output += '\n';
        }

        const testsLimit = shouldSummarize ? 10 : response.affectedTests.length;
        if (response.affectedTests.length > 0) {
            output += `## Affected Tests (${response.affectedTests.length})\n`;
            output += 'Tests that may need updating:\n';
            response.affectedTests.slice(0, testsLimit).forEach(test => {
                output += `🧪 **${test.testName}** at ${test.uri}\n`;
            });
            if (response.affectedTests.length > testsLimit) {
                output += `... and ${response.affectedTests.length - testsLimit} more\n`;
            }
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
    private formatRelatedTests(response: RelatedTestsResponse): string {
        let output = '# Related Tests\n\n';

        if (!response.tests.length) {
            output += 'No related tests found in the codebase.\n';
            output += '\nThis could mean:\n';
            output += '- No tests exist for this code yet\n';
            output += '- Tests exist but are not directly connected in the dependency graph\n';
            output += '- Tests may use mocking or indirect references\n';
            return output;
        }

        output += `Found ${response.tests.length} related test(s):\n\n`;

        response.tests.forEach((test, i) => {
            output += `## ${i + 1}. ${test.testName}\n`;
            output += `Relationship: ${test.relationship}\n`;
            output += `Location: ${test.uri}:${test.range.start.line + 1}\n`;
            output += '\n';
        });

        if (response.truncated) {
            output += '_Results truncated; refine the selection or increase the limit for more tests._\n';
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
        referencesIncluded?: boolean;
        referencesTimedOut?: boolean;
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
                if (!ref || !ref.uri) { return; }
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
        } else if (data.referencesIncluded) {
            // References were requested but none found or timed out
            if (data.referencesTimedOut) {
                output += '## References\n';
                output += '⏱️ Reference search timed out (>5s). The symbol may have many references.\n';
                output += 'Try narrowing your search or use `codegraph_analyze_impact` for dependency analysis.\n\n';
            } else {
                output += '## References\n';
                output += 'No references found for this symbol.\n\n';
            }
        } else {
            // References not requested
            output += '## References\n';
            output += '_References not included. Set `includeReferences: true` to find usages (may be slow)._\n\n';
        }

        if (!data.hovers?.length && !data.definitions?.length && !data.references?.length && !data.referencesIncluded) {
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
