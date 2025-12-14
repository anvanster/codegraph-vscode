# CodeGraph AI Tool Examples

This document provides detailed examples of how AI agents can use CodeGraph tools effectively. These tools are registered via VS Code's Language Model Tools API and are available to any AI assistant (GitHub Copilot, Claude, etc.) operating in VS Code.

---

## Why Use CodeGraph Tools?

CodeGraph tools provide **semantic understanding** rather than text matching. This leads to:

- **75-80% fewer tool calls** compared to grep/read operations
- **75-78% reduction in tokens** consumed
- **Higher accuracy** — no false positives from text matching
- **Intent-aware context** — get relevant code based on what you're trying to do

---

## Tool Reference

### 1. `codegraph_get_dependency_graph`

**Purpose:** Understand what files/modules a piece of code depends on, or what depends on it.

**When to use:**
- Understanding module architecture
- Planning refactoring scope
- Finding circular dependencies
- Analyzing import chains

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file to analyze
  "depth": 3,                           // Optional: traversal depth (1-10)
  "includeExternal": false,             // Optional: include node_modules
  "direction": "both"                   // Optional: "imports", "dependents", or "both"
}
```

**Example — "What does UserService depend on?"**

```
Tool: codegraph_get_dependency_graph
Input: {
  "uri": "file:///project/src/services/UserService.ts",
  "depth": 2,
  "direction": "imports"
}
```

**Output:**
```markdown
# Dependency Graph

Found 8 files/modules with 12 dependencies.

## Dependencies (12)
- UserService.ts → DatabaseClient.ts (import)
- UserService.ts → Logger.ts (import)
- UserService.ts → UserRepository.ts (import)
- UserService.ts → ValidationUtils.ts (import)
- UserRepository.ts → DatabaseClient.ts (import)
- UserRepository.ts → UserModel.ts (import)
...

## Files/Modules
- **UserService.ts** (module, typescript)
- **DatabaseClient.ts** (module, typescript)
- **Logger.ts** (module, typescript)
...
```

**Comparison — Traditional Approach:**
```
Without CodeGraph:
1. read_file UserService.ts (top 50 lines for imports)
2. grep_search for each import to find file location
3. read_file each dependency
4. Repeat for transitive dependencies

= 8-12 tool calls, ~6,000-8,000 tokens

With CodeGraph:
1. codegraph_get_dependency_graph

= 1 tool call, ~500 tokens
```

---

### 2. `codegraph_get_call_graph`

**Purpose:** Understand function call relationships — what calls a function, and what it calls.

**When to use:**
- Tracing execution flow
- Understanding function usage patterns
- Planning function signature changes
- Debugging call chains

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file containing function
  "line": 45,                           // Required: line number (0-indexed)
  "character": 0,                       // Optional: character position
  "depth": 3,                           // Optional: traversal depth
  "direction": "both"                   // Optional: "callers", "callees", or "both"
}
```

**Example — "What functions call processPayment?"**

```
Tool: codegraph_get_call_graph
Input: {
  "uri": "file:///project/src/payments/PaymentProcessor.ts",
  "line": 120,
  "direction": "callers",
  "depth": 2
}
```

**Output:**
```markdown
# Call Graph

Found 5 functions with 7 call relationships.

## Target Function
**processPayment** (async processPayment(order: Order, method: PaymentMethod): Promise<Receipt>)
Location: file:///project/src/payments/PaymentProcessor.ts

## Callers (3)
Functions that call this:
- **checkout** at src/checkout/CheckoutService.ts
- **retryPayment** at src/orders/OrderService.ts
- **processRefund** at src/payments/RefundHandler.ts
```

**Comparison — Traditional Approach:**
```
Without CodeGraph:
1. grep_search for "processPayment(" across codebase
2. Filter out definition, keep only usages
3. read_file surrounding context for each match
4. Manually trace call chains

= 5-7 tool calls, ~5,000-7,000 tokens

With CodeGraph:
1. codegraph_get_call_graph

= 1 tool call, ~400-600 tokens
```

