//! Contract tests for the governance-surface repairs: the forget request that
//! records its running state before the destructive sweep (and never calls a
//! zero-hit completion a failure), review reasons that reach the persisted
//! decision evidence, audit-log and retrieval-trace reads that surface the
//! stored row values, app-surface policy-assignment target authorization, and
//! subject-scoped capability bindings that bind outside the space row.

use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    CapabilityMode, CapabilityTargetType, CreateCapabilityBindingCommand,
    CreatePolicyAssignmentCommand, CreatePolicyCommand, ListAuditLogsQuery, ListCandidatesQuery,
    ListJobsQuery, MemoryAppApi, MemoryAppRequestContext, MemoryBackendApi,
    MemoryBackendRequestContext, MemoryEventRequest, MemoryExtractionRequest, MemoryForgetRequest,
    MemoryOpenApi, MemoryOpenApiRequestContext, MemoryRecordRequest, MemoryRetrievalRequest,
    MemoryReviewRequest, MemoryServiceErrorKind, MemorySpaceRequest, MemoryType,
    PolicyAssignmentTargetType, PolicyInheritanceMode, UpdatePolicyAssignmentCommand,
};
use sdkwork_memory_plugin_native_sql::{
    InsertSubjectCommand, NativeSqlCreateSpaceCommand, NativeSqlMemoryStore,
};
use sdkwork_memory_spi::MemoryScopeContext;
use serde_json::json;

const TENANT_ID: u64 = 91_401;
const ACTOR_ID: u64 = 42;
const OUTSIDER_ID: u64 = 9_999;

fn app_context() -> MemoryAppRequestContext {
    MemoryAppRequestContext {
        tenant_id: TENANT_ID,
        actor_id: Some(ACTOR_ID),
        organization_id: None,
        session_id: None,
    }
}

fn open_context() -> MemoryOpenApiRequestContext {
    MemoryOpenApiRequestContext::for_open_surface(
        "governance-repairs-key",
        TENANT_ID,
        Some(ACTOR_ID),
    )
}

fn backend_context() -> MemoryBackendRequestContext {
    MemoryBackendRequestContext {
        tenant_id: TENANT_ID,
        operator_id: Some(ACTOR_ID),
    }
}

fn space_request(display_name: &str) -> MemorySpaceRequest {
    MemorySpaceRequest {
        organization_id: None,
        owner_subject_type: "user".to_string(),
        owner_subject_id: ACTOR_ID.to_string(),
        space_type: "workspace".to_string(),
        display_name: display_name.to_string(),
        default_scope: None,
        metadata: None,
    }
}

