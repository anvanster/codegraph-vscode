# CodeGraph AI Integration Analysis

## Executive Summary

The CodeGraph extension currently has a **basic AI context provider implementation** but is **NOT integrated** with VS Code's native AI APIs (Language Model API, Chat Participants). AI agents like GitHub Copilot and Claude **cannot auto-discover** the extension's capabilities.

## Current Implementation Status

### ✅ What's Implemented

1. **CodeGraphAIProvider Class** ([src/ai/contextProvider.ts](../src/ai/contextProvider.ts))
   - Fetches AI-optimized context from LSP server
   - Supports different intents: 'explain', 'modify', 'debug', 'test'
   - Formats context with primary code, related code, and architecture info
   - Respects `codegraph.ai.maxContextTokens` setting (default: 4000)

2. **LSP Server AI Context Handler** ([server/src/handlers/ai_context.rs](../server/src/handlers/ai_context.rs))
   - `codegraph/getAIContext` LSP method
   - Smart context selection based on intent
   - Token budget management
   - Relationship scoring (relevance calculation)
   - Includes: direct dependencies, callers, callees, inheritance, related tests

3. **Manual AI Chat Command** ([src/commands/index.ts](../src/commands/index.ts#L188))
   - `codegraph.openAIChat` command
   - Opens formatted context in a new markdown document
   - User must manually copy to AI assistant

### ❌ What's NOT Implemented

1. **VS Code Language Model API Integration**
   - No `vscode.lm.selectChatModels()` usage
   - Cannot access Claude, GPT-4, or other VS Code AI models
   - Cannot send messages directly to AI

2. **Chat Participant Registration**
   - No `vscode.chat.createChatParticipant()` integration
   - No `@codegraph` chat participant for inline queries
   - Cannot respond to AI chat messages

3. **Chat Variable Resolver**
   - No `#codegraph` variable for referencing context in chat
   - Cannot be invoked via `#codegraph:dependencies` or similar

4. **Tool/Function Calling**
   - No tool registration for AI agents to discover capabilities
   - AI cannot automatically trigger dependency graphs, call graphs, etc.

5. **Context Provider Registration**
   - No integration with GitHub Copilot's context API
   - No registration with VS Code's workspace context

## How AI Agents SHOULD Discover Extensions

### VS Code's AI Integration APIs (VS Code 1.90+)

#### 1. **Language Model API** (Proposed API)
```typescript
// Send messages to AI models directly
const models = await vscode.lm.selectChatModels({ family: 'claude-3' });
const model = models[0];

const messages = [
    vscode.LanguageModelChatMessage.User('Explain this code with full context')
];

const response = await model.sendRequest(messages, {}, token);
```

#### 2. **Chat Participant API** (Stable in VS Code 1.90+)
```typescript
// Register @codegraph participant
const participant = vscode.chat.createChatParticipant('codegraph', async (request, context, stream, token) => {
    // Handle @codegraph queries
    if (request.command === 'dependencies') {
        const graph = await getDependencyGraph(request.location);
        stream.markdown(`\`\`\`mermaid\n${graphToMermaid(graph)}\n\`\`\``);
    }
});

// User can type: "@codegraph show me the dependencies of this file"
```

#### 3. **Chat Variable Resolver** (VS Code 1.90+)
```typescript
// Register #codegraph variable
vscode.chat.registerVariable('codegraph', 'CodeGraph context', async (request, context, stream, token) => {
    const editor = vscode.window.activeTextEditor;
    const aiContext = await aiProvider.provideCodeContext(
        editor.document,
        editor.selection.active,
        'explain'
    );

    return [{
        level: vscode.ChatVariableLevel.Full,
        value: formatContextForChat(aiContext),
        description: 'Code context from CodeGraph'
    }];
});

// User can type: "Explain this code #codegraph"
```

#### 4. **Language Model Tools** (Proposed API)
```typescript
// Register tools that AI can call
const tool = vscode.lm.registerTool('codegraph-dependency-graph', {
    name: 'get_dependency_graph',
    description: 'Get the dependency graph for a file',
    inputSchema: {
        type: 'object',
        properties: {
            uri: { type: 'string', description: 'File URI' },
            depth: { type: 'number', description: 'Graph depth' }
        }
    }
}, async (input, token) => {
    const response = await client.sendRequest('workspace/executeCommand', {
        command: 'codegraph.getDependencyGraph',
        arguments: [{ uri: input.uri, depth: input.depth }]
    });

    return {
        content: JSON.stringify(response, null, 2)
    };
});

// AI can now call: get_dependency_graph({ uri: "file:///...", depth: 3 })
```

## Current User Experience Gap

### Today's Flow (Manual, Clunky)
1. User opens file in VS Code
2. User runs command: "CodeGraph: Open AI Assistant"
3. Extension generates context and opens it in a new document
4. User manually copies the context
5. User pastes into GitHub Copilot Chat or Claude Code
6. User asks their question
7. AI responds without ability to call back to CodeGraph

### Ideal Flow (Automated, Integrated)
```
User: "@codegraph explain how this function is called across the codebase"
      ↓
VS Code Chat: Routes to @codegraph participant
      ↓
CodeGraph: Executes call graph analysis
      ↓
CodeGraph: Returns formatted response with interactive graph
      ↓
User: "Show me the dependency graph"
      ↓
AI: Calls get_dependency_graph tool automatically
      ↓
CodeGraph: Returns graph, rendered in chat
```

## GitHub Copilot Integration

### Copilot Chat Extensions API
GitHub Copilot **does** expose an extensions API for context providers:

```typescript
// Copilot Chat API (requires github.copilot-chat extension)
interface CopilotChatParticipant {
    // Copilot can query your extension for context
    provideContext(request: CopilotContextRequest): Promise<CopilotContext>;
}