---

### 3. `codegraph_analyze_impact`

**Purpose:** Before making changes, understand what will break. Shows direct impacts, indirect impacts, and affected tests.

**When to use:**
- Before refactoring
- Before deleting code
- Before renaming symbols
- Assessing change risk

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file containing symbol
  "line": 45,                           // Required: line number (0-indexed)
  "character": 0,                       // Optional: character position
  "changeType": "modify"                // Optional: "modify", "delete", or "rename"
}
```

**Example — "What breaks if I delete validateInput?"**

```
Tool: codegraph_analyze_impact
Input: {
  "uri": "file:///project/src/utils/validation.ts",
  "line": 23,
  "changeType": "delete"
}
```

**Output:**
```markdown
# Impact Analysis

## Summary
- Files Affected: 4
- Breaking Changes: 6
- Warnings: 2

## Direct Impact (6)
Immediate usages that will be affected:
🔴 BREAKING: **reference** at UserService.ts:45
🔴 BREAKING: **reference** at OrderService.ts:89
🔴 BREAKING: **reference** at PaymentProcessor.ts:34
🔴 BREAKING: **reference** at CheckoutController.ts:67
🔴 BREAKING: **reference** at ApiHandler.ts:123
🔴 BREAKING: **reference** at FormValidator.ts:12

## Indirect Impact (2)
Transitive dependencies that will be affected:
🟡 CheckoutFlow.ts
  Dependency path: CheckoutFlow → CheckoutController → validateInput
🟡 OrderWorkflow.ts
  Dependency path: OrderWorkflow → OrderService → validateInput

## Affected Tests (3)
Tests that may need updating:
🧪 **validateInput.test.ts** at tests/utils/validateInput.test.ts
🧪 **UserService.test.ts** at tests/services/UserService.test.ts
🧪 **integration.test.ts** at tests/integration/integration.test.ts
```

**Comparison — Traditional Approach:**
```
Without CodeGraph:
1. grep_search for "validateInput" usage
2. read_file surrounding context for each match
3. Manually identify which are breaking vs warnings
4. grep_search in test files
5. Manually trace indirect impacts

= 5-8 tool calls, ~4,000-5,000 tokens, manual analysis required

With CodeGraph:
1. codegraph_analyze_impact

= 1 tool call, ~500-1,200 tokens, automatic categorization
```

---

### 4. `codegraph_get_ai_context`

**Purpose:** Get comprehensive code context optimized for AI analysis. Automatically selects relevant related code based on your intent.

**When to use:**
- Understanding unfamiliar code
- Before modifying code
- Debugging issues
- Writing tests

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file to analyze
  "line": 45,                           // Required: line number (0-indexed)
  "character": 0,                       // Optional: character position
  "intent": "explain",                  // Optional: "explain", "modify", "debug", "test"
  "maxTokens": 4000                     // Optional: token budget
}
```

**Intent-Aware Context Selection:**

| Intent | Prioritizes |
|--------|-------------|
| `explain` | Type definitions, interfaces, documentation, usage examples |
| `modify` | Callers, dependents, contracts that must be maintained |
| `debug` | Call chain, data flow, error handling, initialization |
| `test` | Related tests, mocks, test utilities, coverage |

**Example — "Help me understand the AuthMiddleware"**

```
Tool: codegraph_get_ai_context
Input: {
  "uri": "file:///project/src/middleware/AuthMiddleware.ts",
  "line": 15,
  "intent": "explain",
  "maxTokens": 4000
}
```

**Output:**
```markdown
# Code Context

## Primary Code
**class: AuthMiddleware**
Language: typescript
Location: file:///project/src/middleware/AuthMiddleware.ts

```typescript
export class AuthMiddleware {
  constructor(
    private tokenService: TokenService,
    private userRepository: UserRepository
  ) {}

