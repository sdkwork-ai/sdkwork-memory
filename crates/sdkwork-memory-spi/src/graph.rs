//! Entity/edge graph port.
//!
//! The graph capability (entities, provenance edges, and their journals) is
//! expressed here as a port so consumers depend on the boundary, not on a
//! specific store plugin. Commands are uuid-based on purpose: internal row ids
//! never cross the port, and each implementation resolves uuids against its own
//! storage. Journal semantics match the canonical-record ports — every mutation
//! carries its outbox/audit pair and must be applied in the same transaction.

use crate::error::MemorySpiResult;
use crate::ports::{MemoryMutationJournal, MemoryScopeContext, MemorySensitivityReadScope};

#[async_trait::async_trait]
pub trait MemoryGraphPort: Send + Sync {
    /// Insert one entity. The implementation assigns internal ids; the caller
    /// supplies the uuid and journal.
    async fn create_entity(&self, command: CreateGraphEntityCommand) -> MemorySpiResult<()> {
        let _ = command;
        Err(graph_port_unsupported("create_entity"))
    }

    /// Patch an entity; `None` fields keep their stored values. Returns whether
    /// a live entity was updated.
    async fn update_entity(&self, command: UpdateGraphEntityCommand) -> MemorySpiResult<bool> {
        let _ = command;
        Err(graph_port_unsupported("update_entity"))
    }

    async fn retrieve_entity(
        &self,
        query: RetrieveGraphEntityQuery,
    ) -> MemorySpiResult<Option<GraphEntityRecord>> {
        let _ = query;
        Err(graph_port_unsupported("retrieve_entity"))
    }

    async fn list_entities(
        &self,
        query: ListGraphEntitiesQuery,
    ) -> MemorySpiResult<Vec<GraphEntityRecord>> {
        let _ = query;
        Err(graph_port_unsupported("list_entities"))
    }

    async fn count_entities_for_tenant(&self, tenant_id: i64) -> MemorySpiResult<i64> {
        let _ = tenant_id;
        Err(graph_port_unsupported("count_entities_for_tenant"))
    }

    /// Insert one edge. `source_memory_id` names the evidencing memory by uuid;
    /// an unknown uuid must fail rather than store a dangling provenance link.
    async fn create_edge(&self, command: CreateGraphEdgeCommand) -> MemorySpiResult<()> {
        let _ = command;
        Err(graph_port_unsupported("create_edge"))
    }

    /// Patch an edge; `None` fields keep their stored values. Returns whether a
    /// live edge was updated.
    async fn update_edge(&self, command: UpdateGraphEdgeCommand) -> MemorySpiResult<bool> {
        let _ = command;
        Err(graph_port_unsupported("update_edge"))
    }

    async fn delete_edge(&self, command: DeleteGraphEdgeCommand) -> MemorySpiResult<()> {
        let _ = command;
        Err(graph_port_unsupported("delete_edge"))
    }

    async fn retrieve_edge(
        &self,
        query: RetrieveGraphEdgeQuery,
    ) -> MemorySpiResult<Option<GraphEdgeRecord>> {
        let _ = query;
        Err(graph_port_unsupported("retrieve_edge"))
    }

    async fn list_edges(
        &self,
        query: ListGraphEdgesQuery,
    ) -> MemorySpiResult<Vec<GraphEdgeRecord>> {
        let _ = query;
        Err(graph_port_unsupported("list_edges"))
    }

    async fn count_edges_for_tenant(&self, tenant_id: i64) -> MemorySpiResult<i64> {
        let _ = tenant_id;
        Err(graph_port_unsupported("count_edges_for_tenant"))
    }

    /// Entity-to-memory provenance pairs for one space, flattened per edge
    /// endpoint. This feeds the retrieval `entity` ranking signal.
    async fn entity_memory_links(
        &self,
        scope: MemoryScopeContext,
    ) -> MemorySpiResult<Vec<EntityMemoryLink>> {
        let _ = scope;
        Err(graph_port_unsupported("entity_memory_links"))
    }
}

fn graph_port_unsupported(operation: &str) -> crate::error::MemorySpiError {
    crate::error::MemorySpiError::PortOperationFailed {
        port: "MemoryGraphPort".to_string(),
        message: format!("graph operation {operation} is not supported by this store"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphEntityRecord {
    pub tenant_id: i64,
    pub space_id: i64,
    pub entity_id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases_json: Option<String>,
    pub attributes_json: Option<String>,
    pub sensitivity_level: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdgeRecord {
    pub tenant_id: i64,
    pub space_id: i64,
    pub edge_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    /// Uuid of the memory evidencing this edge, if any.
    pub source_memory_id: Option<String>,
    pub weight: Option<f64>,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

/// One entity-to-memory provenance pair for retrieval ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityMemoryLink {
    pub entity_name: String,
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateGraphEntityCommand {
    pub scope: MemoryScopeContext,
    pub entity_id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases_json: Option<String>,
    pub attributes_json: Option<String>,
    pub sensitivity_level: String,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateGraphEntityCommand {
    pub scope: MemoryScopeContext,
    pub entity_id: String,
    pub canonical_name: Option<String>,
    pub aliases_json: Option<String>,
    pub attributes_json: Option<String>,
    pub sensitivity_level: Option<String>,
    pub status: Option<String>,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateGraphEdgeCommand {
    pub scope: MemoryScopeContext,
    pub edge_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    pub source_memory_id: Option<String>,
    pub weight: Option<f64>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateGraphEdgeCommand {
    pub scope: MemoryScopeContext,
    pub edge_id: String,
    pub relation_type: Option<String>,
    pub weight: Option<f64>,
    pub status: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteGraphEdgeCommand {
    pub scope: MemoryScopeContext,
    pub edge_id: String,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveGraphEntityQuery {
    pub tenant_id: i64,
    pub entity_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveGraphEdgeQuery {
    pub tenant_id: i64,
    pub edge_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListGraphEntitiesQuery {
    pub tenant_id: i64,
    pub space_id: Option<i64>,
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub page_size: i32,
    pub cursor: Option<String>,
    pub sensitivity_read_scope: MemorySensitivityReadScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListGraphEdgesQuery {
    pub tenant_id: i64,
    pub space_id: Option<i64>,
    pub relation_type: Option<String>,
    pub source_entity_id: Option<String>,
    pub page_size: i32,
    pub cursor: Option<String>,
    pub sensitivity_read_scope: MemorySensitivityReadScope,
}