// Check if Copilot is available
const copilot = vscode.extensions.getExtension('github.copilot-chat');
if (copilot?.isActive) {
    // Register with Copilot's internal API
    // Note: This API is not publicly documented
}
```

However, GitHub has not publicly documented this API. Most extensions use:
1. **Chat Participants** (VS Code native)
2. **Chat Variables** (VS Code native)
3. **Language Model Tools** (for function calling)

## Implementation Roadmap

### Phase 1: Chat Participant (High Priority)
**Effort**: Medium | **Impact**: High | **User Adoption**: Immediate

Implement `@codegraph` chat participant for inline queries:
- `@codegraph explain` - Explain code with full context
- `@codegraph dependencies` - Show dependency graph
- `@codegraph callgraph` - Show call hierarchy
- `@codegraph impact` - Analyze change impact
- `@codegraph tests` - Find related tests

**Benefits**:
- Natural language interaction
- No manual context copying
- Works with any AI (Copilot, Claude, etc.)
- Discoverable in VS Code chat interface

### Phase 2: Chat Variable Resolver (Medium Priority)
**Effort**: Low | **Impact**: Medium | **User Adoption**: Gradual

Implement `#codegraph` variable for context injection:
- Users can reference: `#codegraph` in any chat message
- Automatically includes relevant context
- Works alongside other variables like `#file`, `#selection`

**Benefits**:
- Seamless integration with existing chat
- Passive context enhancement
- Minimal user learning curve

### Phase 3: Language Model Tools (High Priority)
**Effort**: Medium | **Impact**: Very High | **Agentic Workflows**: Enabled

Register CodeGraph capabilities as callable tools:
```typescript
Tools:
- get_dependency_graph(uri, depth, includeExternal)
- get_call_graph(uri, position, depth, direction)
- analyze_impact(uri, position, changeType)
- get_ai_context(uri, position, intent, maxTokens)
- find_related_tests(uri, position)
- get_symbol_info(uri, position)
```

**Benefits**:
- AI can autonomously use CodeGraph
- Enable multi-step reasoning workflows
- Future-proof for agentic AI evolution

### Phase 4: Direct Language Model Integration (Future)
**Effort**: High | **Impact**: Very High | **Requires**: VS Code 1.90+

Send requests directly to AI models with CodeGraph context:
- Intercept user questions
- Automatically enrich with graph context
- Send to Claude/GPT-4 via Language Model API
- Stream responses back to user

**Benefits**:
- Fully automated experience
- Highest quality responses
- No user intervention needed

## API Compatibility Matrix

| Feature | VS Code Version | API Status | Adoption |
|---------|----------------|------------|----------|
| Chat Participants | 1.90+ | Stable | ✅ High |
| Chat Variables | 1.90+ | Stable | ✅ High |
| Language Model API | 1.90+ | Proposed | ⚠️ Medium |
| Language Model Tools | Not Released | Proposed | ❌ None |

## Recommended Next Steps

1. **Immediate (Week 1-2)**:
   - Implement Chat Participant (`@codegraph`)
   - Update VS Code engine requirement to `^1.90.0`
   - Add chat participant to package.json contributions
   - Test with GitHub Copilot and Claude Code

2. **Short-term (Week 3-4)**:
   - Implement Chat Variable Resolver (`#codegraph`)
   - Add tool registration preparation (even if API not stable)
   - Update documentation with AI integration examples
   - Create demo videos showing AI integration

3. **Medium-term (Month 2-3)**:
   - Monitor VS Code's Language Model Tools API stabilization
   - Implement tool registration when API is available
   - Add direct Language Model API integration (if needed)
   - Gather user feedback and iterate

## Technical Requirements

### Dependencies
```json
{
  "engines": {
    "vscode": "^1.90.0"  // Updated from ^1.85.0
  },
  "dependencies": {
    "@vscode/chat-extension-utils": "^1.0.0"  // Optional helper library
  }
}
```

### Package.json Contributions
```json
{
  "contributes": {
    "chatParticipants": [
      {
        "id": "codegraph",
        "name": "CodeGraph",
        "description": "Code intelligence powered by graph analysis",
        "commands": [
          {
            "name": "explain",
            "description": "Explain code with full context"
          },
          {
            "name": "dependencies",
            "description": "Show dependency graph"
          },
          {
            "name": "callgraph",
            "description": "Show call hierarchy"
          },
          {
            "name": "impact",
            "description": "Analyze change impact"
          }
        ]
      }
    ],
    "chatVariables": [
      {
        "id": "codegraph",
        "name": "codegraph",
        "description": "Include CodeGraph context",
        "modelDescription": "Code context from dependency and call graph analysis"
      }
    ]
  }
}
```

## References

- [VS Code Chat API Documentation](https://code.visualstudio.com/api/extension-guides/chat)
- [VS Code Language Model API (Proposed)](https://github.com/microsoft/vscode/blob/main/src/vscode-dts/vscode.proposed.lmTools.d.ts)
- [Building Chat Extensions](https://code.visualstudio.com/api/extension-guides/chat)
- [GitHub Copilot Extensions](https://github.com/features/copilot/extensions)

## Conclusion

The CodeGraph extension has excellent underlying AI context infrastructure but **lacks the integration layer** to make it discoverable and usable by AI agents. Implementing **Chat Participants** and **Language Model Tools** will transform CodeGraph from a manual context provider into an autonomous AI-powered code intelligence system.

**Priority**: Implement Phase 1 (Chat Participant) immediately to unlock AI agent integration.
