import * as vscode from 'vscode';
import * as path from 'path';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';
import { registerCommands } from './commands';
import { registerTreeDataProviders } from './views/treeProviders';
import { CodeGraphAIProvider } from './ai/contextProvider';
import { CodeGraphToolManager } from './ai/toolManager';
import { CodeGraphChatParticipant } from './ai/chatParticipant';
import { getServerPath } from './server';

let client: LanguageClient;
let aiProvider: CodeGraphAIProvider;
let toolManager: CodeGraphToolManager;
let chatParticipant: CodeGraphChatParticipant;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    const config = vscode.workspace.getConfiguration('codegraph');

    if (!config.get<boolean>('enabled', true)) {
        return;
    }

    // Determine server binary path
    const serverModule = getServerPath(context);

    // Server options
    const serverOptions: ServerOptions = {
        command: serverModule,
        args: ['--stdio'],
        transport: TransportKind.stdio,
    };

    // Client options
    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'python' },
            { scheme: 'file', language: 'rust' },
            { scheme: 'file', language: 'typescript' },
            { scheme: 'file', language: 'javascript' },
            { scheme: 'file', language: 'typescriptreact' },
            { scheme: 'file', language: 'javascriptreact' },
            { scheme: 'file', language: 'go' },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*'),
        },
        outputChannel: vscode.window.createOutputChannel('CodeGraph'),
        traceOutputChannel: vscode.window.createOutputChannel('CodeGraph Trace'),
    };

    // Create the language client
    client = new LanguageClient(
        'codegraph',
        'CodeGraph Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client
    try {
        await client.start();
        vscode.window.showInformationMessage('CodeGraph: Language server started');
    } catch (error) {
        vscode.window.showErrorMessage(`CodeGraph: Failed to start language server: ${error}`);
        return;
    }

    // Create AI context provider
    aiProvider = new CodeGraphAIProvider(client);

    // Register Language Model Tools for autonomous AI agent access
    try {
        toolManager = new CodeGraphToolManager(client);
        toolManager.registerTools();
        console.log('[CodeGraph] AI tools registered and available to AI agents');
    } catch (error) {
        console.error('[CodeGraph] Failed to register Language Model Tools:', error);
        vscode.window.showWarningMessage(`CodeGraph: Could not register AI tools: ${error}`);
        // Continue activation even if tool registration fails
    }

    // Register @codegraph chat participant for AI chatbot context
    try {
        chatParticipant = new CodeGraphChatParticipant(client, aiProvider);
        chatParticipant.register();
        console.log('[CodeGraph] @codegraph chat participant available');
    } catch (error) {
        console.error('[CodeGraph] Failed to register chat participant:', error);
        // Continue activation even if chat participant fails
    }

    // Register commands, tree providers, etc.
    registerCommands(context, client, aiProvider);
    registerTreeDataProviders(context, client);

    // Add debug command to verify tool registration
    context.subscriptions.push(
        vscode.commands.registerCommand('codegraph.debugTools', async () => {
            try {
                // Check if vscode.lm exists
                if (!(vscode as any).lm) {
                    vscode.window.showErrorMessage('❌ vscode.lm API not available. VS Code version may be too old (need 1.90+)');
                    return;
                }

                // Get all registered tools (API might be different)
                const lmApi = (vscode as any).lm;
                let allTools: any[] = [];

                // Try to get tools
                if (typeof lmApi.tools === 'function') {
                    allTools = await lmApi.tools();
                } else if (Array.isArray(lmApi.tools)) {
                    allTools = lmApi.tools;
                } else {
                    vscode.window.showWarningMessage('Unable to access vscode.lm.tools - API shape unknown');
                }

                const codegraphTools = allTools.filter(t => t && t.name && t.name.startsWith('codegraph_'));

                // Show results
                const message = [
                    '📊 CodeGraph Tools Debug Info:',
                    `VS Code version: ${vscode.version}`,
                    `Total LM tools: ${allTools.length}`,
                    `CodeGraph tools: ${codegraphTools.length}`,
                    '',
                    codegraphTools.length > 0 ? 'CodeGraph tools found:' : 'No CodeGraph tools found',
                    ...codegraphTools.map(t => `  ✓ ${t.name}`)
                ].join('\n');

                vscode.window.showInformationMessage(message, { modal: true });

                // Also log to console
                console.log('=== CodeGraph Tools Debug ===');
                console.log('VS Code version:', vscode.version);
                console.log('All tools:', allTools.map(t => t?.name || 'unnamed'));
                console.log('CodeGraph tools:', codegraphTools.map(t => t.name));
                console.log('Tool manager instance:', toolManager);
                console.log('Tool manager disposables count:', (toolManager as any).disposables?.length);
            } catch (error) {
                vscode.window.showErrorMessage(`Error checking tools: ${error}`);
                console.error('Debug tools error:', error);
            }
        })
    );

    // Add to disposables
    context.subscriptions.push(client, toolManager, chatParticipant);

    // Set context for conditional UI
    vscode.commands.executeCommand('setContext', 'codegraph.enabled', true);
}

export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
    }
}
