use sdkwork_memory_plugin_native_sql::{NativeSqlCreateSpaceCommand, NativeSqlMemoryStore};
use sdkwork_memory_spi::{
    CreateMemoryCandidateCommand, ListMemoryCandidatesQuery, MemoryCandidateStorePort,
    MemoryScopeContext,
};

fn candidate_command(
    scope: MemoryScopeContext,
    candidate_id: &str,
) -> CreateMemoryCandidateCommand {
    CreateMemoryCandidateCommand {
        scope,
        candidate_id: candidate_id.to_string(),
        candidate_type: "observation".to_string(),
        memory_type: "semantic".to_string(),
        proposed_text: format!("proposal for {candidate_id}"),
        proposed_payload_json: Some(format!(r#"{{"candidate":"{candidate_id}"}}"#)),
        evidence_json: Some(r#"{"source":"test"}"#.to_string()),
        confidence: 0.9,
        learning_job_uuid: None,
    }
}

async fn seed_space(store: &NativeSqlMemoryStore, tenant_id: i64, space_id: i64) {
    store
        .create_space_record(
            tenant_id,
            space_id,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: format!("owner-{tenant_id}-{space_id}"),
                space_type: "workspace".to_string(),
                display_name: format!("Test space {tenant_id}/{space_id}"),
                default_scope: "user".to_string(),
            },
        )
        .await
        .expect("test space should be inserted");
}

#[tokio::test]
async fn native_sql_candidate_listing_is_tenant_scoped_and_cursor_bounded() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("native SQL test store should initialize");
    seed_space(&store, 1, 10).await;
    seed_space(&store, 1, 20).await;
    seed_space(&store, 2, 30).await;

    for candidate_id in ["candidate-a", "candidate-b", "candidate-c"] {
        MemoryCandidateStorePort::create(
            &store,
            candidate_command(MemoryScopeContext::for_test(1, 10), candidate_id),
        )
        .await
        .expect("candidate should be inserted");
    }
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(MemoryScopeContext::for_test(1, 20), "candidate-space-20"),
    )
    .await
    .expect("second-space candidate should be inserted");
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(
            MemoryScopeContext::for_test(2, 30),
            "candidate-other-tenant",
        ),
    )
    .await
    .expect("other-tenant candidate should be inserted");

    assert!(store.supports_candidate_listing());

    let first_page = MemoryCandidateStorePort::list_candidates(
        &store,
        ListMemoryCandidatesQuery {
            tenant_id: 1,
            space_id: Some(10),
            page_size: 2,
            cursor: None,
        },
    )
    .await
    .expect("first candidate page should load");
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<Vec<_>>(),
        ["candidate-a", "candidate-b"]
    );
    assert!(first_page.has_more);
    assert_eq!(first_page.next_cursor.as_deref(), Some("candidate-b"));
    assert!(first_page.items.iter().all(|item| item.space_id == 10));

    let second_page = MemoryCandidateStorePort::list_candidates(
        &store,
        ListMemoryCandidatesQuery {
            tenant_id: 1,
            space_id: Some(10),
            page_size: 2,
            cursor: first_page.next_cursor,
        },
    )
    .await
    .expect("second candidate page should load");
    assert_eq!(
        second_page
            .items
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<Vec<_>>(),
        ["candidate-c"]
    );
    assert!(!second_page.has_more);
    assert_eq!(second_page.next_cursor, None);

    let other_space = MemoryCandidateStorePort::list_candidates(
        &store,
        ListMemoryCandidatesQuery {
            tenant_id: 1,
            space_id: Some(20),
            page_size: 20,
            cursor: None,
        },
    )
    .await
    .expect("space filter should load");
    assert_eq!(
        other_space
            .items
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<Vec<_>>(),
        ["candidate-space-20"]
    );

    let other_tenant = MemoryCandidateStorePort::list_candidates(
        &store,
        ListMemoryCandidatesQuery {
            tenant_id: 2,
            space_id: None,
            page_size: 20,
            cursor: None,
        },
    )
    .await
    .expect("tenant filter should load");
    assert_eq!(
        other_tenant
            .items
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<Vec<_>>(),
        ["candidate-other-tenant"]
    );
}

#[tokio::test]
async fn native_sql_candidate_listing_clamps_invalid_provider_page_sizes() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("native SQL test store should initialize");
    seed_space(&store, 1, 1).await;
    for candidate_id in ["candidate-a", "candidate-b"] {
        MemoryCandidateStorePort::create(
            &store,
            candidate_command(MemoryScopeContext::for_test(1, 1), candidate_id),
        )
        .await
        .expect("candidate should be inserted");
    }

    let page = MemoryCandidateStorePort::list_candidates(
        &store,
        ListMemoryCandidatesQuery {
            tenant_id: 1,
            space_id: Some(1),
            page_size: 0,
            cursor: None,
        },
    )
    .await
    .expect("provider should clamp a zero page size to one");
    assert_eq!(page.items.len(), 1);
    assert!(page.has_more);
    assert_eq!(page.next_cursor.as_deref(), Some("candidate-a"));
}