async fn store_with_owned_space() -> NativeSqlMemoryStore {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("governance repair sqlite store must open");
    store
        .create_space_record(
            TENANT_ID as i64,
            1,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: ACTOR_ID.to_string(),
                space_type: "workspace".to_string(),
                display_name: "Governance Repair Space".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .expect("governance repair space must be created");
    store
}

fn memory_request(text: &str) -> MemoryRecordRequest {
    MemoryRecordRequest {
        space_id: 1,
        scope: "user".to_string(),
        memory_type: MemoryType::Semantic,
        subject: Some("governance".to_string()),
        predicate: Some("matches".to_string()),
        object_text: Some(text.to_string()),
        canonical_text: text.to_string(),
        summary_text: None,
        user_id: Some(ACTOR_ID),
        language: Some("en".to_string()),
        sensitivity_level: Some("internal".to_string()),
        metadata: None,
        tags: None,
        expires_at: None,
    }
}

fn conversation_event(content: &str) -> MemoryEventRequest {
    MemoryEventRequest {
        space_id: 1,
        user_id: None,
        actor_type: Some("user".to_string()),
        actor_id: Some(ACTOR_ID.to_string()),
        session_id: None,
        trace_id: None,
        event_type: "conversation.turn".to_string(),
        source_type: "conversation".to_string(),
        source_ref: None,
        event_time: "2026-09-24T00:00:00.000Z".to_string(),
        payload: json!({ "content": content }),
        sensitivity_level: None,
    }
}

fn audit_scope() -> MemoryScopeContext {
    MemoryScopeContext {
        tenant_id: TENANT_ID as i64,
        space_id: 1,
        organization_id: None,
        user_id: None,
    }
}

/// D3: the running governance row is persisted before the sweep, under a
/// lifecycle-only resource type, so a crash or timeout mid-scan leaves a
/// traceable "running" audit row (PRD privacy traceability). The id-keyed job
/// reads keep resolving exactly the terminal `forget_job` row, and the job
/// list stays one entry per request.
#[tokio::test]
async fn forget_request_records_running_row_before_the_terminal_result() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store.clone());
    let memory = MemoryOpenApi::create_memory(
        &service,
        open_context(),
        memory_request("forget audit target"),
    )
    .await
    .unwrap();

    let job = service
        .create_forget_request(
            app_context(),
            MemoryForgetRequest {
                scope: "memory".to_string(),
                memory_ids: Some(vec![memory.memory_id]),
                space_id: Some(1),
                query: None,
                reason: "user erasure request".to_string(),
                metadata: None,
            },
        )
        .await
        .expect("a targeted forget must complete");
    assert_eq!(job.state, "succeeded");
    assert_eq!(job.result.as_ref().unwrap()["deletedCount"], 1);

    // The id-keyed read resolves the terminal row, not the running one.
    let retrieved = service
        .retrieve_forget_request(app_context(), job.forget_request_id)
        .await
        .unwrap();
    assert_eq!(retrieved.state, "succeeded");

    // The job list stays one entry per request: the running row is not a job.
    let listed = service
        .list_forget_requests(
            app_context(),
            ListJobsQuery {
                cursor: None,
                page_size: None,
                space_id: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].state, "succeeded");

    // The running row exists on the audit surface, keyed to the same job, and
    // carries the running state — exactly what a crashed sweep leaves behind.
    let started = store
        .list_audit_logs_for_tenant(TENANT_ID as i64, Some("forget.request.start"), 50, None)
        .await
        .unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].resource_type, "forget_job_progress");
    assert_eq!(started[0].result, "running");
    assert_eq!(
        started[0].resource_id,
        job.forget_request_id.to_string(),
        "the lifecycle row must name the job through resource_id"
    );
    assert_eq!(started[0].actor_id.as_deref(), Some("42"));
    let running_job: serde_json::Value =
        serde_json::from_str(started[0].metadata_json.as_deref().unwrap()).unwrap();
    assert_eq!(running_job["state"], "running");

    let completed = store
        .list_audit_logs_for_tenant(TENANT_ID as i64, Some("forget.request.create"), 50, None)
        .await
        .unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].resource_type, "forget_job");
}

/// A malformed forget request is rejected before any governance row exists, so
/// a validation failure cannot strand a phantom running job.
#[tokio::test]
async fn invalid_forget_request_leaves_no_running_row_behind() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store.clone());
    let error = service
        .create_forget_request(
            app_context(),
            MemoryForgetRequest {
                scope: "elsewhere".to_string(),
                memory_ids: None,
                space_id: None,
                query: None,
                reason: "invalid".to_string(),
                metadata: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, MemoryServiceErrorKind::Validation);
    assert!(
        store
            .list_audit_logs_for_tenant(TENANT_ID as i64, Some("forget.request.start"), 50, None,)
            .await
            .unwrap()
            .is_empty(),
        "a request that never scanned must not leave a running row"
    );
}

/// D7: a sweep that matched nothing is a legal completed outcome (idempotent
/// replay or no matches), answered as `succeeded` with its zero counts — not a
/// failure. Real faults return through the store error paths instead.
#[tokio::test]
async fn forget_request_with_no_matches_completes_as_succeeded() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store);
    let job = service
        .create_forget_request(
            app_context(),
            MemoryForgetRequest {
                scope: "query".to_string(),
                memory_ids: None,
                space_id: Some(1),
                query: Some("no record carries this needle".to_string()),
                reason: "idempotent replay".to_string(),
                metadata: None,
            },
        )
        .await
        .expect("a zero-hit sweep is a completed run");
    assert_eq!(job.state, "succeeded");
    assert_eq!(job.result.as_ref().unwrap()["deletedCount"], 0);
    assert_eq!(job.result.as_ref().unwrap()["purgedEvents"], 0);
    assert_eq!(job.result.as_ref().unwrap()["rejectedCandidates"], 0);
}

