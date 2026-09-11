export interface MemoryCapabilities {
    embeddingOptional: boolean;
    retrievers: ('sql' | 'keyword' | 'dictionary' | 'time' | 'event' | 'vector' | 'graph' | 'grep_file' | 'custom')[];
    providerInterfaces: ('llm' | 'embedding' | 'rerank' | 'tokenizer' | 'graph' | 'search' | 'file' | 'memory')[];
    implementationKinds: ('native_sql' | 'event_sourced' | 'graph_temporal' | 'search_first' | 'local_embedded' | 'external_provider_bridge' | 'hybrid_platform')[];
    openApiPrefix: string;
    sdkFamily: string;
    checkedAt: string;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-capabilities.d.ts.map