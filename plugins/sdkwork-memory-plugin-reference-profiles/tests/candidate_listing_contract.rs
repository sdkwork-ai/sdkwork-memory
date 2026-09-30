use sdkwork_memory_plugin_reference_profiles::ReferenceMemoryRuntime;
use sdkwork_memory_spi::{
    CreateMemoryCandidateCommand, ListMemoryCandidatesQuery, MemoryCandidateStorePort,
    MemoryScopeContext, MemorySpiError,
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

async fn create_candidate(
    runtime: &ReferenceMemoryRuntime,
    tenant_id: i64,
    space_id: i64,
    candidate_id: &str,
) {
    MemoryCandidateStorePort::create(
        runtime,
        candidate_command(
            MemoryScopeContext::for_test(tenant_id, space_id),
            candidate_id,
        ),
    )
    .await
    .expect("reference candidate should be inserted");
}

#[tokio::test]
async fn reference_candidate_listing_is_tenant_scoped_and_cursor_bounded() {
    let runtime = ReferenceMemoryRuntime::new();
    create_candidate(&runtime, 1, 10, "candidate-a").await;
    create_candidate(&runtime, 1, 10, "candidate-b").await;
    create_candidate(&runtime, 1, 10, "candidate-c").await;
    create_candidate(&runtime, 1, 20, "candidate-space-20").await;
    create_candidate(&runtime, 2, 30, "candidate-other-tenant").await;

    assert!(runtime.supports_candidate_listing());

    let first_page = MemoryCandidateStorePort::list_candidates(
        &runtime,
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
    assert!(first_page
        .items
        .iter()
        .all(|item| item.created_at.ends_with('Z') && item.updated_at.ends_with('Z')));

    let second_page = MemoryCandidateStorePort::list_candidates(
        &runtime,
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

    let other_tenant = MemoryCandidateStorePort::list_candidates(
        &runtime,
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
async fn reference_candidate_listing_rejects_ambiguous_cross_space_cursor() {
    let runtime = ReferenceMemoryRuntime::new();
    create_candidate(&runtime, 7, 70, "candidate-before").await;
    create_candidate(&runtime, 7, 70, "candidate-shared").await;
    create_candidate(&runtime, 7, 71, "candidate-shared").await;

    let error = MemoryCandidateStorePort::list_candidates(
        &runtime,
        ListMemoryCandidatesQuery {
            tenant_id: 7,
            space_id: None,
            page_size: 1,
            cursor: None,
        },
    )
    .await
    .expect_err("a candidate id cannot be an unambiguous cursor across spaces");
    assert!(matches!(
        error,
        MemorySpiError::PortOperationFailed { port, message }
            if port == "MemoryCandidateStorePort"
                && message.contains("ambiguous")
    ));
}

#[tokio::test]
async fn reference_candidate_listing_clamps_invalid_provider_page_sizes() {
    let runtime = ReferenceMemoryRuntime::new();
    create_candidate(&runtime, 1, 1, "candidate-a").await;
    create_candidate(&runtime, 1, 1, "candidate-b").await;

    let page = MemoryCandidateStorePort::list_candidates(
        &runtime,
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
