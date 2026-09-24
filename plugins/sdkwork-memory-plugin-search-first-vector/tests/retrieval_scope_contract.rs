//! Retrieval, isolation, boundedness, and degradation contracts.
//!
//! These tests exercise the two exported ports through the real runtime with
//! injected provider doubles, so every assertion is about observable port
//! behaviour rather than internal state.

mod common;

use std::sync::Arc;

use common::{MappedEmbeddingProvider, ScriptedRerankProvider};
use sdkwork_memory_plugin_search_first_vector::{
    RecordSensitivity, SearchFirstVectorConfig, SearchFirstVectorRuntime, VectorProjectionCommand,
    VectorProjectionOutcome, DEGRADATION_EMBEDDING_CALL_FAILED, DEGRADATION_EMBEDDING_PORT_UNBOUND,
    DEGRADATION_RERANK_CALL_FAILED, DEGRADATION_RETRIEVER_KIND_UNSERVED, INDEX_KIND,
    MAX_SEARCH_LIMIT, RETRIEVER_CODE,
};
use sdkwork_memory_spi::{
    parse_metadata_filter, MemoryIndexPort, MemoryRetrieverKind, MemoryRetrieverPort,
    MemoryScopeContext, MemorySensitivityReadScope, MemorySpiError, SearchMemoryCandidatesQuery,
};
use serde_json::json;

fn scope(tenant_id: i64, space_id: i64) -> MemoryScopeContext {
    MemoryScopeContext {
        tenant_id,
        space_id,
        organization_id: None,
        user_id: None,
    }
}

fn projectable(
    memory_id: &str,
    text: &str,
    sensitivity: RecordSensitivity,
) -> VectorProjectionCommand {
    VectorProjectionCommand {
        memory_id: memory_id.to_string(),
        memory_type: "fact".to_string(),
        subject: Some("user".to_string()),
        predicate: Some("prefers".to_string()),
        object_text: text.to_string(),
        canonical_text: text.to_string(),
        expiration_date: None,
        sensitivity,
    }
}

fn query(
    scope: MemoryScopeContext,
    text: &str,
    limit: u32,
    read_scope: MemorySensitivityReadScope,
) -> SearchMemoryCandidatesQuery {
    SearchMemoryCandidatesQuery {
        scope,
        query: text.to_string(),
        limit,
        retriever_kinds: vec![MemoryRetrieverKind::Vector],
        memory_types: Vec::new(),
        read_scope,
        metadata_filter: None,
        include_expired: false,
    }
}

fn config() -> SearchFirstVectorConfig {
    SearchFirstVectorConfig {
        use_rerank_when_bound: false,
        ..SearchFirstVectorConfig::default()
    }
}

fn runtime_with_embedding(embedding: Arc<MappedEmbeddingProvider>) -> SearchFirstVectorRuntime {
    SearchFirstVectorRuntime::new(config(), Some(embedding), None, None).expect("runtime")
}

/// Two-dimensional embedding space where `alpha` and the query coincide.
fn embedding_provider() -> Arc<MappedEmbeddingProvider> {
    MappedEmbeddingProvider::new(2, vec![1.0, 0.0])
        .with_entry("alpha", vec![1.0, 0.0])
        .with_entry("beta", vec![0.0, 1.0])
        .with_entry("gamma", vec![0.9, 0.1])
        .with_entry("restricted", vec![1.0, 0.0])
}

#[test]
fn runtime_reports_its_port_identity() {
    let runtime = runtime_with_embedding(embedding_provider());

    assert_eq!(runtime.retriever_code(), RETRIEVER_CODE);
    assert!(runtime.supports_bounded_scoped_search());
    assert_eq!(runtime.index_kind(), INDEX_KIND);
    assert_eq!(runtime.dimensions(), Some(2));
}

