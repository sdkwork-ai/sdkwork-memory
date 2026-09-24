use std::collections::BTreeSet;

use sdkwork_memory_contract::{
    MemoryOpenApiRequestContext, MemoryServiceError, MemoryServiceResult,
};
use sdkwork_memory_spi::{
    MemoryActorSpaceBindingFact, MemoryCapabilityBindingFact, MemoryGovernanceActor,
    MemoryScopeContext, MemorySpaceGovernanceFacts, ResolveMemorySpaceGovernanceQuery,
    MAX_MEMORY_GOVERNANCE_FACTS,
};

use crate::platform;
use crate::runtime_data_plane::MemoryRuntimeDataPlane;

// ---------------------------------------------------------------------------
// Commercial capability codes
// ---------------------------------------------------------------------------

/// Capability code controlling memory retrieval operations.
pub const CAPABILITY_MEMORY_RETRIEVE: &str = "memory.retrieve";

/// Capability code controlling memory write operations.
pub const CAPABILITY_MEMORY_WRITE: &str = "memory.write";

/// Check whether a capability binding is currently within its validity period.
fn invalid_governance_state(detail: &'static str) -> MemoryServiceError {
    tracing::error!(detail, "memory governance state is invalid; failing closed");
    MemoryServiceError::storage(detail)
}

fn fact_is_current(
    valid_from: &Option<String>,
    valid_to: &Option<String>,
    now: &str,
) -> MemoryServiceResult<bool> {
    let now = sdkwork_utils_rust::parse_datetime(now, None)
        .ok_or_else(|| invalid_governance_state("runtime governance evaluation time is invalid"))?;
    if let Some(valid_from) = valid_from {
        let valid_from = sdkwork_utils_rust::parse_datetime(valid_from, None)
            .ok_or_else(|| invalid_governance_state("governance validFrom timestamp is invalid"))?;
        if now < valid_from {
            return Ok(false);
        }
    }
    if let Some(valid_to) = valid_to {
        let valid_to = sdkwork_utils_rust::parse_datetime(valid_to, None)
            .ok_or_else(|| invalid_governance_state("governance validTo timestamp is invalid"))?;
        if now > valid_to {
            return Ok(false);
        }
    }
    Ok(true)
}

fn actor_has_active_binding(
    bindings: &[MemoryActorSpaceBindingFact],
    require_write: bool,
    now: &str,
) -> MemoryServiceResult<bool> {
    let mut granted = false;
    for binding in bindings {
        if binding.binding_id.trim().is_empty() || binding.binding_role.trim().is_empty() {
            return Err(invalid_governance_state(
                "governance binding identifiers and roles must not be blank",
            ));
        }
        let active = match binding.status.as_str() {
            "active" => true,
            "disabled" | "expired" | "deleted" => false,
            _ => {
                return Err(invalid_governance_state(
                    "governance binding status is unsupported",
                ))
            }
        };
        let grants_space_access = match binding.binding_kind.as_str() {
            "access" | "share" | "ownership" => true,
            "reference" | "provision" => false,
            _ => {
                return Err(invalid_governance_state(
                    "governance binding kind is unsupported",
                ))
            }
        };
        let role_can_write = match binding.binding_role.as_str() {
            "owner" | "learner" => true,
            "viewer" | "retriever" | "context_source" | "evidence" | "correction"
            | "suppression" | "import_shadow" => false,
            _ => {
                return Err(invalid_governance_state(
                    "governance binding role is unsupported",
                ))
            }
        };
        let current = fact_is_current(&binding.valid_from, &binding.valid_to, now)?;
        if active && current && grants_space_access && (!require_write || role_can_write) {
            granted = true;
        }
    }
    Ok(granted)
}

