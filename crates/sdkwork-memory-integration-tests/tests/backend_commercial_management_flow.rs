#![allow(clippy::await_holding_lock)] // Process-wide test environment must remain serialized.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::api_envelope;
use sdkwork_memory_test_support::web_auth::{
    lock_integration_test_env, memory_access_token, memory_auth_token_bearer,
    memory_content_sha256, memory_idempotency_key,
};
use sdkwork_routes_memory_backend_api::{
    build_router_with_backend_api, wrap_router_with_iam_database_web_framework,
};
use sdkwork_web_core::CONTENT_SHA256_HEADER;
use serde_json::json;
use tower::util::ServiceExt;

fn authed_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", memory_auth_token_bearer("9001"))
        .header("Access-Token", memory_access_token("9001"))
        .body(Body::empty())
        .unwrap()
}

fn authed_json(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    let body_text = body.to_string();
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("Authorization", memory_auth_token_bearer("9001"))
        .header("Access-Token", memory_access_token("9001"))
        .header(
            "Idempotency-Key",
            memory_idempotency_key(method, uri, &body_text),
        )
        .header(CONTENT_SHA256_HEADER, memory_content_sha256(&body_text))
        .body(Body::from(body_text))
        .unwrap()
}

#[tokio::test]
async fn backend_commercial_entity_edge_policy_and_readiness_flow() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let pool = store.pool().clone();
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_backend_api(OpenMemoryService::new(store)),
    );

    let subject = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/subjects",
            json!({
                "subjectType": "user",
                "subjectRef": "user:commercial-9001",
                "displayName": "Commercial operator",
                "defaultSpaceId": "1"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(subject.status(), StatusCode::CREATED);
    let subject_body = to_bytes(subject.into_body(), usize::MAX).await.unwrap();
    let subject_json: serde_json::Value = serde_json::from_slice(&subject_body).unwrap();
    let subject_id = api_envelope::item(&subject_json)["subjectId"]
        .as_str()
        .expect("subjectId");

    let binding = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/bindings",
            json!({
                "bindingKind": "access",
                "bindingRole": "viewer",
                "sourceSubjectId": subject_id,
                "targetSpaceId": "1"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(binding.status(), StatusCode::CREATED);

    let capability_binding = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/capability_bindings",
            json!({
                "capabilityCode": "memory.read",
                "targetType": "space",
                "targetId": "1",
                "mode": "allow",
                "priority": 10
            }),
        ))
        .await
        .unwrap();
    assert_eq!(capability_binding.status(), StatusCode::CREATED);

    let entity_a = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/entities",
            json!({
                "spaceId": "1",
                "entityType": "person",
                "canonicalName": "Alice Example"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(entity_a.status(), StatusCode::CREATED);
    let entity_a_body = to_bytes(entity_a.into_body(), usize::MAX).await.unwrap();
    let entity_a_json: serde_json::Value = serde_json::from_slice(&entity_a_body).unwrap();
    let entity_a_id = api_envelope::item(&entity_a_json)["entityId"]
        .as_str()
        .expect("entityId");

    let entity_b = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/entities",
            json!({
                "spaceId": "1",
                "entityType": "person",
                "canonicalName": "Bob Example"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(entity_b.status(), StatusCode::CREATED);
    let entity_b_body = to_bytes(entity_b.into_body(), usize::MAX).await.unwrap();
    let entity_b_json: serde_json::Value = serde_json::from_slice(&entity_b_body).unwrap();
    let entity_b_id = api_envelope::item(&entity_b_json)["entityId"]
        .as_str()
        .expect("entityId");
    assert_ne!(entity_a_id, entity_b_id);

    // A provenance memory the edge can point at (source_memory_id is a real
    // foreign key into ai_record); written straight through the store because
    // the backend face has no memory-create endpoint.
    sqlx::query(
        r#"
        INSERT INTO ai_record (
          id, uuid, tenant_id, space_id, user_id, scope, memory_type,
          subject, predicate, object_text, canonical_text,
          confidence, evidence_count, contradiction_count,
          importance_score, recency_score,
          status, sensitivity_level, created_at, updated_at, version
        )
        VALUES (9001, '99001', 100001, 1, 9001, 'user', 'semantic',
                'person', 'knows', 'Alice knows Bob', 'Alice knows Bob',
                1.0, 1, 0, 0.5, 0.5, 'active', 'internal',
                '2026-09-23T00:00:00.000Z', '2026-09-23T00:00:00.000Z', 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("provenance memory fixture");
    let provenance_memory_id = "99001";

    let edge = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/edges",
            json!({
                "spaceId": "1",
                "sourceEntityId": entity_a_id,
                "targetEntityId": entity_b_id,
                "relationType": "knows",
                "sourceMemoryId": provenance_memory_id
            }),
        ))
        .await
        .unwrap();
    assert_eq!(edge.status(), StatusCode::CREATED);
    let edge_body = to_bytes(edge.into_body(), usize::MAX).await.unwrap();
    let edge_json: serde_json::Value = serde_json::from_slice(&edge_body).unwrap();
    assert_eq!(
        api_envelope::item(&edge_json)["sourceMemoryId"].as_str(),
        Some(provenance_memory_id),
        "the edge must persist and echo its provenance memory id"
    );

    let policy = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/policies",
            json!({
                "policyType": "retrieval",
                "scope": "tenant",
                "policy": { "allowLearning": true }
            }),
        ))
        .await
        .unwrap();
    assert_eq!(policy.status(), StatusCode::CREATED);
    let policy_body = to_bytes(policy.into_body(), usize::MAX).await.unwrap();
    let policy_json: serde_json::Value = serde_json::from_slice(&policy_body).unwrap();
    let policy_id = api_envelope::item(&policy_json)["policyId"]
        .as_str()
        .expect("policyId");

    let assignment = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/policy_assignments",
            json!({
                "policyId": policy_id,
                "targetType": "space",
                "targetId": "1",
                "inheritanceMode": "inherit"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(assignment.status(), StatusCode::CREATED);

    let resolved = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/capabilities/resolve",
            json!({
                "targetType": "space",
                "targetId": "1"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resolved.status(), StatusCode::OK);
    let resolved_body = to_bytes(resolved.into_body(), usize::MAX).await.unwrap();
    let resolved_json: serde_json::Value = serde_json::from_slice(&resolved_body).unwrap();
    api_envelope::assert_cursor_page_info(&resolved_json);

    let rebuild = app
        .clone()
        .oneshot(authed_json(
            "POST",
            "/backend/v3/api/memory/commercial_readiness/rebuild",
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(rebuild.status(), StatusCode::OK);
    let rebuild_body = to_bytes(rebuild.into_body(), usize::MAX).await.unwrap();
    let rebuild_json: serde_json::Value = serde_json::from_slice(&rebuild_body).unwrap();
    let readiness_item = api_envelope::item(&rebuild_json);
    assert!(readiness_item["score"].as_f64().unwrap_or(0.0) > 0.0);
    assert_eq!(
        readiness_item["managementCoverage"]["entities"].as_i64(),
        Some(2)
    );

    let list = app
        .clone()
        .oneshot(authed_get("/backend/v3/api/memory/entities?space_id=1"))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    api_envelope::assert_cursor_page_info(&list_json);
    assert_eq!(api_envelope::items(&list_json).as_array().unwrap().len(), 2);

    let graph_outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND event_type IN ('memory.entity.created', 'memory.edge.created')",
    )
    .bind(100_001_i64)
    .fetch_one(&pool)
    .await
    .expect("count graph outbox events");
    let graph_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND action IN ('memory.entity.created', 'memory.edge.created') AND actor_id = ?",
    )
    .bind(100_001_i64)
    .bind("9001")
    .fetch_one(&pool)
    .await
    .expect("count graph audit events");
    assert_eq!(graph_outbox_count, 3);
    assert_eq!(graph_audit_count, 3);

    let system_commercial_outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND event_type IN ('memory.subject.created', 'memory.binding.created', 'memory.capability_binding.created', 'memory.policy.created', 'memory.policy_assignment.created')",
    )
    .bind(100_001_i64)
    .fetch_one(&pool)
    .await
    .expect("count system commercial outbox events");
    let system_commercial_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND actor_type = 'system' AND action IN ('memory.subject.created', 'memory.binding.created', 'memory.capability_binding.created', 'memory.policy.created', 'memory.policy_assignment.created')",
    )
    .bind(100_001_i64)
    .fetch_one(&pool)
    .await
    .expect("count system commercial audit events");
    assert_eq!(system_commercial_outbox_count, 5);
    assert_eq!(system_commercial_audit_count, 5);
}
