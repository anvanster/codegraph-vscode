import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { CodeGraphAIProvider } from './contextProvider';
import {
    DependencyGraphResponse,
    CallGraphResponse,
    ImpactAnalysisResponse,
    AIContextResponse,
} from '../types';

/**
 * CodeGraph Chat Participant
 *
 * Registers a @codegraph chat participant that provides code intelligence
 * context to any AI chatbot operating in VS Code (Claude, Copilot, etc.).
 *
 * Usage in any AI chat:
 *   @codegraph explain this function
 *   @codegraph dependencies
 *   @codegraph impact analysis
 *   @codegraph call graph
 */
export class CodeGraphChatParticipant {
    private disposables: vscode.Disposable[] = [];

    constructor(
        private client: LanguageClient,
        private aiProvider: CodeGraphAIProvider
    ) {}

    /**
     * Register the @codegraph chat participant with VS Code.
     */
    register(): void {
        // Check if vscode.chat API exists (VS Code 1.90+)
        if (!(vscode as any).chat) {
            console.log('[CodeGraph] vscode.chat API not available - Chat Participant requires VS Code 1.90+');
            return;
        }

        const createParticipant = (vscode as any).chat.createChatParticipant;
        if (typeof createParticipant !== 'function') {
            console.log('[CodeGraph] vscode.chat.createChatParticipant not available');
            return;
        }

        console.log('[CodeGraph] Registering @codegraph chat participant...');

        try {
            const participant = createParticipant('codegraph', this.handleRequest.bind(this));

            participant.iconPath = new vscode.ThemeIcon('type-hierarchy');
            participant.followupProvider = {
                provideFollowups: this.provideFollowups.bind(this)
            };

            this.disposables.push(participant);
            console.log('[CodeGraph] @codegraph chat participant registered successfully');
        } catch (error) {
            console.error('[CodeGraph] Failed to register chat participant:', error);
        }
    }

    /**
     * Handle chat requests to @codegraph
     */
    private async handleRequest(
        request: vscode.ChatRequest,
        context: vscode.ChatContext,
        stream: vscode.ChatResponseStream,
        token: vscode.CancellationToken
    ): Promise<vscode.ChatResult> {
        const prompt = request.prompt.toLowerCase();
        const editor = vscode.window.activeTextEditor;

        // Provide context about current file if available
        if (editor) {
            const uri = editor.document.uri.toString();
            const position = editor.selection.active;

            try {
                // Determine what type of context to provide based on the request
                if (prompt.includes('depend') || prompt.includes('import')) {
                    await this.handleDependencyRequest(stream, uri, token);
                } else if (prompt.includes('call') || prompt.includes('caller') || prompt.includes('callee')) {
                    await this.handleCallGraphRequest(stream, uri, position, token);
                } else if (prompt.includes('impact') || prompt.includes('affect') || prompt.includes('change')) {
                    await this.handleImpactRequest(stream, uri, position, token);
                } else if (prompt.includes('test')) {
                    await this.handleTestRequest(stream, uri, position, token);
                } else {
                    // Default: provide general AI context
                    await this.handleContextRequest(stream, uri, position, prompt, token);
                }
            } catch (error) {
                stream.markdown(`\n\n**Error:** ${error}\n`);
            }
        } else {
            stream.markdown('No active editor. Please open a file to get code context.\n');
        }

        return { metadata: { command: 'codegraph' } };
    }

    /**
     * Handle dependency graph requests
     */
    private async handleDependencyRequest(
        stream: vscode.ChatResponseStream,
        uri: string,
        token: vscode.CancellationToken
    ): Promise<void> {
        stream.progress('Analyzing dependencies...');

        const response = await this.client.sendRequest('workspace/executeCommand', {
            command: 'codegraph.getDependencyGraph',
            arguments: [{
                uri,
                depth: 3,
                includeExternal: false,
                direction: 'both',
            }]
        }, token) as DependencyGraphResponse;

        stream.markdown('## Dependency Graph\n\n');
        stream.markdown(`Found **${response.nodes.length}** files/modules with **${response.edges.length}** dependencies.\n\n`);

        if (response.edges.length > 0) {
            stream.markdown('### Dependencies\n\n');
            const imports = response.edges.slice(0, 10);
            for (const edge of imports) {
                const fromNode = response.nodes.find(n => n.id === edge.from);
                const toNode = response.nodes.find(n => n.id === edge.to);
                stream.markdown(`- \`${fromNode?.label || edge.from}\` → \`${toNode?.label || edge.to}\`\n`);
            }
            if (response.edges.length > 10) {
                stream.markdown(`\n*...and ${response.edges.length - 10} more dependencies*\n`);
            }
        }
    }