fn capability_is_denied(
    bindings: &[MemoryCapabilityBindingFact],
    capability_code: &str,
    now: &str,
) -> MemoryServiceResult<bool> {
    let mut denied = false;
    for binding in bindings {
        if binding.binding_id.trim().is_empty() || binding.capability_code != capability_code {
            return Err(invalid_governance_state(
                "governance capability fact does not match the requested capability",
            ));
        }
        let active = match binding.status.as_str() {
            "active" => true,
            "disabled" | "deleted" => false,
            _ => {
                return Err(invalid_governance_state(
                    "governance capability status is unsupported",
                ))
            }
        };
        let mode = match binding.mode.as_str() {
            "allow" => CapabilityDecision::Allow,
            "deny" => CapabilityDecision::Deny,
            "conditional" => CapabilityDecision::Conditional,
            _ => {
                return Err(invalid_governance_state(
                    "governance capability mode is unsupported",
                ))
            }
        };
        let current = fact_is_current(&binding.valid_from, &binding.valid_to, now)?;
        if active && current {
            match mode {
                CapabilityDecision::Allow => {}
                CapabilityDecision::Deny => denied = true,
                CapabilityDecision::Conditional => {
                    return Err(invalid_governance_state(
                        "conditional capability policy has no reviewed evaluator",
                    ))
                }
            }
        }
    }
    Ok(denied)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityDecision {
    Allow,
    Deny,
    Conditional,
}

/// Resolve capabilities for a space and determine whether the given capability
/// code is explicitly denied. Deny wins over allow per commercial management
/// design §8.2. Backend operators with elevated tenant access bypass this check.
async fn resolve_governance_facts(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
    capability_code: Option<&str>,
) -> MemoryServiceResult<MemorySpaceGovernanceFacts> {
    let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
    let space_id_i64 = platform::space_id_i64(space_id)?;
    let facts = data_plane
        .resolve_space_governance(ResolveMemorySpaceGovernanceQuery {
            scope: MemoryScopeContext {
                tenant_id,
                space_id: space_id_i64,
                organization_id: None,
                user_id: context
                    .actor_id
                    .and_then(|actor_id| i64::try_from(actor_id).ok()),
            },
            actor: context.actor_id.map(|actor_id| MemoryGovernanceActor {
                // The current HTTP context carries only an actor id. Providers detect and reject
                // ambiguous identifiers until IAM supplies a trusted subject type.
                subject_type: None,
                subject_id: actor_id.to_string(),
            }),
            capability_code: capability_code.map(str::to_string),
            fact_limit: MAX_MEMORY_GOVERNANCE_FACTS,
        })
        .await?;
    if !facts.complete {
        return Err(invalid_governance_state(
            "governance fact set exceeded the bounded authorization limit",
        ));
    }
    if facts
        .space
        .as_ref()
        .is_some_and(|space| space.space_id != space_id_i64)
    {
        return Err(invalid_governance_state(
            "governance provider returned a mismatched space",
        ));
    }
    Ok(facts)
}

fn forbidden_error(detail: impl Into<String>) -> MemoryServiceError {
    crate::domain_metrics::memory_domain_metrics().record_authz_denied();
    MemoryServiceError::forbidden(detail)
}

fn forbidden(detail: impl Into<String>) -> MemoryServiceResult<()> {
    Err(forbidden_error(detail))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySpaceAuthorization {
    pub space_id: u64,
    pub actor_is_space_owner: bool,
}

fn actor_is_space_owner(
    facts: &MemorySpaceGovernanceFacts,
    context: &MemoryOpenApiRequestContext,
) -> bool {
    let Some(space) = facts.space.as_ref() else {
        return false;
    };
    let Some(actor_id) = context.actor_id else {
        return false;
    };
    matches!(space.owner_subject_type.as_str(), "user" | "agent")
        && space.owner_subject_id == actor_id.to_string()
}

async fn authorize_actor_for_space(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
    require_write: bool,
    capability_code: Option<&'static str>,
) -> MemoryServiceResult<MemorySpaceAuthorization> {
    let capability_query = if context.elevated_tenant_access {
        None
    } else {
        capability_code
    };
    let facts = resolve_governance_facts(data_plane, context, space_id, capability_query).await?;
    authorize_actor_for_existing_space(
        &facts,
        context,
        space_id,
        require_write,
        capability_query,
        &platform::current_timestamp(),
    )
}

fn authorize_actor_for_existing_space(
    facts: &MemorySpaceGovernanceFacts,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
    require_write: bool,
    capability_code: Option<&'static str>,
    evaluated_at: &str,
) -> MemoryServiceResult<MemorySpaceAuthorization> {
    let Some(space) = facts.space.as_ref() else {
        return Err(MemoryServiceError::not_found("memory space not found"));
    };

    match space.lifecycle_status.as_str() {
        "active" => {}
        "archived" if require_write => {
            return Err(forbidden_error("archived memory spaces are read-only"));
        }
        "archived" => {}
        "deleted" => return Err(MemoryServiceError::not_found("memory space not found")),
        _ => {
            return Err(invalid_governance_state(
                "memory space lifecycle status is unsupported",
            ));
        }
    }

    let actor_is_space_owner = actor_is_space_owner(facts, context);
    let authorization = MemorySpaceAuthorization {
        space_id,
        actor_is_space_owner,
    };

    if context.elevated_tenant_access {
        tracing::warn!(
            tenant_id = context.tenant_id,
            space_id,
            actor_id = ?context.actor_id,
            "elevated_tenant_access bypass: ownership, binding, and capability checks skipped"
        );
        return Ok(authorization);
    }

    if context.actor_id.is_none() {
        return Err(forbidden_error(
            "authenticated actor is required for memory space access",
        ));
    }

    if !actor_is_space_owner
        && !actor_has_active_binding(&facts.actor_bindings, require_write, evaluated_at)?
    {
        return Err(forbidden_error(
            "actor is not authorized for this memory space",
        ));
    }

    if let Some(capability_code) = capability_code {
        if capability_is_denied(&facts.capability_bindings, capability_code, evaluated_at)? {
            tracing::warn!(
                tenant_id = context.tenant_id,
                space_id,
                capability_code,
                "memory operation denied by capability binding"
            );
            let detail = match capability_code {
                CAPABILITY_MEMORY_RETRIEVE => {
                    "retrieval denied for this memory space by capability policy"
                }
                CAPABILITY_MEMORY_WRITE => {
                    "write denied for this memory space by capability policy"
                }
                _ => "memory operation denied for this space by capability policy",
            };
            return Err(forbidden_error(detail));
        }
    }

    Ok(authorization)
}

/// Resolve access, lifecycle, ownership, and retrieval capability from one
/// bounded governance snapshot.
pub async fn authorize_actor_for_space_retrieval(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
) -> MemoryServiceResult<MemorySpaceAuthorization> {
    authorize_actor_for_space(
        data_plane,
        context,
        space_id,
        false,
        Some(CAPABILITY_MEMORY_RETRIEVE),
    )
    .await
}

pub async fn authorize_actor_for_space_access(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
) -> MemoryServiceResult<MemorySpaceAuthorization> {
    authorize_actor_for_space(data_plane, context, space_id, false, None).await
}

pub async fn assert_actor_can_access_space(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
) -> MemoryServiceResult<()> {
    authorize_actor_for_space_access(data_plane, context, space_id)
        .await
        .map(|_| ())
}

pub async fn assert_actor_can_access_space_for_write(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
) -> MemoryServiceResult<()> {
    authorize_actor_for_space(
        data_plane,
        context,
        space_id,
        true,
        Some(CAPABILITY_MEMORY_WRITE),
    )
    .await
    .map(|_| ())
}

pub fn actor_may_read_sensitivity(
    context: &MemoryOpenApiRequestContext,
    sensitivity_level: &str,
    actual_actor_is_space_owner: bool,
) -> bool {
    match sensitivity_level {
        "public" | "internal" => true,
        "private" | "sensitive" => {
            if context.elevated_tenant_access {
                tracing::warn!(
                    tenant_id = context.tenant_id,
                    sensitivity_level,
                    actor_id = ?context.actor_id,
                    "elevated_tenant_access: backend operator accessing private/sensitive memory"
                );
                return true;
            }
            actual_actor_is_space_owner
        }
        "restricted" => actual_actor_is_space_owner,
        _ => actual_actor_is_space_owner,
    }
}

/// Enforces read-path sensitivity policy for a single memory record.
///
/// Returns `not_found` when the actor lacks read access so existence of
/// restricted records is not leaked to unauthorized callers.
pub async fn assert_actor_may_read_record_sensitivity(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
    sensitivity_level: &str,
) -> MemoryServiceResult<()> {
    let actual_owner = actual_actor_is_space_owner(data_plane, context, space_id).await?;
    assert_actor_may_read_record_sensitivity_for_owner(context, sensitivity_level, actual_owner)
}

pub fn assert_actor_may_read_record_sensitivity_for_owner(
    context: &MemoryOpenApiRequestContext,
    sensitivity_level: &str,
    actual_actor_is_space_owner: bool,
) -> MemoryServiceResult<()> {
    if actor_may_read_sensitivity(context, sensitivity_level, actual_actor_is_space_owner) {
        Ok(())
    } else {
        Err(MemoryServiceError::not_found("memory not found"))
    }
}

pub async fn actual_actor_is_space_owner(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
) -> MemoryServiceResult<bool> {
    let facts = resolve_governance_facts(data_plane, context, space_id, None).await?;
    let Some(space) = facts.space else {
        return Ok(false);
    };
    match space.lifecycle_status.as_str() {
        "active" | "archived" => {}
        "deleted" => return Ok(false),
        _ => {
            return Err(invalid_governance_state(
                "memory space lifecycle status is unsupported",
            ))
        }
    }
    let Some(actor_id) = context.actor_id.as_ref() else {
        return Ok(false);
    };
    Ok(
        matches!(space.owner_subject_type.as_str(), "user" | "agent")
            && space.owner_subject_id == actor_id.to_string(),
    )
}

pub fn sensitivity_read_scope(
    context: &MemoryOpenApiRequestContext,
    actual_actor_is_space_owner: bool,
) -> i32 {
    if actual_actor_is_space_owner {
        2
    } else if context.elevated_tenant_access {
        1
    } else {
        0
    }
}

pub async fn authorize_actor_for_spaces_access(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_ids: &[u64],
) -> MemoryServiceResult<Vec<MemorySpaceAuthorization>> {
    let unique_space_ids = unique_bounded_space_ids(space_ids)?;
    let mut authorizations = Vec::with_capacity(unique_space_ids.len());
    for space_id in unique_space_ids {
        authorizations.push(authorize_actor_for_space_access(data_plane, context, space_id).await?);
    }
    Ok(authorizations)
}

pub async fn authorize_actor_for_retrieval_spaces(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_ids: &[u64],
) -> MemoryServiceResult<Vec<MemorySpaceAuthorization>> {
    let unique_space_ids = unique_bounded_space_ids(space_ids)?;
    let mut authorizations = Vec::with_capacity(unique_space_ids.len());
    for space_id in unique_space_ids {
        authorizations
            .push(authorize_actor_for_space_retrieval(data_plane, context, space_id).await?);
    }
    Ok(authorizations)
}

fn unique_bounded_space_ids(space_ids: &[u64]) -> MemoryServiceResult<Vec<u64>> {
    let mut seen = BTreeSet::new();
    let unique_space_ids = space_ids
        .iter()
        .copied()
        .filter(|space_id| seen.insert(*space_id))
        .collect::<Vec<_>>();
    if unique_space_ids.len() > MAX_MEMORY_GOVERNANCE_FACTS as usize {
        return Err(MemoryServiceError::validation(format!(
            "at most {MAX_MEMORY_GOVERNANCE_FACTS} memory spaces may be authorized per request"
        )));
    }
    Ok(unique_space_ids)
}

pub fn require_list_space_id(space_id: Option<u64>) -> MemoryServiceResult<u64> {
    space_id.ok_or_else(|| MemoryServiceError::validation("spaceId query parameter is required"))
}

pub fn require_commercial_list_space_id(
    context: &MemoryOpenApiRequestContext,
    space_id: Option<u64>,
) -> MemoryServiceResult<Option<u64>> {
    if context.elevated_tenant_access {
        Ok(space_id)
    } else {
        Ok(Some(require_list_space_id(space_id)?))
    }
}

pub async fn assert_actor_may_read_entity_sensitivity(
    context: &MemoryOpenApiRequestContext,
    sensitivity_level: &str,
    actual_actor_is_space_owner: bool,
) -> MemoryServiceResult<()> {
    if actor_may_read_sensitivity(context, sensitivity_level, actual_actor_is_space_owner) {
        Ok(())
    } else {
        Err(MemoryServiceError::not_found("entity not found"))
    }
}

pub async fn assert_actor_can_access_space_i64(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: i64,
) -> MemoryServiceResult<()> {
    if space_id < 0 {
        return Err(MemoryServiceError::validation(
            "spaceId must be non-negative",
        ));
    }
    assert_actor_can_access_space(
        data_plane,
        context,
        u64::try_from(space_id).map_err(|_| {
            MemoryServiceError::validation("spaceId must fit in an unsigned 64-bit integer")
        })?,
    )
    .await
}

pub async fn assert_actor_can_access_space_for_write_i64(
    data_plane: &MemoryRuntimeDataPlane,
    context: &MemoryOpenApiRequestContext,
    space_id: i64,
) -> MemoryServiceResult<()> {
    if space_id < 0 {
        return Err(MemoryServiceError::validation(
            "spaceId must be non-negative",
        ));
    }
    assert_actor_can_access_space_for_write(
        data_plane,
        context,
        u64::try_from(space_id).map_err(|_| {
            MemoryServiceError::validation("spaceId must fit in an unsigned 64-bit integer")
        })?,
    )
    .await
}

pub fn validate_user_space_owner(
    context: &MemoryOpenApiRequestContext,
    owner_subject_type: &str,
    owner_subject_id: &str,
) -> MemoryServiceResult<()> {
    if context.elevated_tenant_access {
        tracing::warn!(
            tenant_id = context.tenant_id,
            owner_subject_type,
            "elevated_tenant_access bypass: space owner validation skipped"
        );
        return Ok(());
    }
    if owner_subject_type != "user" {
        return forbidden("only backend operators may create non-user-owned memory spaces");
    }
    let Some(actor_id) = context.actor_id else {
        return forbidden("authenticated actor is required to create a user-owned memory space");
    };
    if owner_subject_id != actor_id.to_string() {
        return forbidden(
            "ownerSubjectId must match the authenticated actor for user-owned spaces",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use sdkwork_memory_plugin_native_sql::{NativeSqlCreateSpaceCommand, NativeSqlMemoryStore};
    use sdkwork_memory_spi::{
        MemoryCoreRuntime, MemoryDeploymentMode, MemoryExecutablePluginRuntime,
        MemoryGovernanceAccessPort, MemoryImplementationKind, MemoryPluginPorts,
        MemoryRuntimeProfileMetadata, MemorySpaceGovernanceFact, ResolveMemorySpaceGovernanceQuery,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn test_data_plane(store: &NativeSqlMemoryStore) -> MemoryRuntimeDataPlane {
        crate::OpenMemoryService::new(store.clone())
            .runtime_data_plane()
            .clone()
    }

    async fn seed_user_space(
        store: &NativeSqlMemoryStore,
        tenant_id: i64,
        space_id: i64,
        owner_id: &str,
    ) {
        store
            .create_space_record(
                tenant_id,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: owner_id.to_string(),
                    space_type: "personal".to_string(),
                    display_name: "test space".to_string(),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .unwrap();
    }

    #[derive(Debug)]
    struct CountingGovernance {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl MemoryGovernanceAccessPort for CountingGovernance {
        fn supports_bounded_governance_access(&self) -> bool {
            true
        }

        async fn resolve_space_governance(
            &self,
            query: ResolveMemorySpaceGovernanceQuery,
        ) -> sdkwork_memory_spi::MemorySpiResult<MemorySpaceGovernanceFacts> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(MemorySpaceGovernanceFacts {
                space: (query.scope.space_id != 404).then_some(MemorySpaceGovernanceFact {
                    space_id: query.scope.space_id,
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: "2001".to_string(),
                    lifecycle_status: "active".to_string(),
                }),
                actor_bindings: Vec::new(),
                capability_bindings: Vec::new(),
                complete: true,
            })
        }
    }

    fn counting_data_plane(governance: Arc<CountingGovernance>) -> MemoryRuntimeDataPlane {
        let executable = MemoryExecutablePluginRuntime::new(
            MemoryPluginPorts::new().with_governance_access(governance),
        );
        let mut runtime = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
            profile_id: "access-single-snapshot-contract".to_string(),
            implementation_kind: MemoryImplementationKind::HybridPlatform,
            primary_plugin_id: "counting-governance".to_string(),
            deployment_mode: MemoryDeploymentMode::EvalOnly,
        });
        runtime
            .bind_port(
                "counting-governance",
                "MemoryGovernanceAccessPort",
                &executable,
            )
            .expect("counting governance port must bind");
        MemoryRuntimeDataPlane::from_core_runtime(runtime)
    }

    #[tokio::test]
    async fn retrieval_authorization_uses_one_bounded_snapshot() {
        let calls = Arc::new(AtomicUsize::new(0));
        let data_plane = counting_data_plane(Arc::new(CountingGovernance {
            calls: calls.clone(),
        }));
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));

        let authorization = authorize_actor_for_space_retrieval(&data_plane, &context, 7)
            .await
            .expect("owner retrieval authorization should succeed");

        assert_eq!(authorization.space_id, 7);
        assert!(authorization.actor_is_space_owner);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retrieval_authorization_deduplicates_spaces_without_reordering() {
        let calls = Arc::new(AtomicUsize::new(0));
        let data_plane = counting_data_plane(Arc::new(CountingGovernance {
            calls: calls.clone(),
        }));
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));

        let authorizations =
            authorize_actor_for_retrieval_spaces(&data_plane, &context, &[2, 1, 2])
                .await
                .expect("bounded retrieval authorization should succeed");

        assert_eq!(
            authorizations
                .iter()
                .map(|authorization| authorization.space_id)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn elevated_retrieval_authorization_cannot_bypass_missing_space() {
        let calls = Arc::new(AtomicUsize::new(0));
        let data_plane = counting_data_plane(Arc::new(CountingGovernance {
            calls: calls.clone(),
        }));
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));

        let error = authorize_actor_for_space_retrieval(&data_plane, &context, 404)
            .await
            .expect_err("elevated access must still require an existing space");

        assert_eq!(error.code, "not_found");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn write_to_missing_space_is_denied() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        let error = assert_actor_can_access_space_for_write(&data_plane, &context, 99)
            .await
            .expect_err("missing space write must be forbidden");
        assert_eq!(error.code, "not_found");
    }

    #[tokio::test]
    async fn actor_can_access_owned_user_space() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 1, "2001").await;
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert_actor_can_access_space(&data_plane, &context, 1)
            .await
            .expect("owned space should be accessible");
    }

    #[tokio::test]
    async fn actor_cannot_access_foreign_user_space() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 2, "3002").await;
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        let error = assert_actor_can_access_space(&data_plane, &context, 2)
            .await
            .expect_err("foreign space must be forbidden");
        assert_eq!(error.code, "forbidden");
    }

    #[tokio::test]
    async fn missing_actor_is_denied_for_user_space() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 1, "2001").await;
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, None);
        let error = assert_actor_can_access_space(&data_plane, &context, 1)
            .await
            .expect_err("missing actor must fail closed");
        assert_eq!(error.code, "forbidden");
    }

    #[tokio::test]
    async fn backend_operator_can_access_tenant_spaces() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 2, "3002").await;
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));
        assert_actor_can_access_space(&data_plane, &context, 2)
            .await
            .expect("backend operator should have tenant-wide access");
    }

    #[tokio::test]
    async fn actor_can_access_space_via_active_binding() {
        use sdkwork_memory_plugin_native_sql::InsertSubjectCommand;

        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 2, "3002").await;
        store
            .insert_subject(InsertSubjectCommand {
                id: 501,
                uuid: "501",
                tenant_id: 100_001,
                organization_id: None,
                subject_type: "user",
                subject_ref: "2001",
                display_name: "bound actor",
                default_space_id: None,
                metadata_json: None,
            })
            .await
            .unwrap();
        store
            .insert_binding(
                601,
                "601",
                100_001,
                None,
                "access",
                "viewer",
                Some(501),
                None,
                Some(2),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert_actor_can_access_space(&data_plane, &context, 2)
            .await
            .expect("binding grant should allow read access");
    }

    #[tokio::test]
    async fn viewer_binding_allows_read_but_denies_write() {
        use sdkwork_memory_plugin_native_sql::InsertSubjectCommand;

        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 2, "3002").await;
        store
            .insert_subject(InsertSubjectCommand {
                id: 502,
                uuid: "502",
                tenant_id: 100_001,
                organization_id: None,
                subject_type: "user",
                subject_ref: "2001",
                display_name: "viewer actor",
                default_space_id: None,
                metadata_json: None,
            })
            .await
            .unwrap();
        store
            .insert_binding(
                602,
                "602",
                100_001,
                None,
                "access",
                "viewer",
                Some(502),
                None,
                Some(2),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert_actor_can_access_space(&data_plane, &context, 2)
            .await
            .expect("viewer binding should allow read");
        let error = assert_actor_can_access_space_for_write(&data_plane, &context, 2)
            .await
            .expect_err("viewer binding must not allow write");
        assert_eq!(error.code, "forbidden");
    }

    #[tokio::test]
    async fn direct_owner_can_write_and_learner_binding_can_write() {
        use sdkwork_memory_plugin_native_sql::InsertSubjectCommand;

        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 3, "2001").await;
        seed_user_space(&store, 100_001, 4, "3003").await;
        store
            .insert_subject(InsertSubjectCommand {
                id: 503,
                uuid: "503",
                tenant_id: 100_001,
                organization_id: None,
                subject_type: "user",
                subject_ref: "2001",
                display_name: "learner actor",
                default_space_id: None,
                metadata_json: None,
            })
            .await
            .unwrap();
        store
            .insert_binding(
                603,
                "603",
                100_001,
                None,
                "share",
                "learner",
                Some(503),
                None,
                Some(4),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let owner_context =
            MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert_actor_can_access_space_for_write(&data_plane, &owner_context, 3)
            .await
            .expect("direct owner should be able to write");
        assert_actor_can_access_space_for_write(&data_plane, &owner_context, 4)
            .await
            .expect("learner binding should be able to write");
    }

    #[tokio::test]
    async fn elevated_operator_cannot_write_missing_space() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));
        let error = assert_actor_can_access_space_for_write(&data_plane, &context, 404)
            .await
            .expect_err("elevated missing-space writes must fail closed");
        assert_eq!(error.code, "not_found");
    }

    #[tokio::test]
    async fn capability_deny_is_validity_aware_and_fail_closed_for_malformed_state() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 5, "2001").await;
        store
            .insert_capability_binding(
                701,
                "cap-future",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                5,
                "deny",
                10,
                Some("2999-01-01T00:00:00.000Z"),
                None,
                None,
            )
            .await
            .unwrap();
        store
            .insert_capability_binding(
                704,
                "cap-expired",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                5,
                "deny",
                5,
                None,
                Some("2000-01-01T00:00:00.000Z"),
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert_actor_can_access_space_for_write(&data_plane, &context, 5)
            .await
            .expect("future deny must not be active");

        store
            .insert_capability_binding(
                702,
                "cap-invalid-time",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                5,
                "deny",
                20,
                Some("not-a-timestamp"),
                None,
                None,
            )
            .await
            .unwrap();
        let error = assert_actor_can_access_space_for_write(&data_plane, &context, 5)
            .await
            .expect_err("malformed governance timestamps must fail closed");
        assert_eq!(error.code, "storage_error");
    }

    #[tokio::test]
    async fn active_capability_deny_blocks_write() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 6, "2001").await;
        store
            .insert_capability_binding(
                705,
                "cap-higher-priority-allow",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                6,
                "allow",
                100,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        store
            .insert_capability_binding(
                703,
                "cap-active-deny",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                6,
                "deny",
                10,
                Some("2000-01-01T00:00:00.000Z"),
                Some("2999-01-01T00:00:00.000Z"),
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        let error = assert_actor_can_access_space_for_write(&data_plane, &context, 6)
            .await
            .expect_err("active deny must block write");
        assert_eq!(error.code, "forbidden");
    }

    #[tokio::test]
    async fn unknown_binding_role_and_capability_mode_fail_closed() {
        use sdkwork_memory_plugin_native_sql::InsertSubjectCommand;

        let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
        seed_user_space(&store, 100_001, 7, "3003").await;
        store
            .insert_subject(InsertSubjectCommand {
                id: 507,
                uuid: "507",
                tenant_id: 100_001,
                organization_id: None,
                subject_type: "user",
                subject_ref: "2001",
                display_name: "invalid governance actor",
                default_space_id: None,
                metadata_json: None,
            })
            .await
            .unwrap();
        store
            .insert_binding(
                607,
                "607",
                100_001,
                None,
                "access",
                "unknown-role",
                Some(507),
                None,
                Some(7),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let data_plane = test_data_plane(&store);
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        let binding_error = assert_actor_can_access_space(&data_plane, &context, 7)
            .await
            .expect_err("unknown binding roles must fail closed");
        assert_eq!(binding_error.code, "storage_error");

        seed_user_space(&store, 100_001, 8, "2001").await;
        store
            .insert_capability_binding(
                708,
                "cap-unknown-mode",
                100_001,
                CAPABILITY_MEMORY_WRITE,
                "space",
                8,
                "unknown-mode",
                1,
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let capability_error = assert_actor_can_access_space_for_write(&data_plane, &context, 8)
            .await
            .expect_err("unknown capability modes must fail closed");
        assert_eq!(capability_error.code, "storage_error");
    }

    #[test]
    fn actor_may_read_public_and_internal_without_ownership() {
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert!(actor_may_read_sensitivity(&context, "public", false));
        assert!(actor_may_read_sensitivity(&context, "internal", false));
    }

    #[test]
    fn actor_may_not_read_restricted_sensitivity_without_ownership() {
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert!(!actor_may_read_sensitivity(&context, "private", false));
        assert!(!actor_may_read_sensitivity(&context, "sensitive", false));
        assert!(!actor_may_read_sensitivity(&context, "restricted", false));
    }

    #[test]
    fn space_owner_may_read_restricted_sensitivity() {
        let context = MemoryOpenApiRequestContext::for_open_surface("key-1", 100_001, Some(2001));
        assert!(actor_may_read_sensitivity(&context, "restricted", true));
    }

    #[test]
    fn elevated_backend_can_read_private_and_sensitive() {
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));
        assert!(actor_may_read_sensitivity(&context, "private", false));
        assert!(actor_may_read_sensitivity(&context, "sensitive", false));
    }

    #[test]
    fn elevated_backend_cannot_read_restricted_without_ownership() {
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));
        assert!(!actor_may_read_sensitivity(&context, "restricted", false));
    }

    #[test]
    fn elevated_backend_can_read_restricted_as_space_owner() {
        let context = MemoryOpenApiRequestContext::for_backend_surface(100_001, Some(9001));
        assert!(actor_may_read_sensitivity(&context, "restricted", true));
    }
}
