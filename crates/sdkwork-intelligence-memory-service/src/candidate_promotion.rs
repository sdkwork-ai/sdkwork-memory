use sdkwork_memory_contract::{MemoryCandidate, MemoryServiceError, MemoryServiceResult};
use sdkwork_memory_spi::{
    MemoryCandidateEvidenceLink, MemoryMutationJournal, MemoryScopeContext,
    PromoteMemoryCandidateAtomicCommand, PromoteMemoryCandidateAtomicWithJournalCommand,
    RetrieveMemoryCandidateDetailQuery,
};
use sdkwork_utils_rust::is_blank;
use serde_json::Value;

use crate::open_api::OpenMemoryService;

pub(crate) fn parse_evidence_event_ids(evidence_json: Option<&str>) -> Vec<String> {
    let Some(raw) = evidence_json.filter(|value| !is_blank(Some(value))) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };

    if let Some(items) = value.as_array() {
        return items
            .iter()
            .filter_map(|item| {
                item.as_str().and_then(|entry| {
                    entry
                        .strip_prefix("event:")
                        .map(|event_id| event_id.to_string())
                })
            })
            .collect();
    }

    if let Some(event_id) = value
        .get("eventId")
        .or_else(|| value.get("event_id"))
        .and_then(|item| item.as_str())
    {
        return vec![event_id.to_string()];
    }

    Vec::new()
}

impl OpenMemoryService {
    pub(crate) async fn approve_candidate_with_promotion(
        &self,
        tenant_id: i64,
        scope: MemoryScopeContext,
        candidate_id: u64,
        decided_by: Option<u64>,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let detail = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;
        if detail.space_id != scope.space_id {
            return Err(MemoryServiceError::forbidden(
                "candidate does not belong to the requested memory space",
            ));
        }

        if detail.decision_state == "approved" {
            if detail.target_memory_id.is_none() {
                return Err(MemoryServiceError::storage(
                    "approved candidate is missing its promoted memory",
                ));
            }
            return Self::map_candidate_api_detail(detail);
        }

        let requested_memory_id = match detail.target_memory_id.clone() {
            Some(memory_id) => memory_id,
            None => self.next_id()?.to_string(),
        };
        let event_ids = parse_evidence_event_ids(detail.evidence_json.as_deref());
        let mut evidence_links = Vec::with_capacity(event_ids.len());
        for event_id in event_ids {
            evidence_links.push(MemoryCandidateEvidenceLink {
                source_id: self.next_id()?.to_string(),
                event_id,
                confidence_delta: Some(detail.confidence),
            });
        }
        let journal_memory_id = requested_memory_id.clone();
        let journal = MemoryMutationJournal {
            outbox_id: self.next_id()?.to_string(),
            aggregate_type: "memory_record".to_string(),
            aggregate_id: journal_memory_id.clone(),
            event_type: "memory.candidate.promoted".to_string(),
            event_version: "1.0".to_string(),
            payload_json: serde_json::json!({
                "candidateId": candidate_id,
                "memoryId": journal_memory_id,
            })
            .to_string(),
            audit_id: self.next_id()?.to_string(),
            audit_action: "memory.candidate.promoted".to_string(),
            audit_resource_type: "memory_record".to_string(),
            audit_resource_id: requested_memory_id.clone(),
            audit_result: "accepted".to_string(),
        };
        let quota_limits =
            crate::tenant_quota::resolve_quota_limits(&self.store, tenant_id).await?;
        let admission = self
            .runtime_data_plane
            .promote_candidate_atomic_with_quota_and_journal(
                PromoteMemoryCandidateAtomicWithJournalCommand {
                    promotion: PromoteMemoryCandidateAtomicCommand {
                        scope: scope.clone(),
                        candidate_id: candidate_id.to_string(),
                        memory_id: requested_memory_id,
                        memory_type: detail.memory_type.clone(),
                        proposed_text: detail.proposed_text.clone(),
                        evidence_links,
                        decided_by: decided_by.map(|value| value as i64),
                    },
                    journal,
                },
                quota_limits.max_records_per_space,
            )
            .await?;
        let promotion =
            crate::tenant_quota::resolve_space_record_quota_admission(&scope, admission)?;
        let memory_uuid = promotion.memory_id;

        let refreshed = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;

        if refreshed.target_memory_id.as_deref() != Some(memory_uuid.as_str()) {
            return Err(MemoryServiceError::storage(
                "approved candidate did not retain promoted memory",
            ));
        }

        Self::map_candidate_api_detail(refreshed)
    }
}
