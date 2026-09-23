//! Commercial memory management contract types.
//!
//! These types model the commercial layer that binds memory spaces to
//! subjects (tenants, organizations, users, applications), assigns
//! capabilities, and enforces policies for retrieval and access control.

use crate::serde_int64::{
    deserialize_option_u64_from_string_or_number, deserialize_u64_from_string_or_number,
    serialize_option_u64_as_string, serialize_u64_as_string,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Subject
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    Tenant,
    Organization,
    User,
    Application,
    Service,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySubject {
    pub subject_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub organization_id: Option<u64>,
    pub subject_type: SubjectType,
    pub subject_ref: String,
    pub display_name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub default_space_id: Option<u64>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSubjectCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub organization_id: Option<u64>,
    pub subject_type: SubjectType,
    pub subject_ref: String,
    pub display_name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub default_space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSubjectCommand {
    pub display_name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub default_space_id: Option<u64>,
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListSubjectsQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_type: Option<SubjectType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySubjectList {
    pub items: Vec<MemorySubject>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Memory Binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    Ownership,
    Access,
    Share,
    Reference,
    Provision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBinding {
    pub binding_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub space_id: Option<u64>,
    pub binding_kind: BindingKind,
    pub binding_role: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub source_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_codes: Option<Vec<String>>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateBindingCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub space_id: Option<u64>,
    pub binding_kind: BindingKind,
    pub binding_role: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub source_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_codes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListBindingsQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub source_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_subject_id: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_kind: Option<BindingKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBindingList {
    pub items: Vec<MemoryBinding>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Capability Binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMode {
    Allow,
    Deny,
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTargetType {
    Subject,
    Space,
    Binding,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCapabilityBinding {
    pub capability_binding_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub capability_code: String,
    pub target_type: CapabilityTargetType,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub target_id: u64,
    pub mode: CapabilityMode,
    pub priority: i32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCapabilityBindingCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub capability_code: String,
    pub target_type: CapabilityTargetType,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub target_id: u64,
    pub mode: CapabilityMode,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListCapabilityBindingsQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<CapabilityTargetType>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCapabilityBindingList {
    pub items: Vec<MemoryCapabilityBinding>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Resolved capability (result of binding resolution)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCapability {
    pub capability_code: String,
    pub mode: CapabilityMode,
    pub priority: i32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResolvedCapabilityList {
    pub items: Vec<ResolvedCapability>,
    pub page_info: crate::dto::PageInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveCapabilitiesQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub target_type: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub target_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntity {
    pub entity_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub space_id: u64,
    pub entity_type: String,
    pub canonical_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    pub sensitivity_level: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateEntityCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub space_id: u64,
    pub entity_type: String,
    pub canonical_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    #[serde(default = "default_internal_sensitivity")]
    pub sensitivity_level: String,
}

fn default_internal_sensitivity() -> String {
    "internal".to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateEntityCommand {
    pub canonical_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Value>,
    pub sensitivity_level: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListEntitiesQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntityList {
    pub items: Vec<MemoryEntity>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEdge {
    pub edge_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub space_id: u64,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    /// Canonical memory evidencing this edge (the edge's provenance link).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateEdgeCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub space_id: u64,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    /// Canonical memory evidencing this edge; persisted so deleting that
    /// memory clears exactly the edges it supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateEdgeCommand {
    pub relation_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListEdgesQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub space_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEdgeList {
    pub items: Vec<MemoryEdge>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicy {
    pub policy_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub policy_type: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,
    pub status: String,
    pub policy: Value,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePolicyCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub policy_type: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,
    pub policy: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePolicyCommand {
    pub policy_type: Option<String>,
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListPoliciesQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyList {
    pub items: Vec<MemoryPolicy>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Policy assignment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAssignmentTargetType {
    Subject,
    Space,
    Entity,
    Binding,
    CapabilityBinding,
    ImplementationProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyInheritanceMode {
    Inherit,
    Override,
    Deny,
    Shadow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyAssignment {
    pub policy_assignment_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub policy_id: String,
    pub target_type: PolicyAssignmentTargetType,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub target_id: u64,
    pub priority: i32,
    pub inheritance_mode: PolicyInheritanceMode,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePolicyAssignmentCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    pub policy_id: String,
    pub target_type: PolicyAssignmentTargetType,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub target_id: u64,
    #[serde(default)]
    pub priority: i32,
    pub inheritance_mode: PolicyInheritanceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePolicyAssignmentCommand {
    pub priority: Option<i32>,
    pub inheritance_mode: Option<PolicyInheritanceMode>,
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListPolicyAssignmentsQuery {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<PolicyAssignmentTargetType>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub target_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyAssignmentList {
    pub items: Vec<MemoryPolicyAssignment>,
    pub page_info: crate::dto::PageInfo,
}

// ---------------------------------------------------------------------------
// Commercial readiness
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCommercialReadiness {
    pub readiness_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub implementation_profile_id: Option<u64>,
    pub score: f64,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_conformance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observability_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_coverage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_findings: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning_findings: Option<Vec<String>>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RebuildCommercialReadinessCommand {
    #[serde(
        skip_deserializing,
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string_or_number"
    )]
    pub tenant_id: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_option_u64_as_string",
        deserialize_with = "deserialize_option_u64_from_string_or_number"
    )]
    pub implementation_profile_id: Option<u64>,
}
