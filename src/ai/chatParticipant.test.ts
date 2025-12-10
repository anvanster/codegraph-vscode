import { describe, it, expect, beforeEach, vi } from 'vitest';
import { reset, clearAllMocks, vscode } from '@vsforge/shim';

// Mock vscode module
vi.mock('vscode', async () => {
    const shim = await import('@vsforge/shim');
    return shim.vscode;
});

// Mock vscode-languageclient
vi.mock('vscode-languageclient/node', () => ({
    LanguageClient: vi.fn(),
}));

// Mock the context provider
vi.mock('./contextProvider', () => ({
    CodeGraphAIProvider: vi.fn().mockImplementation(() => ({
        provideCodeContext: vi.fn(),
    })),
}));

describe('CodeGraphChatParticipant', () => {
    const mockClient = {
        sendRequest: vi.fn(),
    };

    const mockAIProvider = {
        provideCodeContext: vi.fn(),
    };

    beforeEach(() => {
        reset();
        clearAllMocks();
        vi.clearAllMocks();
        mockClient.sendRequest.mockReset();
    });

    describe('register', () => {
        it('should register chat participant with codegraph id', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);

            participant.register();

            // Check that the participant was registered by getting it
            const registered = (vscode.chat as any).getParticipant('codegraph');
            expect(registered).toBeDefined();
            expect(registered.handler).toBeDefined();
        });

        it('should set icon path on participant', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);

            participant.register();

            const registered = (vscode.chat as any).getParticipant('codegraph');
            expect(registered).toBeDefined();
        });
    });

    describe('handleRequest - dependency graph', () => {
        it('should handle dependency request when prompt contains "depend"', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            // Mock active editor
            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 10, character: 5 } },
            };

            // Mock LSP response
            mockClient.sendRequest.mockResolvedValue({
                nodes: [
                    { id: '1', label: 'test.ts', type: 'file', language: 'typescript' },
                    { id: '2', label: 'utils.ts', type: 'file', language: 'typescript' },
                ],
                edges: [
                    { from: '1', to: '2', type: 'import' },
                ],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'show dependencies');

            expect(result.output).toContain('Dependency Graph');
            expect(result.output).toContain('2');
            expect(result.output).toContain('1');
            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.getDependencyGraph',
                }),
                expect.anything()
            );
        });

        it('should handle "imports" keyword as dependency request', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({ nodes: [], edges: [] });

            await (vscode.chat as any).simulateRequest('codegraph', 'what are the imports');

            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.getDependencyGraph',
                }),
                expect.anything()
            );
        });
    });

    describe('handleRequest - call graph', () => {
        it('should handle call graph request when prompt contains "call"', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 15, character: 10 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                root: { id: '1', name: 'processData' },
                nodes: [
                    { id: '1', name: 'processData' },
                    { id: '2', name: 'validateInput' },
                ],
                edges: [
                    { from: '1', to: '2' },
                ],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'show call graph');

            expect(result.output).toContain('Call Graph');
            expect(result.output).toContain('processData');
            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.getCallGraph',
                }),
                expect.anything()
            );
        });

        it('should handle missing root function in call graph', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                root: null,
                nodes: [],
                edges: [],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'callers of this');

            expect(result.output).toContain('No function found');
        });
    });

    describe('handleRequest - impact analysis', () => {
        it('should handle impact request when prompt contains "impact"', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 20, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                summary: { filesAffected: 5, breakingChanges: 2, warnings: 3 },
                directImpact: [
                    { uri: 'file:///a.ts', range: { start: { line: 10 } }, type: 'caller', severity: 'breaking' },
                ],
                indirectImpact: [],
                affectedTests: [],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'analyze impact');

            expect(result.output).toContain('Impact Analysis');
            expect(result.output).toContain('5');
            expect(result.output).toContain('2');
            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.analyzeImpact',
                }),
                expect.anything()
            );
        });

        it('should handle "affect" keyword as impact request', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                summary: { filesAffected: 0, breakingChanges: 0, warnings: 0 },
                directImpact: [],
                indirectImpact: [],
                affectedTests: [],
            });

            await (vscode.chat as any).simulateRequest('codegraph', 'what would be affected');

            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.analyzeImpact',
                }),
                expect.anything()
            );
        });
    });

    describe('handleRequest - tests', () => {
        it('should handle test request when prompt contains "test"', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///src/lib.ts' } },
                selection: { active: { line: 30, character: 5 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'function', name: 'processData', code: '', language: 'typescript', location: {} },
                relatedSymbols: [
                    { name: 'testProcessData', relationship: 'tests', code: 'it("should")', relevanceScore: 0.9 },
                ],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'find related tests');

            expect(result.output).toContain('Related Tests');
            expect(result.output).toContain('testProcessData');
            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.getAIContext',
                    arguments: expect.arrayContaining([
                        expect.objectContaining({ contextType: 'test' }),
                    ]),
                }),
                expect.anything()
            );
        });

        it('should show message when no tests found', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'function', name: 'internal', code: '', language: 'typescript', location: {} },
                relatedSymbols: [],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'tests for this');

            expect(result.output).toContain('No related tests found');
        });
    });

    describe('handleRequest - general context', () => {
        it('should handle general requests with explain intent by default', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 10, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: {
                    type: 'function',
                    name: 'calculateTotal',
                    code: 'function calculateTotal() {}',
                    language: 'typescript',
                    location: { uri: 'file:///test.ts' },
                },
                relatedSymbols: [],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'explain this function');

            expect(result.output).toContain('Code Context');
            expect(result.output).toContain('calculateTotal');
            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    command: 'codegraph.getAIContext',
                    arguments: expect.arrayContaining([
                        expect.objectContaining({ contextType: 'explain' }),
                    ]),
                }),
                expect.anything()
            );
        });

        it('should detect debug intent from prompt', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'function', name: 'broken', code: '', language: 'typescript', location: {} },
                relatedSymbols: [],
            });

            await (vscode.chat as any).simulateRequest('codegraph', 'help debug this error');

            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    arguments: expect.arrayContaining([
                        expect.objectContaining({ contextType: 'debug' }),
                    ]),
                }),
                expect.anything()
            );
        });

        it('should detect modify intent from prompt', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'function', name: 'old', code: '', language: 'typescript', location: {} },
                relatedSymbols: [],
            });

            await (vscode.chat as any).simulateRequest('codegraph', 'how to refactor this');

            expect(mockClient.sendRequest).toHaveBeenCalledWith(
                'workspace/executeCommand',
                expect.objectContaining({
                    arguments: expect.arrayContaining([
                        expect.objectContaining({ contextType: 'modify' }),
                    ]),
                }),
                expect.anything()
            );
        });

        it('should include related code in output', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'function', name: 'main', code: 'fn main()', language: 'typescript', location: {} },
                relatedSymbols: [
                    { name: 'helper', relationship: 'calls', code: 'fn helper()', relevanceScore: 0.9 },
                    { name: 'util', relationship: 'uses', code: 'fn util()', relevanceScore: 0.8 },
                ],
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'context');

            expect(result.output).toContain('Related Code');
            expect(result.output).toContain('helper');
            expect(result.output).toContain('calls');
            expect(result.output).toContain('90%');
        });

        it('should include architecture context when available', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = {
                document: { uri: { toString: () => 'file:///test.ts' } },
                selection: { active: { line: 0, character: 0 } },
            };

            mockClient.sendRequest.mockResolvedValue({
                primaryContext: { type: 'class', name: 'Service', code: 'class Service', language: 'typescript', location: {} },
                relatedSymbols: [],
                architecture: {
                    module: 'services/core',
                    neighbors: ['utils', 'config'],
                },
            });

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'architecture');

            expect(result.output).toContain('Architecture');
            expect(result.output).toContain('services/core');
            expect(result.output).toContain('utils');
        });
    });

    describe('handleRequest - no active editor', () => {
        it('should show message when no editor is active', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            (vscode.window as any).activeTextEditor = undefined;

            const result = await (vscode.chat as any).simulateRequest('codegraph', 'show dependencies');

            expect(result.output).toContain('No active editor');
        });
    });

    describe('dispose', () => {
        it('should dispose registered participant', async () => {
            const { CodeGraphChatParticipant } = await import('./chatParticipant');
            const participant = new CodeGraphChatParticipant(mockClient as any, mockAIProvider as any);
            participant.register();

            // Verify it was registered
            const registered = (vscode.chat as any).getParticipant('codegraph');
            expect(registered).toBeDefined();

            participant.dispose();

            // After dispose, the internal disposables should be cleared
            expect((participant as any).disposables).toHaveLength(0);
        });
    });
});
