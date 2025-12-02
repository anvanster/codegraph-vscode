import * as vscode from 'vscode';
import { LanguageClient, RequestType } from 'vscode-languageclient/node';
import { CodeGraphAIProvider } from '../ai/contextProvider';
import {
    DependencyGraphParams,
    DependencyGraphResponse,
    CallGraphParams,
    CallGraphResponse,
    ImpactAnalysisParams,
    ImpactAnalysisResponse,
    ParserMetricsParams,
    ParserMetricsResponse,
} from '../types';
import { GraphVisualizationPanel } from '../views/graphPanel';

// Define custom request types
namespace GetDependencyGraphRequest {
    export const type = new RequestType<DependencyGraphParams, DependencyGraphResponse, void>(
        'codegraph/getDependencyGraph'
    );
}

namespace GetCallGraphRequest {
    export const type = new RequestType<CallGraphParams, CallGraphResponse, void>(
        'codegraph/getCallGraph'
    );
}

namespace GetImpactAnalysisRequest {
    export const type = new RequestType<ImpactAnalysisParams, ImpactAnalysisResponse, void>(
        'codegraph/analyzeImpact'
    );
}

namespace GetParserMetricsRequest {
    export const type = new RequestType<ParserMetricsParams, ParserMetricsResponse, void>(
        'codegraph/getParserMetrics'
    );
}

namespace ReindexWorkspaceRequest {
    export const type = new RequestType<void, void, void>(
        'codegraph/reindexWorkspace'
    );
}

/**
 * Register all CodeGraph commands
 */
