pub mod admin_dto;
pub mod app_ports;
pub mod backend_ports;
pub mod commercial;
pub mod dto;
pub mod ports;
pub mod runtime_env;
mod serde_int64;
pub mod space;

pub use admin_dto::{
    ListAdminResourcesQuery, MemoryAuditLog, MemoryAuditLogList, MemoryEvalRun, MemoryEvalRunList,
    MemoryEvalRunRequest, MemoryImplementationProfile, MemoryImplementationProfileList,
    MemoryImplementationProfileRequest, MemoryIndex, MemoryIndexList, MemoryIndexRequest,
    MemoryMigrationJobRequest, MemoryProviderBindingList, MemoryProviderBindingRequest,
    MemoryRetentionJobRequest, MemoryRetrievalProfile, MemoryRetrievalProfileList,
    MemoryRetrievalProfileRequest,
};
pub use app_ports::{MemoryAppApi, MemoryAppRequestContext};
pub use backend_ports::{MemoryBackendApi, MemoryBackendRequestContext};
pub use commercial::*;
pub use dto::{PageInfo, *};
pub use ports::{
    MemoryOpenApi, MemoryOpenApiRequestContext, MemoryServiceError, MemoryServiceErrorKind,
    MemoryServiceResult, STORAGE_ERROR_DETAIL,
};
pub use runtime_env::{
    env_test_lock, memory_dev_auth_bypass_enabled, memory_environment_name,
    require_explicit_memory_environment,
    memory_is_production_like_environment, memory_use_dev_inline_auth_resolver, MemoryEnvScope,
};
pub use space::{ListSpacesQuery, MemorySpace, MemorySpaceList, MemorySpaceRequest};

#[cfg(test)]
mod query_wire_tests {
    use super::{
        CreateEntityCommand, ListEntitiesQuery, ListMemoriesQuery, ListSpacesQuery,
        ResolveCapabilitiesQuery,
    };
    use serde_json::json;

    #[test]
    fn query_contracts_accept_only_canonical_snake_case_names() {
        let spaces: ListSpacesQuery =
            serde_json::from_value(json!({ "page_size": 20 })).expect("canonical page_size query");
        assert_eq!(spaces.page_size, Some(20));

        let memories: ListMemoriesQuery = serde_json::from_value(json!({
            "space_id": 7,
            "page_size": 50
        }))
        .expect("canonical memory query");
        assert_eq!(memories.space_id, Some(7));
        assert_eq!(memories.page_size, Some(50));

        let entities: ListEntitiesQuery = serde_json::from_value(json!({
            "space_id": 7,
            "page_size": 50
        }))
        .expect("canonical commercial query");
        assert_eq!(entities.space_id, Some(7));

        for alias in [
            json!({ "pageSize": 20 }),
            json!({ "limit": 20 }),
            json!({ "size": 20 }),
        ] {
            assert!(serde_json::from_value::<ListSpacesQuery>(alias).is_err());
        }
        assert!(serde_json::from_value::<ListMemoriesQuery>(json!({ "spaceId": 7 })).is_err());
        assert!(serde_json::from_value::<ListEntitiesQuery>(json!({ "tenantId": 1 })).is_err());
    }

    #[test]
    fn capability_resolution_post_body_remains_canonical_camel_case() {
        let body: ResolveCapabilitiesQuery = serde_json::from_value(json!({
            "targetType": "space",
            "targetId": "7"
        }))
        .expect("capability resolution is a JSON body, not a URL query");
        assert_eq!(body.tenant_id, 0);
        assert_eq!(body.target_type, "space");
        assert_eq!(body.target_id, 7);
        assert_eq!(body.page_size, None);
        assert!(serde_json::from_value::<ResolveCapabilitiesQuery>(json!({
            "tenantId": "100001",
            "targetType": "space",
            "targetId": "7"
        }))
        .is_err());
        assert!(serde_json::from_value::<ResolveCapabilitiesQuery>(json!({
            "tenant_id": "100001",
            "target_type": "space",
            "target_id": "7"
        }))
        .is_err());
    }

    #[test]
    fn commercial_request_bodies_reject_client_supplied_tenant_identity() {
        let request: CreateEntityCommand = serde_json::from_value(json!({
            "spaceId": "7",
            "entityType": "person",
            "canonicalName": "Alice"
        }))
        .expect("canonical entity request");
        assert_eq!(request.tenant_id, 0);
        assert!(serde_json::from_value::<CreateEntityCommand>(json!({
            "tenantId": "999999",
            "spaceId": "7",
            "entityType": "person",
            "canonicalName": "Alice"
        }))
        .is_err());
    }
}
