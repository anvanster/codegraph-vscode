import * as os from 'os';
import * as path from 'path';
import * as fs from 'fs';
import * as vscode from 'vscode';

/**
 * Get the path to the LSP server binary for the current platform.
 */
export function getServerPath(context: vscode.ExtensionContext): string {
    const platform = os.platform();
    const arch = os.arch();

    let binaryName: string;

    switch (platform) {
        case 'linux':
            binaryName = 'codegraph-lsp-linux-x64';
            break;
        case 'darwin':
            binaryName = arch === 'arm64'
                ? 'codegraph-lsp-darwin-arm64'
                : 'codegraph-lsp-darwin-x64';
            break;
        case 'win32':
            binaryName = 'codegraph-lsp-win32-x64.exe';
            break;
        default:
            throw new Error(`Unsupported platform: ${platform}`);
    }

    // First, try the packaged binary path (for production)
    const packagedPath = context.asAbsolutePath(path.join('bin', binaryName));
    if (fs.existsSync(packagedPath)) {
        return packagedPath;
    }

    // For development, try the cargo target directory (workspace root)
    const targetPath = context.asAbsolutePath(
        path.join('target', 'release', 'codegraph-lsp')
    );
    if (fs.existsSync(targetPath)) {
        return targetPath;
    }

    // Windows development path
    const targetPathExe = context.asAbsolutePath(
        path.join('target', 'release', 'codegraph-lsp.exe')
    );
    if (fs.existsSync(targetPathExe)) {
        return targetPathExe;
    }

    // Debug build for development
    const debugPath = context.asAbsolutePath(
        path.join('target', 'debug', 'codegraph-lsp')
    );
    if (fs.existsSync(debugPath)) {
        return debugPath;
    }

    // Windows debug build
    const debugPathExe = context.asAbsolutePath(
        path.join('target', 'debug', 'codegraph-lsp.exe')
    );
    if (fs.existsSync(debugPathExe)) {
        return debugPathExe;
    }

    throw new Error(
        `CodeGraph LSP server binary not found. Expected at: ${packagedPath}\n` +
        `For development, build with: cargo build --release`
    );
}
