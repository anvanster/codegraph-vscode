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
import { getServerPath } from './server';

let client: LanguageClient;
let aiProvider: CodeGraphAIProvider;

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

    // Register commands, tree providers, etc.
    registerCommands(context, client, aiProvider);
    registerTreeDataProviders(context, client);

    // Add client to disposables
    context.subscriptions.push(client);

    // Set context for conditional UI
    vscode.commands.executeCommand('setContext', 'codegraph.enabled', true);
}

export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
    }
}