  async authenticate(req: Request, res: Response, next: NextFunction) {
    const token = this.extractToken(req);
    if (!token) {
      return res.status(401).json({ error: 'No token provided' });
    }

    try {
      const decoded = await this.tokenService.verify(token);
      req.user = await this.userRepository.findById(decoded.userId);
      next();
    } catch (error) {
      return res.status(401).json({ error: 'Invalid token' });
    }
  }
  // ...
}
```

## Related Code (3)

### 1. type_definition (relevance: 95%)
**TokenService**
```typescript
interface TokenService {
  verify(token: string): Promise<TokenPayload>;
  sign(payload: TokenPayload): string;
}
```

### 2. dependency (relevance: 88%)
**UserRepository**
```typescript
class UserRepository {
  async findById(id: string): Promise<User | null>;
  // ...
}
```

### 3. usage (relevance: 75%)
**app.ts**
```typescript
app.use('/api', authMiddleware.authenticate);
```

## Architecture Context
- Module: middleware
- Neighbors: TokenService, UserRepository, Express
```

**Comparison — Traditional Approach:**
```
Without CodeGraph:
1. read_file AuthMiddleware.ts (full file)
2. grep_search for TokenService definition
3. read_file TokenService.ts
4. grep_search for UserRepository
5. read_file UserRepository.ts
6. grep_search for middleware usage
7. read_file app.ts

= 7+ tool calls, ~8,000-10,000 tokens, manual context assembly

With CodeGraph:
1. codegraph_get_ai_context (intent: explain)

= 1 tool call, ~3,000-4,000 tokens, pre-assembled context
```

---

### 5. `codegraph_find_related_tests`

**Purpose:** Discover tests that cover a piece of code. Essential for understanding test coverage and identifying tests that need updating.

**When to use:**
- Before modifying code (what tests to run?)
- After modifying code (what tests to update?)
- Assessing test coverage
- Finding test examples

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file to find tests for
  "line": 0                             // Optional: specific line (0-indexed)
}
```

**Example — "Find tests for PaymentProcessor"**

```
Tool: codegraph_find_related_tests
Input: {
  "uri": "file:///project/src/payments/PaymentProcessor.ts",
  "line": 0
}
```

**Output:**
```markdown
# Related Tests

Found 3 related test(s):

## 1. PaymentProcessor.test.ts
Relationship: direct_test
Relevance: 98%
```typescript
describe('PaymentProcessor', () => {
  it('should process valid payment', async () => {
    const processor = new PaymentProcessor(mockGateway);
    const result = await processor.processPayment(validOrder, 'card');
    expect(result.success).toBe(true);
  });
  // ...
});
```

## 2. integration/checkout.test.ts
Relationship: integration_test
Relevance: 72%
```typescript
describe('Checkout Flow', () => {
  it('should complete checkout with payment', async () => {
    // Uses PaymentProcessor internally
    const result = await checkout.complete(cart, paymentMethod);
    // ...
  });
});
```

## 3. e2e/payment.e2e.ts
Relationship: e2e_test
Relevance: 65%
```typescript
test('user can complete payment', async ({ page }) => {
  // End-to-end payment test
});
```
```

---

### 6. `codegraph_get_symbol_info`

**Purpose:** Get metadata about a symbol — its type, signature, documentation, and usage statistics.

**When to use:**
- Quick symbol lookup
- Understanding function signatures
- Finding documentation
- Assessing symbol usage

**Parameters:**
```json
{
  "uri": "file:///path/to/file.ts",    // Required: file containing symbol
  "line": 45,                           // Required: line number (0-indexed)
  "character": 0                        // Optional: character position
}
```

**Example — "What is calculateDiscount?"**

```
Tool: codegraph_get_symbol_info
Input: {
  "uri": "file:///project/src/pricing/discounts.ts",
  "line": 34,
  "character": 15
}
```

**Output:**
```markdown
# Symbol Information

Location: file:///project/src/pricing/discounts.ts:35:16