#[tokio::test]
async fn search_scoped_refuses_a_blank_query() {
    let runtime = runtime_with_embedding(embedding_provider());

    let error = runtime
        .search_scoped(query(
            scope(1, 1),
            "   ",
            5,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect_err("a blank query must be refused");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryRetrieverPort");
            assert!(message.contains("must not be blank"), "got: {message}");
        }
        other => panic!("expected a retriever port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn search_scoped_refuses_a_limit_outside_the_bounded_ceiling() {
    let runtime = runtime_with_embedding(embedding_provider());

    for refused in [0, MAX_SEARCH_LIMIT + 1] {
        let error = runtime
            .search_scoped(query(
                scope(1, 1),
                "alpha",
                refused,
                MemorySensitivityReadScope::Owner,
            ))
            .await
            .expect_err("an out-of-bounds limit must be refused");

        match error {
            MemorySpiError::PortOperationFailed { message, .. } => {
                assert!(message.contains("out of bounds"), "got: {message}");
            }
            other => panic!("expected a retriever port failure, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn search_scoped_accepts_the_ceiling_itself() {
    let runtime = runtime_with_embedding(embedding_provider());

    runtime
        .search_scoped(query(
            scope(1, 1),
            "alpha",
            MAX_SEARCH_LIMIT,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("the ceiling itself is a valid request");
}

#[tokio::test]
async fn search_scoped_degrades_when_no_embedding_provider_is_bound() {
    let runtime = SearchFirstVectorRuntime::without_providers(config()).expect("runtime");

    let result = runtime
        .search_scoped(query(
            scope(1, 1),
            "alpha",
            5,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("an unbound provider is a degradation, not a hard failure");

    assert!(result.degraded);
    assert_eq!(
        result.degradation_codes,
        vec![DEGRADATION_EMBEDDING_PORT_UNBOUND.to_string()]
    );
    assert_eq!(
        result.unavailable_retriever_kinds,
        vec![MemoryRetrieverKind::Vector]
    );
    assert!(result.records.is_empty());
}

#[tokio::test]
async fn search_scoped_degrades_when_the_requested_retriever_kind_is_not_served() {
    let runtime = runtime_with_embedding(embedding_provider());
    let mut request = query(scope(1, 1), "alpha", 5, MemorySensitivityReadScope::Owner);
    request.retriever_kinds = vec![MemoryRetrieverKind::Sql, MemoryRetrieverKind::Keyword];

    let result = runtime.search_scoped(request).await.expect("degradation");

    assert!(result.degraded);
    assert_eq!(
        result.degradation_codes,
        vec![DEGRADATION_RETRIEVER_KIND_UNSERVED.to_string()]
    );
    assert!(result.records.is_empty());
}

#[tokio::test]
async fn search_scoped_serves_an_empty_retriever_kind_request() {
    // An empty list means the caller applied no kind restriction, which cannot
    // widen the result beyond this plugin's single declared kind.
    let runtime = runtime_with_embedding(embedding_provider());
    runtime
        .project_record(
            &scope(1, 1),
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    let mut request = query(scope(1, 1), "alpha", 5, MemorySensitivityReadScope::Owner);
    request.retriever_kinds = Vec::new();

    let result = runtime.search_scoped(request).await.expect("search");

    assert!(!result.degraded);
    assert_eq!(result.records.len(), 1);
}

#[tokio::test]
async fn search_scoped_degrades_when_the_embedding_provider_refuses() {
    let embedding = MappedEmbeddingProvider::new(2, vec![1.0, 0.0]).failing("upstream unavailable");
    let runtime = runtime_with_embedding(embedding);

    let result = runtime
        .search_scoped(query(
            scope(1, 1),
            "alpha",
            5,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("a provider refusal is reported as degradation");

    assert!(result.degraded);
    assert_eq!(
        result.degradation_codes,
        vec![DEGRADATION_EMBEDDING_CALL_FAILED.to_string()]
    );
    assert_eq!(
        result.unavailable_retriever_kinds,
        vec![MemoryRetrieverKind::Vector]
    );
}

#[tokio::test]
async fn a_projection_never_leaks_across_a_tenant_or_space_boundary() {
    let runtime = runtime_with_embedding(embedding_provider());
    runtime
        .project_record(
            &scope(1, 10),
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    for elsewhere in [scope(1, 11), scope(2, 10), scope(2, 11)] {
        let result = runtime
            .search_scoped(query(
                elsewhere,
                "alpha",
                10,
                MemorySensitivityReadScope::Owner,
            ))
            .await
            .expect("search");

        assert!(
            result.records.is_empty(),
            "a projection must never be visible outside its own tenant and space"
        );
    }

    // The owning scope still sees it, so the isolation above is isolation and
    // not a broken index.
    let own = runtime
        .search_scoped(query(
            scope(1, 10),
            "alpha",
            10,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("search");

    assert_eq!(own.records.len(), 1);
}

#[tokio::test]
async fn the_limit_is_honoured_at_the_index_boundary() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    for (memory_id, text) in [
        ("m-alpha", "alpha"),
        ("m-beta", "beta"),
        ("m-gamma", "gamma"),
    ] {
        runtime
            .project_record(
                &owner,
                projectable(memory_id, text, RecordSensitivity::Public),
            )
            .await
            .expect("projection");
    }

    let limited = runtime
        .search_scoped(query(
            owner.clone(),
            "alpha",
            2,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("search");

    assert_eq!(limited.records.len(), 2);
    // Descending similarity: the exact match first, then the near match.
    assert_eq!(limited.records[0].memory_id, "m-alpha");
    assert_eq!(limited.records[1].memory_id, "m-gamma");

    let all_three = runtime
        .search_scoped(query(owner, "alpha", 3, MemorySensitivityReadScope::Owner))
        .await
        .expect("search");

    assert_eq!(all_three.records.len(), 3);
}

#[tokio::test]
async fn the_read_scope_is_never_widened() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-public", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");
    runtime
        .project_record(
            &owner,
            projectable("m-restricted", "restricted", RecordSensitivity::Owner),
        )
        .await
        .expect("projection");

    let public_only = runtime
        .search_scoped(query(
            owner.clone(),
            "alpha",
            10,
            MemorySensitivityReadScope::Public,
        ))
        .await
        .expect("search");

    assert_eq!(public_only.records.len(), 1);
    assert_eq!(public_only.records[0].memory_id, "m-public");

    let as_owner = runtime
        .search_scoped(query(owner, "alpha", 10, MemorySensitivityReadScope::Owner))
        .await
        .expect("search");

    assert_eq!(as_owner.records.len(), 2);
}

#[tokio::test]
async fn sensitivity_filtering_happens_before_truncation() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    // Five restricted records all match the query better than the single
    // permitted one, so a top-K-then-filter implementation would return nothing.
    for offset in 0..5 {
        runtime
            .project_record(
                &owner,
                projectable(
                    &format!("m-owner-{offset}"),
                    "restricted",
                    RecordSensitivity::Owner,
                ),
            )
            .await
            .expect("projection");
    }
    runtime
        .project_record(
            &owner,
            projectable("m-public", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    let result = runtime
        .search_scoped(query(
            owner,
            "restricted",
            1,
            MemorySensitivityReadScope::Public,
        ))
        .await
        .expect("search");

    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].memory_id, "m-public");
}

#[tokio::test]
async fn the_legacy_unscoped_retrieve_is_refused() {
    let runtime = runtime_with_embedding(embedding_provider());

    let error = runtime
        .retrieve(sdkwork_memory_spi::RetrieveMemoryCandidatesCommand {
        read_scope: MemorySensitivityReadScope::Public,
        limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
            query: "alpha".to_string(),
        })
        .await
        .expect_err("unscoped retrieval must fail closed");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryRetrieverPort");
            assert!(message.contains("scope-aware"), "got: {message}");
        }
        other => panic!("expected a retriever port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn the_legacy_scoped_retrieve_returns_only_public_records() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-public", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");
    runtime
        .project_record(
            &owner,
            projectable("m-owner", "restricted", RecordSensitivity::Owner),
        )
        .await
        .expect("projection");

    // This helper carries no read scope of its own, so it reads at the least
    // privileged tier instead of assuming one.
    let result = runtime
        .retrieve_scoped(
            owner,
            sdkwork_memory_spi::RetrieveMemoryCandidatesCommand {
        read_scope: MemorySensitivityReadScope::Public,
        limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
                query: "alpha".to_string(),
            },
        )
        .await
        .expect("bounded legacy retrieval");

    assert_eq!(result.memory_ids, vec!["m-public".to_string()]);
}

#[tokio::test]
async fn index_port_refuses_an_unprojected_memory_id() {
    let runtime = runtime_with_embedding(embedding_provider());

    let error = runtime
        .index("m-absent".to_string())
        .await
        .expect_err("an unprojected id must be refused");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryIndexPort");
            assert!(message.contains("no projection"), "got: {message}");
        }
        other => panic!("expected an index port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn index_port_accepts_a_projected_memory_id() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    let receipt = runtime
        .index("m-alpha".to_string())
        .await
        .expect("a projected id is accepted");

    assert_eq!(receipt.memory_id, "m-alpha");
}

#[tokio::test]
async fn index_port_refuses_an_id_projected_into_two_scopes() {
    let runtime = runtime_with_embedding(embedding_provider());

    for owner in [scope(1, 1), scope(1, 2)] {
        runtime
            .project_record(
                &owner,
                projectable("m-alpha", "alpha", RecordSensitivity::Public),
            )
            .await
            .expect("projection");
    }

    let error = runtime
        .index("m-alpha".to_string())
        .await
        .expect_err("an ambiguous projection must be refused");

    match error {
        MemorySpiError::PortOperationFailed { message, .. } => {
            assert!(message.contains("2 scopes"), "got: {message}");
        }
        other => panic!("expected an index port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn index_port_refuses_a_blank_memory_id() {
    let runtime = runtime_with_embedding(embedding_provider());

    let error = runtime
        .index("  ".to_string())
        .await
        .expect_err("a blank id must be refused");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryIndexPort");
            assert!(message.contains("usable projection key"), "got: {message}");
        }
        other => panic!("expected an index port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn index_port_refuses_when_no_embedding_provider_is_bound() {
    let runtime = SearchFirstVectorRuntime::without_providers(config()).expect("runtime");

    let error = runtime
        .index("m-alpha".to_string())
        .await
        .expect_err("no index exists without an embedding provider");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryIndexPort");
            assert!(message.contains("EmbeddingModelPort"), "got: {message}");
        }
        other => panic!("expected an index port failure, got {other:?}"),
    }
}

#[tokio::test]
async fn deletion_propagates_out_of_the_derived_index() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    assert!(runtime.remove_record(&owner, "m-alpha").expect("removal"));
    assert!(
        !runtime
            .remove_record(&owner, "m-alpha")
            .expect("second removal"),
        "a second removal must report that nothing was removed"
    );
    assert_eq!(runtime.indexed_len().expect("len"), 0);

    // The record must no longer be answerable from the derived index.
    let result = runtime
        .search_scoped(query(owner, "alpha", 10, MemorySensitivityReadScope::Owner))
        .await
        .expect("search");
    assert!(result.records.is_empty());
    assert!(runtime.index("m-alpha".to_string()).await.is_err());
}

#[tokio::test]
async fn a_projection_is_refused_when_the_embedding_width_disagrees() {
    // The provider advertises three dimensions but returns two, which must be
    // refused rather than padded.
    let embedding =
        MappedEmbeddingProvider::new(3, vec![1.0, 0.0, 0.0]).with_forced_vector(vec![1.0, 0.0]);
    let runtime = SearchFirstVectorRuntime::new(config(), Some(embedding), None, None)
        .expect("runtime construction");

    let error = runtime
        .project_record(
            &scope(1, 1),
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect_err("a mis-sized embedding must be refused");

    assert!(matches!(
        error,
        sdkwork_memory_plugin_search_first_vector::SearchFirstVectorError::EmbeddingDimensionMismatch { .. }
    ));
    assert_eq!(runtime.indexed_len().expect("len"), 0);
}

#[tokio::test]
async fn a_refresh_keeps_one_projection_and_reports_a_refresh() {
    let runtime = runtime_with_embedding(embedding_provider());
    let owner = scope(1, 1);

    let inserted = runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");
    let refreshed = runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("refresh");

    assert_eq!(inserted, VectorProjectionOutcome::Inserted);
    assert_eq!(refreshed, VectorProjectionOutcome::Refreshed);
    assert_eq!(runtime.indexed_len().expect("len"), 1);
}

#[tokio::test]
async fn a_buckets_entry_ceiling_evicts_the_oldest_projection() {
    let runtime = SearchFirstVectorRuntime::new(
        SearchFirstVectorConfig {
            max_entries_per_scope: 2,
            use_rerank_when_bound: false,
            ..SearchFirstVectorConfig::default()
        },
        Some(embedding_provider()),
        None,
        None,
    )
    .expect("runtime construction");
    let owner = scope(1, 1);

    for (memory_id, text) in [
        ("m-alpha", "alpha"),
        ("m-beta", "beta"),
        ("m-gamma", "gamma"),
    ] {
        runtime
            .project_record(
                &owner,
                projectable(memory_id, text, RecordSensitivity::Public),
            )
            .await
            .expect("projection");
    }

    assert_eq!(runtime.indexed_len().expect("len"), 2);
    // The first projection is the deterministically oldest, so it is the one
    // displaced.
    assert!(runtime
        .projection(&owner, "m-alpha")
        .expect("lookup")
        .is_none());
    assert!(runtime
        .projection(&owner, "m-gamma")
        .expect("lookup")
        .is_some());
}

#[tokio::test]
async fn rerank_reorders_candidates_without_dropping_any() {
    let rerank =
        ScriptedRerankProvider::new(vec![vec!["m-beta".to_string(), "m-alpha".to_string()]]);
    let runtime = SearchFirstVectorRuntime::new(
        SearchFirstVectorConfig::default(),
        Some(embedding_provider()),
        None,
        Some(rerank),
    )
    .expect("runtime construction");
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");
    runtime
        .project_record(
            &owner,
            projectable("m-beta", "gamma", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    let result = runtime
        .search_scoped(query(owner, "alpha", 10, MemorySensitivityReadScope::Owner))
        .await
        .expect("search");

    assert!(!result.degraded);
    assert_eq!(result.records.len(), 2);
    assert_eq!(result.records[0].memory_id, "m-beta");
    assert_eq!(result.records[1].memory_id, "m-alpha");
}

#[tokio::test]
async fn a_rerank_refusal_degrades_without_losing_candidates() {
    let rerank = ScriptedRerankProvider::new(Vec::new()).failing("rerank offline");
    let runtime = SearchFirstVectorRuntime::new(
        SearchFirstVectorConfig::default(),
        Some(embedding_provider()),
        None,
        Some(rerank),
    )
    .expect("runtime construction");
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");
    runtime
        .project_record(
            &owner,
            projectable("m-beta", "gamma", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    let result = runtime
        .search_scoped(query(owner, "alpha", 10, MemorySensitivityReadScope::Owner))
        .await
        .expect("search");

    assert!(result.degraded);
    assert_eq!(
        result.degradation_codes,
        vec![DEGRADATION_RERANK_CALL_FAILED.to_string()]
    );
    // Only reranking degraded, so no retriever kind is unavailable.
    assert!(result.unavailable_retriever_kinds.is_empty());
    assert_eq!(result.records.len(), 2);
}

#[tokio::test]
async fn search_scoped_refuses_a_metadata_filter_instead_of_ignoring_it() {
    let embedding = embedding_provider();
    let runtime = runtime_with_embedding(Arc::clone(&embedding));
    let owner = scope(1, 1);

    runtime
        .project_record(
            &owner,
            projectable("m-alpha", "alpha", RecordSensitivity::Public),
        )
        .await
        .expect("projection");

    // Control: the same request with no filter is served, so the refusal below is
    // caused by the filter and by nothing else.
    let served = runtime
        .search_scoped(query(
            owner.clone(),
            "alpha",
            5,
            MemorySensitivityReadScope::Owner,
        ))
        .await
        .expect("an unfiltered request is served");
    assert_eq!(served.records.len(), 1);
    // One call for the projection above and one for this search itself.
    assert_eq!(embedding.recorded_inputs().len(), 2);

    let calls_before_refusal = embedding.recorded_inputs();

    let mut request = query(owner, "alpha", 5, MemorySensitivityReadScope::Owner);
    request.metadata_filter = Some(
        parse_metadata_filter(&json!({"owner": "alice"}))
            .expect("a well-formed filter")
            .expect("a present filter"),
    );

    let error = runtime
        .search_scoped(request)
        .await
        .expect_err("a metadata filter must be refused, never silently ignored");

    match error {
        MemorySpiError::PortOperationFailed { port, message } => {
            assert_eq!(port, "MemoryRetrieverPort");
            assert!(message.contains("metadata filter"), "got: {message}");
        }
        other => panic!("expected a retriever port failure, got {other:?}"),
    }

    // Refusal precedes provider work: the rejected request must not have produced
    // a further embedding call, so no cost is paid for a request that can never
    // be satisfied.
    assert_eq!(
        embedding.recorded_inputs(),
        calls_before_refusal,
        "the filter must be refused before any further embedding call"
    );
}
