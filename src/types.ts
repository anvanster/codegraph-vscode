import { Position, Range, Location } from 'vscode-languageclient';

// ==========================================
// Dependency Graph Types
// ==========================================

export interface DependencyGraphParams {
    uri: string;
    depth?: number;
    includeExternal?: boolean;
    direction?: 'imports' | 'importedBy' | 'both';
}

export interface DependencyNode {
    id: string;
    label: string;
    type: 'module' | 'package' | 'file';
    language: string;
    uri: string;
    metadata?: Record<string, unknown>;
}

export interface DependencyEdge {
    from: string;
    to: string;
    type: 'import' | 'require' | 'use';
    metadata?: Record<string, unknown>;
}

export interface DependencyGraphResponse {
    nodes: DependencyNode[];
    edges: DependencyEdge[];
}

// ==========================================
// Call Graph Types
// ==========================================

export interface CallGraphParams {
    uri: string;
    position: Position;
    direction?: 'callers' | 'callees' | 'both';
    depth?: number;
    includeExternal?: boolean;
}

export interface FunctionNode {
    id: string;
    name: string;
    signature: string;
    uri: string;
    range: Range;
    language: string;
    metrics?: {
        complexity?: number;
        linesOfCode?: number;
        callCount?: number;
    };
}

export interface CallEdge {
    from: string;
    to: string;
    callSites: Location[];
    isRecursive?: boolean;
}

export interface CallGraphResponse {
    root: FunctionNode;
    nodes: FunctionNode[];
    edges: CallEdge[];
}

// ==========================================
// AI Context Types
// ==========================================

export interface AIContextParams {
    uri: string;
    position: Position;
    contextType: 'explain' | 'modify' | 'debug' | 'test';
    maxTokens?: number;
}

export interface PrimaryContext {
    type: 'function' | 'class' | 'module';
    name: string;
    code: string;
    language: string;
    location: Location;
}

export interface RelatedSymbol {
    name: string;
    relationship: 'calls' | 'called_by' | 'uses' | 'used_by' | 'inherits' | 'implements' | 'tests' | 'similar';
    code: string;
    location: Location;
    relevanceScore: number;
}

export interface DependencyInfo {
    name: string;
    type: 'import' | 'type_dependency';
    code?: string;
}

export interface UsageExample {
    code: string;
    location: Location;
    description?: string;
}

export interface ArchitectureInfo {
    module: string;
    layer?: string;
    neighbors: string[];
}

export interface AIContextResponse {
    primaryContext: PrimaryContext;
    relatedSymbols: RelatedSymbol[];
    dependencies: DependencyInfo[];
    usageExamples?: UsageExample[];
    architecture?: ArchitectureInfo;
    metadata: {
        totalTokens: number;
        queryTime: number;
    };
}

// ==========================================
// Impact Analysis Types
// ==========================================

export interface ImpactAnalysisParams {
    uri: string;
    position: Position;
    analysisType: 'modify' | 'delete' | 'rename';
}

export interface DirectImpact {
    uri: string;
    range: Range;
    type: 'caller' | 'reference' | 'subclass' | 'implementation';
    severity: 'breaking' | 'warning' | 'info';
}

export interface IndirectImpact {
    uri: string;
    path: string[];
    severity: 'breaking' | 'warning' | 'info';
}

export interface AffectedTest {
    uri: string;
    testName: string;
}

export interface ImpactAnalysisResponse {
    directImpact: DirectImpact[];
    indirectImpact: IndirectImpact[];
    affectedTests: AffectedTest[];
    summary: {
        filesAffected: number;
        breakingChanges: number;
        warnings: number;
    };
}

// ==========================================
// Related Tests Types
// ==========================================

export interface RelatedTestsParams {
    uri: string;
    position: Position;
    limit?: number;
}

export interface RelatedTest {
    uri: string;
    testName: string;
    relationship: string;
    range: Range;
}

export interface RelatedTestsResponse {
    tests: RelatedTest[];
    truncated?: boolean;
}

// ==========================================
// Parser Metrics Types
// ==========================================

export interface ParserMetricsParams {
    language?: string;
}

export interface ParserMetric {
    language: string;
    filesAttempted: number;
    filesSucceeded: number;
    filesFailed: number;
    totalEntities: number;
    totalRelationships: number;
    totalParseTimeMs: number;
    avgParseTimeMs: number;
}

export interface ParserMetricsResponse {
    metrics: ParserMetric[];
    totals: {
        filesAttempted: number;
        filesSucceeded: number;
        filesFailed: number;
        totalEntities: number;
        successRate: number;
    };
}

// ==========================================
// Graph Visualization Types (for webview)
// ==========================================

export interface GraphNode {
    id: string;
    label: string;
    type: string;
    language?: string;
    x?: number;
    y?: number;
}

export interface GraphEdge {
    from: string;
    to: string;
    type: string;
}

export interface GraphData {
    nodes: GraphNode[];
    edges: GraphEdge[];
}
