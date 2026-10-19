use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCommercialReadiness {
    #[serde(rename = "readinessId")]
    pub readiness_id: String,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

    #[serde(rename = "implementationProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_profile_id: Option<String>,

    pub score: f64,

    pub state: String,

    #[serde(rename = "contractCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "managementCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "runtimeConformance")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_conformance: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "privacyCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "auditCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "sdkCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "evaluationCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "observabilityCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observability_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "migrationCoverage")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_coverage: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "blockingFindings")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking_findings: Option<Vec<String>>,

    #[serde(rename = "warningFindings")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning_findings: Option<Vec<String>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