/// D5: the operator's typed review reason reaches the persisted decision
/// evidence — the candidate row's `decision_reason` on reject, and a
/// candidate-keyed audit row on approve (the index-rebuild precedent).
#[tokio::test]
async fn candidate_review_persists_the_operator_reason() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store.clone());
    let context = open_context();
    let first_event = MemoryOpenApi::create_event(
        &service,
        context.clone(),
        conversation_event("review bait one"),
    )
    .await
    .unwrap();
    let second_event = MemoryOpenApi::create_event(
        &service,
        context.clone(),
        conversation_event("review bait two"),
    )
    .await
    .unwrap();
    let summary = service
        .run_extraction_now(
            context.clone(),
            MemoryExtractionRequest {
                space_id: 1,
                input_events: vec![first_event.event_id, second_event.event_id],
                extraction_mode: Some("deterministic".to_string()),
                custom_instructions: None,
                observation_date: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(summary["candidateCount"], 2);

    let candidates = MemoryAppApi::list_candidates(
        &service,
        app_context(),
        ListCandidatesQuery {
                cursor: None,
                page_size: None,
                space_id: Some(1),
            },
        )
        .await
        .unwrap();
    assert_eq!(candidates.items.len(), 2);
    let mut candidate_ids: Vec<u64> = candidates
        .items
        .iter()
        .map(|candidate| candidate.candidate_id)
        .collect();
    candidate_ids.sort_unstable();
    let (reject_id, approve_id) = (candidate_ids[0], candidate_ids[1]);

    let rejected = MemoryAppApi::reject_candidate(
        &service,
        app_context(),
        reject_id,
            MemoryReviewRequest {
                reason: Some("not stable".to_string()),
                reviewer_note: Some("duplicate of another memory".to_string()),
                metadata: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(rejected.decision_state, "rejected");
    let rejected_row = store
        .retrieve_candidate(&audit_scope(), &reject_id.to_string())
        .await
        .unwrap()
        .expect("the rejected candidate must exist");
    assert_eq!(
        rejected_row.decision_reason.as_deref(),
        Some("not stable"),
        "the typed review reason must reach the candidate row"
    );

    let approved = MemoryAppApi::approve_candidate(
        &service,
        app_context(),
        approve_id,
            MemoryReviewRequest {
                reason: Some("verified against the source event".to_string()),
                reviewer_note: None,
                metadata: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(approved.decision_state, "approved");
    let review_audits = store
        .list_audit_logs_for_tenant(
            TENANT_ID as i64,
            Some("memory.candidate.approved"),
            50,
            None,
        )
        .await
        .unwrap();
    assert_eq!(review_audits.len(), 1);
    assert_eq!(review_audits[0].resource_type, "memory_candidate");
    assert_eq!(review_audits[0].resource_id, approve_id.to_string());
    let review: serde_json::Value =
        serde_json::from_str(review_audits[0].metadata_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        review["reason"], "verified against the source event",
        "the approve review payload must carry the operator reason"
    );
    assert!(review["reviewerNote"].is_null());
}

/// D4: the trace read surfaces the stored row values — the trace's own
/// creation timestamp and its recorded space — not read-time placeholders.
#[tokio::test]
async fn retrieval_trace_read_surfaces_the_stored_timestamp_and_space() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store.clone());
    MemoryOpenApi::create_memory(&service, open_context(), memory_request("trace probe memory"))
        .await
        .unwrap();
    let result = MemoryOpenApi::create_retrieval(
        &service,
        open_context(),
            MemoryRetrievalRequest {
                query: "trace probe".to_string(),
                space_ids: vec![1],
                actor_id: Some(ACTOR_ID.to_string()),
                retrieval_profile_id: None,
                memory_types: None,
                filters: None,
                top_k: 5,
                context_budget_tokens: 512,
                show_expired: None,
                threshold: None,
                explain: None,
                include_trace: Some(true),
            },
        )
        .await
        .unwrap();
    let trace_id = result
        .trace
        .as_ref()
        .expect("includeTrace must echo the trace")
        .trace_id;

    let stored = store
        .retrieve_retrieval_trace_for_tenant(TENANT_ID as i64, &trace_id.to_string())
        .await
        .unwrap()
        .expect("the retrieval trace must be persisted");
    let trace = service
        .retrieve_retrieval_trace(backend_context(), trace_id)
        .await
        .unwrap();
    assert_eq!(
        trace.created_at, stored.created_at,
        "the trace read must surface the stored creation timestamp"
    );
    assert_eq!(
        trace.space_id,
        stored
            .space_id
            .map(|value| u64::try_from(value).expect("space id must be non-negative")),
        "the trace read must surface the recorded space"
    );
    assert_eq!(trace.space_id, Some(1));
}

/// D6: the audit log mapping surfaces the row's own correlation and payload
/// columns instead of hardcoded `None`s — the governance job rows written with
/// metadata come back with it.
#[tokio::test]
async fn audit_log_mapping_surfaces_row_metadata() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store);
    let job = service
        .create_forget_request(
            app_context(),
            MemoryForgetRequest {
                scope: "query".to_string(),
                memory_ids: None,
                space_id: Some(1),
                query: Some("nothing matches this either".to_string()),
                reason: "audit mapping probe".to_string(),
                metadata: None,
            },
        )
        .await
        .unwrap();

    let audits = service
        .list_audit_logs(
            backend_context(),
            ListAuditLogsQuery {
                cursor: None,
                page_size: None,
                action: Some("forget.request.create".to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(audits.items.len(), 1);
    let audit = &audits.items[0];
    let metadata = audit
        .metadata
        .as_ref()
        .expect("governance job rows carry their job payload as metadata");
    assert_eq!(metadata["state"], "succeeded");
    assert_eq!(
        metadata["forgetRequestId"]
            .as_str()
            .expect("wire ids serialize as strings"),
        job.forget_request_id.to_string()
    );
    assert_eq!(audit.reason, None);
    assert_eq!(audit.trace_id, None);
}

fn policy_command() -> CreatePolicyCommand {
    CreatePolicyCommand {
        tenant_id: TENANT_ID,
        policy_type: "retrieval".to_string(),
        scope: "tenant".to_string(),
        scope_ref: None,
        policy: json!({ "allowLearning": true }),
    }
}

/// D9: app-surface policy assignment authorization. A subject target must be
/// the acting subject, a space target must pass the space write gate, and
/// target types with no reviewed app-surface evaluator fail closed. Updates
/// are authorized against the target the existing assignment already names.
#[tokio::test]
async fn policy_assignment_for_actor_enforces_target_authorization() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store);
    let policy = service.create_policy(policy_command()).await.unwrap();

    let outsider = MemoryOpenApiRequestContext::for_open_surface(
        "governance-repairs-outsider-key",
        TENANT_ID,
        Some(OUTSIDER_ID),
    );

    // A subject target may only be the actor itself.
    let foreign_subject = service
        .create_policy_assignment_for_actor(
            open_context(),
            CreatePolicyAssignmentCommand {
                tenant_id: TENANT_ID,
                policy_id: policy.policy_id.clone(),
                target_type: PolicyAssignmentTargetType::Subject,
                target_id: OUTSIDER_ID,
                priority: 0,
                inheritance_mode: PolicyInheritanceMode::Inherit,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(foreign_subject.kind, MemoryServiceErrorKind::Forbidden);

    let assignment = service
        .create_policy_assignment_for_actor(
            open_context(),
            CreatePolicyAssignmentCommand {
                tenant_id: TENANT_ID,
                policy_id: policy.policy_id.clone(),
                target_type: PolicyAssignmentTargetType::Subject,
                target_id: ACTOR_ID,
                priority: 0,
                inheritance_mode: PolicyInheritanceMode::Inherit,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .expect("an actor may target itself");

    // A space target requires the space write gate; the outsider has none.
    let unwritable_space = service
        .create_policy_assignment_for_actor(
            outsider.clone(),
            CreatePolicyAssignmentCommand {
                tenant_id: TENANT_ID,
                policy_id: policy.policy_id.clone(),
                target_type: PolicyAssignmentTargetType::Space,
                target_id: 1,
                priority: 0,
                inheritance_mode: PolicyInheritanceMode::Inherit,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(unwritable_space.kind, MemoryServiceErrorKind::Forbidden);

    // Target types without a reviewed app-surface evaluator fail closed.
    let unevaluated_target = service
        .create_policy_assignment_for_actor(
            open_context(),
            CreatePolicyAssignmentCommand {
                tenant_id: TENANT_ID,
                policy_id: policy.policy_id.clone(),
                target_type: PolicyAssignmentTargetType::Entity,
                target_id: 1,
                priority: 0,
                inheritance_mode: PolicyInheritanceMode::Inherit,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(unevaluated_target.kind, MemoryServiceErrorKind::Validation);

    // The update path authorizes against the existing assignment's target:
    // the assignment names ACTOR, so the outsider may not mutate it.
    let denied_update = service
        .update_policy_assignment_for_actor(
            outsider,
            TENANT_ID,
            &assignment.policy_assignment_id,
            UpdatePolicyAssignmentCommand {
                priority: Some(5),
                inheritance_mode: None,
                status: None,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(denied_update.kind, MemoryServiceErrorKind::Forbidden);

    let allowed_update = service
        .update_policy_assignment_for_actor(
            open_context(),
            TENANT_ID,
            &assignment.policy_assignment_id,
            UpdatePolicyAssignmentCommand {
                priority: Some(5),
                inheritance_mode: None,
                status: None,
                valid_from: None,
                valid_to: None,
            },
        )
        .await
        .expect("the acting subject may mutate its own assignment");
    assert_eq!(allowed_update.priority, 5);
}

/// D2: a capability deny recorded on the actor's subject row (target_type
/// `subject`, target_id = the subject's internal numeric id) binds outside the
/// space row: the space-owner actor passes the ownership gate but still hits
/// the subject-scoped deny on writes, while another subject's binding does not
/// apply, and reads (a different capability code) stay open.
#[tokio::test]
async fn subject_scoped_capability_binding_denies_write_for_the_bound_actor() {
    let store = store_with_owned_space().await;
    let actor_ref = ACTOR_ID.to_string();
    store
        .insert_subject(InsertSubjectCommand {
            id: 501,
            uuid: "501",
            tenant_id: TENANT_ID as i64,
            organization_id: None,
            subject_type: "user",
            subject_ref: &actor_ref,
            display_name: "bound actor",
            default_space_id: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    store
        .insert_capability_binding(
            9001,
            "cap-deny-write-subject",
            TENANT_ID as i64,
            "memory.write",
            "subject",
            501,
            "deny",
            100,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let service = OpenMemoryService::new(store);

    // The owner passes ownership and the (absent) binding gate, then hits the
    // subject-scoped capability deny.
    let denied = MemoryAppApi::update_space(
        &service,
        app_context(),
        1,
        space_request("Denied Rename"),
    )
    .await
    .unwrap_err();
    assert_eq!(denied.kind, MemoryServiceErrorKind::Forbidden);
    assert!(
        denied.detail.contains("capability policy"),
        "the denial must come from the capability gate, got: {}",
        denied.detail
    );

    // The read gate does not enforce the write-scoped deny: reads stay open.
    MemoryAppApi::retrieve_space(&service, app_context(), 1)
        .await
        .expect("a write-scoped subject deny must not close reads");
}

/// D2 (negative): a subject binding that targets a different subject never
/// applies — the join must resolve the actor's own subject row, not just any
/// subject-targeted binding in the tenant.
#[tokio::test]
async fn subject_capability_binding_for_another_subject_does_not_apply() {
    let store = store_with_owned_space().await;
    store
        .insert_subject(InsertSubjectCommand {
            id: 502,
            uuid: "502",
            tenant_id: TENANT_ID as i64,
            organization_id: None,
            subject_type: "user",
            subject_ref: "someone-else",
            display_name: "other subject",
            default_space_id: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    store
        .insert_capability_binding(
            9002,
            "cap-deny-write-other-subject",
            TENANT_ID as i64,
            "memory.write",
            "subject",
            502,
            "deny",
            100,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let service = OpenMemoryService::new(store);

    MemoryAppApi::update_space(&service, app_context(), 1, space_request("Unbound Rename"))
        .await
        .expect("another subject's capability deny must not apply to this actor");
}

/// D2b: phase-1 capability bindings may only target a space or a subject —
/// `binding` and `memory` targets have no reviewed evaluator and are rejected
/// with a typed validation error instead of being stored outside enforcement.
#[tokio::test]
async fn capability_binding_creation_rejects_phase1_unsupported_targets() {
    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store);

    for target_type in [CapabilityTargetType::Binding, CapabilityTargetType::Memory] {
        let error = service
            .create_capability_binding(CreateCapabilityBindingCommand {
                tenant_id: TENANT_ID,
                capability_code: "memory.write".to_string(),
                target_type,
                target_id: 1,
                mode: CapabilityMode::Deny,
                priority: 0,
                valid_from: None,
                valid_to: None,
                metadata: None,
            })
            .await
            .unwrap_err();
        assert_eq!(error.kind, MemoryServiceErrorKind::Validation);
        assert!(error.detail.contains("space, subject"));
    }

    let binding = service
        .create_capability_binding(CreateCapabilityBindingCommand {
            tenant_id: TENANT_ID,
            capability_code: "memory.write".to_string(),
            target_type: CapabilityTargetType::Space,
            target_id: 1,
            mode: CapabilityMode::Allow,
            priority: 10,
            valid_from: None,
            valid_to: None,
            metadata: None,
        })
        .await
        .expect("space-targeted bindings stay supported");
    assert_eq!(binding.target_type, CapabilityTargetType::Space);
}