    /**
     * Handle call graph requests
     */
    private async handleCallGraphRequest(
        stream: vscode.ChatResponseStream,
        uri: string,
        position: vscode.Position,
        token: vscode.CancellationToken
    ): Promise<void> {
        stream.progress('Analyzing call graph...');

        const response = await this.client.sendRequest('workspace/executeCommand', {
            command: 'codegraph.getCallGraph',
            arguments: [{
                uri,
                position: {
                    line: position.line,
                    character: position.character,
                },
                depth: 3,
                direction: 'both',
                includeExternal: false,
            }]
        }, token) as CallGraphResponse;

        stream.markdown('## Call Graph\n\n');

        if (!response.root) {
            stream.markdown('No function found at cursor position. Place cursor on a function definition.\n');
            return;
        }

        stream.markdown(`**Target:** \`${response.root.name}\`\n\n`);

        const callers = response.edges.filter(e => e.to === response.root!.id);
        const callees = response.edges.filter(e => e.from === response.root!.id);

        if (callers.length > 0) {
            stream.markdown('### Callers (functions that call this)\n');
            for (const edge of callers.slice(0, 5)) {
                const caller = response.nodes.find(n => n.id === edge.from);
                if (caller) {
                    stream.markdown(`- \`${caller.name}\`\n`);
                }
            }
            stream.markdown('\n');
        }

        if (callees.length > 0) {
            stream.markdown('### Callees (functions this calls)\n');
            for (const edge of callees.slice(0, 5)) {
                const callee = response.nodes.find(n => n.id === edge.to);
                if (callee) {
                    stream.markdown(`- \`${callee.name}\`\n`);
                }
            }
        }
    }

    /**
     * Handle impact analysis requests
     */
    private async handleImpactRequest(
        stream: vscode.ChatResponseStream,
        uri: string,
        position: vscode.Position,
        token: vscode.CancellationToken
    ): Promise<void> {
        stream.progress('Analyzing impact...');

        const response = await this.client.sendRequest('workspace/executeCommand', {
            command: 'codegraph.analyzeImpact',
            arguments: [{
                uri,
                position: {
                    line: position.line,
                    character: position.character,
                },
                analysisType: 'modify',
            }]
        }, token) as ImpactAnalysisResponse;

        stream.markdown('## Impact Analysis\n\n');
        stream.markdown(`| Metric | Count |\n|--------|-------|\n`);
        stream.markdown(`| Files Affected | ${response.summary.filesAffected} |\n`);
        stream.markdown(`| Breaking Changes | ${response.summary.breakingChanges} |\n`);
        stream.markdown(`| Warnings | ${response.summary.warnings} |\n\n`);

        if (response.directImpact.length > 0) {
            stream.markdown('### Direct Impact\n');
            for (const impact of response.directImpact.slice(0, 5)) {
                const severity = impact.severity === 'breaking' ? '🔴' :
                                impact.severity === 'warning' ? '🟡' : '🔵';
                const fileName = vscode.Uri.parse(impact.uri).path.split('/').pop();
                stream.markdown(`${severity} \`${fileName}:${impact.range.start.line + 1}\` - ${impact.type}\n`);
            }
        }

        if (response.affectedTests.length > 0) {
            stream.markdown('\n### Affected Tests\n');
            for (const test of response.affectedTests.slice(0, 3)) {
                stream.markdown(`🧪 \`${test.testName}\`\n`);
            }
        }
    }

