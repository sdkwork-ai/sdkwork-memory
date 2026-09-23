//! Differential contract tests for metadata filter pushdown.
//!
//! The SPI crate owns an executable definition of what a metadata filter means
//! (`MetadataFilterExpression::matches`). The plugin owns a SQL translation of the same
//! filter. These tests hold the two against each other over one real SQLite database: for
//! every filter below, the set of memory ids the SQL query returns must equal the set the
//! oracle admits over the same metadata.
//!
//! This is the only way to catch the failure mode the batch exists to prevent. A pushed-down
//! predicate that is subtly too wide or too narrow still returns rows, so nothing looks
//! broken; the oracle is what makes the divergence visible.

use serde_json::{json, Value};

use sdkwork_memory_plugin_native_sql::{NativeSqlCreateSpaceCommand, NativeSqlMemoryStore};
use sdkwork_memory_spi::{
    parse_metadata_filter, CreateCanonicalMemoryCommand, MemoryRecordStorePort,
    MemoryRetrieverKind, MemoryRetrieverPort, MemoryScopeContext, MemorySensitivityReadScope,
    MetadataFilterExpression, SearchMemoryCandidatesQuery,
};

const TENANT_ID: i64 = 1;
const SPACE_ID: i64 = 1;

/// A metadata document is the only thing that varies between fixtures.
///
/// `None` means the record has no metadata value at all, which is distinct from an empty
/// object and must not be treated as one.
const FIXTURES: [(&str, Option<&str>); 9] = [
    (
        "m-tenant-t1",
        Some(
            r#"{"tenant":"t1","score":0.9,"tag":["a","b"],"owner":"alice","flag":true,"text":"A Foo Bar"}"#,
        ),
    ),
    (
        "m-tenant-t2",
        Some(
            r#"{"tenant":"t2","score":0.4,"tag":["c"],"owner":"bob","flag":false,"text":"a foo bar"}"#,
        ),
    ),
    ("m-null-value", Some(r#"{"tenant":null,"score":1}"#)),
    ("m-empty-object", Some("{}")),
    ("m-no-metadata", None),
    ("m-case", Some(r#"{"text":"A Foo Bar","tenant":"t1"}"#)),
    ("m-numeric-string", Some(r#"{"score":"9","tenant":"t3"}"#)),
    ("m-non-numeric", Some(r#"{"score":"high","tenant":"t4"}"#)),
    ("m-unicode", Some(r#"{"text":"le café","tenant":"t5"}"#)),
];

/// Opens an in-memory SQLite store holding one space and the fixture records.
async fn fixture_store() -> NativeSqlMemoryStore {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("sqlite store must initialize");
    store
        .create_space_record(
            TENANT_ID,
            SPACE_ID,
            &NativeSqlCreateSpaceCommand {
                organization_id: Some(7),
                owner_subject_type: "user".to_string(),
                owner_subject_id: "owner-1".to_string(),
                space_type: "team".to_string(),
                display_name: "Filter Space".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .expect("space must be created");
    let scope = MemoryScopeContext::for_test(TENANT_ID, SPACE_ID);
    for (memory_id, _) in FIXTURES {
        MemoryRecordStorePort::create_canonical_atomic(
            &store,
            CreateCanonicalMemoryCommand {
                scope: scope.clone(),
                memory_id: memory_id.to_string(),
                scope_label: "user".to_string(),
                memory_type: "semantic".to_string(),
                subject: Some("retrieval".to_string()),
                predicate: Some("matches".to_string()),
                object_text: format!("needle {memory_id}"),
                canonical_text: format!("needle {memory_id}"),
                sensitivity_level: "internal".to_string(),
                journal: journal(memory_id),
                expires_at: None,
            },
        )
        .await
        .expect("fixture record must be created");
    }

    // No write path carries caller metadata yet, so seed the column the filter reads
    // directly. The value under test is the predicate, not the write path.
    for (memory_id, metadata) in FIXTURES {
        sqlx::query("UPDATE ai_record SET metadata_json = ? WHERE uuid = ?")
            .bind(metadata)
            .bind(memory_id)
            .execute(store.pool())
            .await
            .expect("metadata seed must apply");
    }
    store
}

fn journal(memory_id: &str) -> sdkwork_memory_spi::MemoryMutationJournal {
    sdkwork_memory_spi::MemoryMutationJournal {
        outbox_id: format!("outbox-{memory_id}"),
        aggregate_type: "memory_record".to_string(),
        aggregate_id: memory_id.to_string(),
        event_type: "memory.record.created".to_string(),
        event_version: "1.0".to_string(),
        payload_json: format!(r#"{{"memoryId":"{memory_id}"}}"#),
        audit_id: format!("audit-{memory_id}"),
        audit_action: "memory.record.create".to_string(),
        audit_resource_type: "memory_record".to_string(),
        audit_resource_id: memory_id.to_string(),
        audit_result: "accepted".to_string(),
    }
}

/// Runs the filter through the SQL pushdown and returns the memory ids it admits.
async fn pushdown_ids(
    store: &NativeSqlMemoryStore,
    filter: Option<&MetadataFilterExpression>,
) -> Vec<String> {
    let result = MemoryRetrieverPort::search_scoped(
        store,
        SearchMemoryCandidatesQuery {
            scope: MemoryScopeContext::for_test(TENANT_ID, SPACE_ID),
            // Every fixture contains this token, so the keyword predicate never masks a
            // metadata mismatch by excluding a row for an unrelated reason.
            query: "needle".to_string(),
            limit: 50,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: filter.cloned(),
            include_expired: false,
        },
    )
    .await
    .expect("a supported filter must not fail the query");

    let mut ids = result
        .records
        .into_iter()
        .map(|candidate| candidate.memory_id)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

/// Computes the same answer with the SPI oracle.
fn oracle_ids(filter: &MetadataFilterExpression) -> Vec<String> {
    let mut ids = FIXTURES
        .iter()
        .filter(|(_, metadata)| {
            filter
                .matches(*metadata)
                .expect("fixture metadata must be valid JSON")
        })
        .map(|(memory_id, _)| (*memory_id).to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

/// Every filter the SQL dialects must agree with the oracle on.
fn differential_filters() -> Vec<Value> {
    vec![
        json!({"tenant": "t1"}),
        json!({"tenant": {"eq": "t1"}}),
        json!({"tenant": {"ne": "t1"}}),
        json!({"tenant": {"ne": "nobody"}}),
        json!({"score": {"gte": 0.5}}),
        json!({"score": {"gt": 2}}),
        json!({"score": {"lt": 0.5}}),
        json!({"score": {"lte": 0.4}}),
        json!({"score": {"gt": 1}}),
        json!({"tag": {"in": ["a"]}}),
        json!({"tag": {"nin": ["a"]}}),
        json!({"tag": ["a", "c"]}),
        json!({"owner": "*"}),
        json!({"absent": "*"}),
        json!({"text": {"contains": "Foo"}}),
        json!({"text": {"icontains": "foo"}}),
        json!({"text": {"contains": "foo"}}),
        json!({"flag": true}),
        json!({"flag": false}),
        json!({"score": {"gte": 0}}),
        json!({"AND": [{"tenant": "t1"}, {"flag": true}]}),
        json!({"AND": [{"tenant": "t1"}, {"owner": "bob"}]}),
        json!({"OR": [{"tenant": "t1"}, {"tenant": "t2"}]}),
        json!({"OR": [{"tenant": "t1"}, {"absent": "x"}]}),
        json!({"NOT": [{"tenant": "t1"}]}),
        json!({"NOT": [{"tenant": "t1"}, {"owner": "bob"}]}),
        json!({"NOT": [{"owner": "*"}]}),
        json!({"missing": "x"}),
        json!({"tenant": "t1", "score": {"gte": 0.9}}),
        json!({"OR": [{"AND": [{"tenant": "t1"}, {"flag": true}]}, {"tenant": "t2"}]}),
    ]
}

/// Asserts SQL pushdown and the oracle agree for every filter.
async fn assert_pushdown_matches_oracle(store: &NativeSqlMemoryStore, label: &str) {
    let unfiltered = pushdown_ids(store, None).await;
    assert_eq!(
        unfiltered.len(),
        FIXTURES.len(),
        "{label}: every fixture must be retrievable before filtering, otherwise a mismatch \
         below could be caused by the base query instead of the filter"
    );

    for filter in differential_filters() {
        let parsed = parse_metadata_filter(&filter)
            .expect("fixture filter must parse")
            .expect("fixture filter must carry a condition");
        let expected = oracle_ids(&parsed);
        let actual = pushdown_ids(store, Some(&parsed)).await;
        assert_eq!(
            actual, expected,
            "{label}: SQL pushdown disagreed with the oracle for {filter}"
        );
    }
}

#[tokio::test]
async fn sqlite_metadata_filter_pushdown_matches_the_spi_oracle() {
    let store = fixture_store().await;
    assert_pushdown_matches_oracle(&store, "fulltext path").await;
}

#[tokio::test]
async fn sqlite_metadata_filter_pushdown_matches_the_oracle_on_the_like_fallback() {
    let store = fixture_store().await;
    // The retrieval path answers from the full-text index first and falls back to LIKE only
    // when it finds nothing, so emptying the index is what exercises the fallback. A filter
    // wired into only one of the two would silently stop applying on the other.
    sqlx::query("DELETE FROM ai_record_fts")
        .execute(store.pool())
        .await
        .expect("clearing the full-text index must succeed");
    assert_pushdown_matches_oracle(&store, "like fallback").await;
}

#[tokio::test]
async fn an_applied_filter_actually_narrows_the_result_set() {
    // The regression this guards: `filters` used to reach the service and never reach the
    // query, so a filtered request returned every record with no signal. A filter that is
    // ignored cannot satisfy this assertion.
    let store = fixture_store().await;
    let unfiltered = pushdown_ids(&store, None).await;
    let parsed = parse_metadata_filter(&json!({"tenant": "t1"}))
        .expect("filter parses")
        .expect("filter carries a condition");
    let filtered = pushdown_ids(&store, Some(&parsed)).await;

    assert!(!filtered.is_empty(), "the filter must match something");
    assert!(
        filtered.len() < unfiltered.len(),
        "a narrowing filter must return fewer rows than no filter, got {} of {}",
        filtered.len(),
        unfiltered.len()
    );
}

#[tokio::test]
async fn sqlite_refuses_a_filter_it_cannot_express_exactly() {
    // SQLite's lower() folds ASCII only, so a non-ASCII icontains needle would match fewer
    // rows here than PostgreSQL. The store must say so instead of returning a narrower set
    // that looks like a legitimate result.
    let store = fixture_store().await;
    let parsed = parse_metadata_filter(&json!({"text": {"icontains": "CAFÉ"}}))
        .expect("filter parses")
        .expect("filter carries a condition");

    let error = MemoryRetrieverPort::search_scoped(
        &store,
        SearchMemoryCandidatesQuery {
            scope: MemoryScopeContext::for_test(TENANT_ID, SPACE_ID),
            query: "needle".to_string(),
            limit: 50,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: Some(parsed),
            include_expired: false,
        },
    )
    .await
    .expect_err("a non-ASCII icontains filter must be refused, not narrowed");

    let message = format!("{error}");
    assert!(
        message.contains("metadata filter"),
        "the refusal must name the metadata filter, got: {message}"
    );
}