export function registerCommands(
    context: vscode.ExtensionContext,
    client: LanguageClient,
    aiProvider: CodeGraphAIProvider
): void {
    // Helper to safely register commands
    const safeRegisterCommand = (commandId: string, callback: (...args: any[]) => any) => {
        try {
            context.subscriptions.push(vscode.commands.registerCommand(commandId, callback));
        } catch (error) {
            console.warn(`Command ${commandId} already registered, skipping`);
        }
    };

    // Show Dependency Graph
    safeRegisterCommand('codegraph.showDependencyGraph', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('CodeGraph: No active editor');
                return;
            }

            try {
                const response = await client.sendRequest('workspace/executeCommand', {
                    command: 'codegraph.getDependencyGraph',
                    arguments: [{
                        uri: editor.document.uri.toString(),
                        depth: vscode.workspace.getConfiguration('codegraph')
                            .get<number>('visualization.defaultDepth', 3),
                        includeExternal: false,
                        direction: 'both',
                    }]
                }) as DependencyGraphResponse;

                GraphVisualizationPanel.createOrShow(
                    context.extensionUri,
                    client,
                    'dependency',
                    response
                );
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to get dependency graph: ${error}`);
            }
    });

    // Show Call Graph
    safeRegisterCommand('codegraph.showCallGraph', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('CodeGraph: No active editor');
                return;
            }

            try {
                const response = await client.sendRequest('workspace/executeCommand', {
                    command: 'codegraph.getCallGraph',
                    arguments: [{
                        uri: editor.document.uri.toString(),
                        position: {
                            line: editor.selection.active.line,
                            character: editor.selection.active.character,
                        },
                        direction: 'both',
                        depth: vscode.workspace.getConfiguration('codegraph')
                            .get<number>('visualization.defaultDepth', 3),
                        includeExternal: false,
                    }]
                }) as CallGraphResponse;

                GraphVisualizationPanel.createOrShow(
                    context.extensionUri,
                    client,
                    'call',
                    response
                );
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to get call graph: ${error}`);
            }
    });

    // Analyze Impact
    safeRegisterCommand('codegraph.analyzeImpact', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('CodeGraph: No active editor');
                return;
            }

            // Ask user for analysis type
            const analysisType = await vscode.window.showQuickPick(
                [
                    { label: 'Modify', value: 'modify', description: 'Impact if this symbol is modified' },
                    { label: 'Delete', value: 'delete', description: 'Impact if this symbol is deleted' },
                    { label: 'Rename', value: 'rename', description: 'Impact if this symbol is renamed' },
                ],
                { placeHolder: 'Select analysis type' }
            );

            if (!analysisType) {
                return;
            }

            try {
                const response = await client.sendRequest('workspace/executeCommand', {
                    command: 'codegraph.analyzeImpact',
                    arguments: [{
                        uri: editor.document.uri.toString(),
                        position: {
                            line: editor.selection.active.line,
                            character: editor.selection.active.character,
                        },
                        analysisType: analysisType.value as 'modify' | 'delete' | 'rename',
                    }]
                }) as ImpactAnalysisResponse;

                // Show impact analysis results
                showImpactAnalysisResults(response);
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to analyze impact: ${error}`);
            }
    });

    // Show Parser Metrics
    safeRegisterCommand('codegraph.showMetrics', async () => {
            try {
                const response = await client.sendRequest('workspace/executeCommand', {
                    command: 'codegraph.getParserMetrics',
                    arguments: []
                }) as ParserMetricsResponse;

                showMetricsPanel(response);
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to get metrics: ${error}`);
            }
    });

    // Open AI Chat
    safeRegisterCommand('codegraph.openAIChat', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('CodeGraph: No active editor');
                return;
            }

            // For now, just get context and show it
            // Full chat panel implementation would go here
            try {
                const context = await aiProvider.provideCodeContext(
                    editor.document,
                    editor.selection.active,
                    'explain'
                );

                // Create a new document with the context
                const doc = await vscode.workspace.openTextDocument({
                    language: 'markdown',
                    content: formatAIContext(context),
                });
                await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to get AI context: ${error}`);
            }
    });

    // Reindex Workspace
    safeRegisterCommand('codegraph.reindex', async () => {
            try {
                await vscode.window.withProgress(
                    {
                        location: vscode.ProgressLocation.Notification,
                        title: 'CodeGraph: Reindexing workspace...',
                        cancellable: false,
                    },
                    async () => {
                        await client.sendRequest('workspace/executeCommand', {
                            command: 'codegraph.reindexWorkspace',
                            arguments: []
                        });
                    }
                );
                vscode.window.showInformationMessage('CodeGraph: Workspace reindexed successfully');
            } catch (error) {
                vscode.window.showErrorMessage(`CodeGraph: Failed to reindex workspace: ${error}`);
            }
    });
}

/**
 * Show impact analysis results in an output panel
 */
function showImpactAnalysisResults(response: ImpactAnalysisResponse): void {
    const outputChannel = vscode.window.createOutputChannel('CodeGraph Impact Analysis');
    outputChannel.clear();

    outputChannel.appendLine('=== Impact Analysis Results ===\n');
    outputChannel.appendLine(`Summary:`);
    outputChannel.appendLine(`  Files Affected: ${response.summary.filesAffected}`);
    outputChannel.appendLine(`  Breaking Changes: ${response.summary.breakingChanges}`);
    outputChannel.appendLine(`  Warnings: ${response.summary.warnings}`);

    if (response.directImpact.length > 0) {
        outputChannel.appendLine('\n--- Direct Impact ---');
        for (const impact of response.directImpact) {
            const severityIcon = impact.severity === 'breaking' ? '🔴' :
                impact.severity === 'warning' ? '🟡' : '🔵';
            outputChannel.appendLine(`${severityIcon} ${impact.type}: ${impact.uri}`);
            outputChannel.appendLine(`   Line ${impact.range.start.line + 1}`);
        }
    }

    if (response.indirectImpact.length > 0) {
        outputChannel.appendLine('\n--- Indirect Impact ---');
        for (const impact of response.indirectImpact) {
            const severityIcon = impact.severity === 'breaking' ? '🔴' :
                impact.severity === 'warning' ? '🟡' : '🔵';
            outputChannel.appendLine(`${severityIcon} ${impact.uri}`);
            outputChannel.appendLine(`   Path: ${impact.path.join(' → ')}`);
        }
    }

    if (response.affectedTests.length > 0) {
        outputChannel.appendLine('\n--- Affected Tests ---');
        for (const test of response.affectedTests) {
            outputChannel.appendLine(`🧪 ${test.testName}`);
            outputChannel.appendLine(`   ${test.uri}`);
        }
    }

    outputChannel.show();
}

/**
 * Show parser metrics in an output panel
 */
function showMetricsPanel(response: ParserMetricsResponse): void {
    const outputChannel = vscode.window.createOutputChannel('CodeGraph Metrics');
    outputChannel.clear();

    outputChannel.appendLine('=== CodeGraph Parser Metrics ===\n');

    outputChannel.appendLine('Overall:');
    outputChannel.appendLine(`  Files Attempted: ${response.totals.filesAttempted}`);
    outputChannel.appendLine(`  Files Succeeded: ${response.totals.filesSucceeded}`);
    outputChannel.appendLine(`  Files Failed: ${response.totals.filesFailed}`);
    outputChannel.appendLine(`  Total Entities: ${response.totals.totalEntities}`);
    outputChannel.appendLine(`  Success Rate: ${(response.totals.successRate * 100).toFixed(1)}%`);

    outputChannel.appendLine('\nBy Language:');
    for (const metric of response.metrics) {
        outputChannel.appendLine(`\n  ${metric.language.toUpperCase()}:`);
        outputChannel.appendLine(`    Files: ${metric.filesSucceeded}/${metric.filesAttempted}`);
        outputChannel.appendLine(`    Entities: ${metric.totalEntities}`);
        outputChannel.appendLine(`    Relationships: ${metric.totalRelationships}`);
        outputChannel.appendLine(`    Parse Time: ${metric.totalParseTimeMs}ms (avg: ${metric.avgParseTimeMs}ms)`);
    }

    outputChannel.show();
}

/**
 * Format AI context for display
 */
function formatAIContext(context: {
    primary: { code: string; language: string; description: string };
    related: Array<{ code: string; relationship: string; relevance: number }>;
    architecture?: { module: string; neighbors: string[] };
}): string {
    let content = '# CodeGraph AI Context\n\n';

    content += `## Primary Code\n`;
    content += `*${context.primary.description}*\n\n`;
    content += '```' + context.primary.language + '\n';
    content += context.primary.code;
    content += '\n```\n\n';

    if (context.related.length > 0) {
        content += '## Related Code\n\n';
        for (const related of context.related.slice(0, 5)) {
            content += `### ${related.relationship} (relevance: ${(related.relevance * 100).toFixed(0)}%)\n`;
            content += '```\n';
            content += related.code;
            content += '\n```\n\n';
        }
    }

    if (context.architecture) {
        content += '## Architecture\n\n';
        content += `- **Module**: ${context.architecture.module}\n`;
        content += `- **Neighbors**: ${context.architecture.neighbors.join(', ')}\n`;
    }

    return content;
}