    /**
     * Handle test-related requests
     */
    private async handleTestRequest(
        stream: vscode.ChatResponseStream,
        uri: string,
        position: vscode.Position,
        token: vscode.CancellationToken
    ): Promise<void> {
        stream.progress('Finding related tests...');

        const response = await this.client.sendRequest('workspace/executeCommand', {
            command: 'codegraph.getAIContext',
            arguments: [{
                uri,
                position: {
                    line: position.line,
                    character: position.character,
                },
                contextType: 'test',
                maxTokens: 2000,
            }]
        }, token) as AIContextResponse;

        stream.markdown('## Related Tests\n\n');

        const tests = response.relatedSymbols.filter(s =>
            s.relationship.toLowerCase().includes('test') ||
            s.name.toLowerCase().includes('test')
        );

        if (tests.length === 0) {
            stream.markdown('No related tests found. Consider adding tests for this code.\n');
        } else {
            for (const test of tests.slice(0, 5)) {
                stream.markdown(`### \`${test.name}\`\n`);
                stream.markdown(`*${test.relationship}* (relevance: ${(test.relevanceScore * 100).toFixed(0)}%)\n\n`);
                stream.markdown('```\n' + test.code.substring(0, 500) + '\n```\n\n');
            }
        }
    }

    /**
     * Handle general context requests (default)
     */
    private async handleContextRequest(
        stream: vscode.ChatResponseStream,
        uri: string,
        position: vscode.Position,
        prompt: string,
        token: vscode.CancellationToken
    ): Promise<void> {
        // Determine intent from prompt
        let intent: 'explain' | 'modify' | 'debug' | 'test' = 'explain';
        if (prompt.includes('debug') || prompt.includes('fix') || prompt.includes('error')) {
            intent = 'debug';
        } else if (prompt.includes('modify') || prompt.includes('change') || prompt.includes('refactor')) {
            intent = 'modify';
        } else if (prompt.includes('test')) {
            intent = 'test';
        }

        stream.progress(`Getting ${intent} context...`);

        const response = await this.client.sendRequest('workspace/executeCommand', {
            command: 'codegraph.getAIContext',
            arguments: [{
                uri,
                position: {
                    line: position.line,
                    character: position.character,
                },
                contextType: intent,
                maxTokens: 4000,
            }]
        }, token) as AIContextResponse;

        // Stream the primary context
        stream.markdown('## Code Context\n\n');
        stream.markdown(`**${response.primaryContext.type}:** \`${response.primaryContext.name}\`\n\n`);
        stream.markdown('```' + response.primaryContext.language + '\n');
        stream.markdown(response.primaryContext.code + '\n');
        stream.markdown('```\n\n');

        // Stream related code
        if (response.relatedSymbols.length > 0) {
            stream.markdown('## Related Code\n\n');
            for (const symbol of response.relatedSymbols.slice(0, 3)) {
                stream.markdown(`### ${symbol.relationship}\n`);
                stream.markdown(`\`${symbol.name}\` (relevance: ${(symbol.relevanceScore * 100).toFixed(0)}%)\n\n`);
                stream.markdown('```\n' + symbol.code.substring(0, 300) + '\n```\n\n');
            }
        }

        // Architecture context
        if (response.architecture) {
            stream.markdown('## Architecture\n\n');
            stream.markdown(`- **Module:** ${response.architecture.module}\n`);
            stream.markdown(`- **Neighbors:** ${response.architecture.neighbors.join(', ')}\n`);
        }
    }

    /**
     * Provide follow-up suggestions after a response
     */
    private provideFollowups(
        _result: vscode.ChatResult,
        _context: vscode.ChatContext,
        _token: vscode.CancellationToken
    ): vscode.ChatFollowup[] {
        return [
            {
                prompt: 'Show me the dependency graph',
                label: 'Dependencies',
                command: 'dependencies'
            },
            {
                prompt: 'Analyze the call graph',
                label: 'Call Graph',
                command: 'callgraph'
            },
            {
                prompt: 'What would be affected if I change this?',
                label: 'Impact Analysis',
                command: 'impact'
            },
            {
                prompt: 'Find related tests',
                label: 'Tests',
                command: 'tests'
            }
        ];
    }

    /**
     * Dispose all registrations
     */
    dispose(): void {
        console.log('[CodeGraph] Disposing chat participant');
        this.disposables.forEach(d => d.dispose());
        this.disposables = [];
    }
}