## Documentation & Type Information
```typescript
/**
 * Calculates the discount amount based on order total and customer tier.
 * @param total - The order total before discount
 * @param tier - Customer loyalty tier
 * @returns The discount amount to subtract
 */
function calculateDiscount(total: number, tier: CustomerTier): number
```

## Definition
- /project/src/pricing/discounts.ts:35

## References (12 usages)
- **CheckoutService.ts** (3 references)
  Line 45, Line 89, Line 123
- **PriceCalculator.ts** (4 references)
  Line 12, Line 34, Line 56, Line 78
- **discounts.test.ts** (5 references)
  Line 10, Line 25, Line 40
  ... and 2 more
```

---

## Best Practices for AI Agents

### 1. Start with Context, Then Narrow Down

```
❌ Don't: Immediately grep for specific strings

✅ Do:
1. codegraph_get_ai_context (understand the area)
2. codegraph_get_dependency_graph (if architecture matters)
3. Then use grep for specific patterns if needed
```

### 2. Always Check Impact Before Changes

```
❌ Don't: Make changes and hope tests catch issues

✅ Do:
1. codegraph_analyze_impact (understand blast radius)
2. codegraph_find_related_tests (know what tests to run)
3. Make changes
4. Run affected tests
```

### 3. Use Intent-Aware Context

```
# For explaining code:
codegraph_get_ai_context with intent: "explain"
→ Gets type definitions, interfaces, documentation

# For modifying code:
codegraph_get_ai_context with intent: "modify"
→ Gets callers, contracts, things that might break

# For debugging:
codegraph_get_ai_context with intent: "debug"
→ Gets call chain, error handling, data flow

# For testing:
codegraph_get_ai_context with intent: "test"
→ Gets related tests, mocks, test utilities
```

### 4. Hybrid Approach

CodeGraph tools excel at **understanding relationships**. Traditional tools excel at **specific text searches**.

```
Use CodeGraph for:
- Understanding code structure
- Impact analysis
- Dependency analysis
- Getting comprehensive context

Use grep/read for:
- Finding specific strings or patterns
- Reading specific file sections
- Custom regex searches
```

---

## Efficiency Comparison

Based on real-world benchmarks:

| Scenario | Without CodeGraph | With CodeGraph | Improvement |
|----------|------------------|----------------|-------------|
| Explain a class | 4 tool calls, 9.8K tokens | 1 call, 4.2K tokens | 75% fewer tools, 57% fewer tokens |
| Find dependencies | 10 tool calls, 7K tokens | 1 call, 500 tokens | 90% fewer tools, 93% fewer tokens |
| Impact analysis | 6 tool calls, 5K tokens | 1 call, 1.2K tokens | 83% fewer tools, 76% fewer tokens |
| Debug investigation | 7 tool calls, 8K tokens | 1 call, 3K tokens | 86% fewer tools, 62% fewer tokens |
| Check for circular deps | 12 tool calls, 10K tokens | 1 call, 800 tokens | 92% fewer tools, 92% fewer tokens |

**Overall: 75-80% reduction in tool calls and tokens consumed.**

---

## Error Handling

CodeGraph tools provide helpful error messages when things go wrong:

```markdown
# AI Context Unavailable

❌ No code symbol found at the specified position.

**This could mean:**
- The position is in whitespace, comments, or imports
- The file has not been indexed by CodeGraph yet
- The specified line/character is out of bounds

**Try:**
- Place cursor on a function, class, or variable definition
- Run "CodeGraph: Reindex Workspace" to update the index
- Verify the file is a supported language (TypeScript, JavaScript, Python, Rust, Go)
```

---

## Summary

CodeGraph tools transform how AI agents interact with codebases:

| Traditional | CodeGraph |
|-------------|-----------|
| Text matching | Semantic understanding |
| Many small queries | One comprehensive query |
| Manual relationship tracing | Automatic graph traversal |
| Generic context | Intent-aware context |

**Result: Faster, more accurate, and more efficient code understanding.**
