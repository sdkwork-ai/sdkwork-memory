use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceErrorKind, STORAGE_ERROR_DETAIL};
use sdkwork_memory_plugin_native_sql::NativeSqlStoreError;
use sdkwork_memory_spi::MemorySpiError;

pub fn map_memory_spi_error(error: MemorySpiError) -> MemoryServiceError {
    if let MemorySpiError::IdempotencyConflict { .. } = error {
        return MemoryServiceError::conflict(
            "resource already exists with a different idempotent payload",
        );
    }

    tracing::error!(error = %error, "memory runtime port operation failed");
    MemoryServiceError {
        kind: MemoryServiceErrorKind::Storage,
        code: "storage_error".to_string(),
        detail: STORAGE_ERROR_DETAIL.to_string(),
    }
}

pub fn map_native_sql_store_error(error: NativeSqlStoreError) -> MemoryServiceError {
    if let NativeSqlStoreError::EventConflict { .. } = error {
        return MemoryServiceError::conflict(
            "event already exists with different payload for the same idempotency key",
        );
    }
    if let NativeSqlStoreError::OutboxConflict { .. } = error {
        return MemoryServiceError::conflict(
            "outbox event already exists with different payload for the same idempotency key",
        );
    }
    if let NativeSqlStoreError::Database(ref db_err) = error {
        if db_err
            .as_database_error()
            .is_some_and(|db| db.is_unique_violation())
        {
            return MemoryServiceError::conflict(
                "resource already exists for the requested unique key",
            );
        }
    }
    tracing::error!(error = %error, "memory store operation failed");
    MemoryServiceError {
        kind: MemoryServiceErrorKind::Storage,
        code: "storage_error".to_string(),
        detail: STORAGE_ERROR_DETAIL.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_memory_contract::MemoryServiceErrorKind;

    #[test]
    fn maps_event_conflict_to_http_conflict() {
        let mapped = map_native_sql_store_error(NativeSqlStoreError::EventConflict {
            tenant_id: 100_001,
            event_id: "evt-1".to_string(),
        });
        assert_eq!(mapped.kind, MemoryServiceErrorKind::Conflict);
    }

    #[test]
    fn maps_outbox_conflict_to_http_conflict() {
        let mapped = map_native_sql_store_error(NativeSqlStoreError::OutboxConflict {
            tenant_id: 100_001,
            outbox_id: "ob-1".to_string(),
        });
        assert_eq!(mapped.kind, MemoryServiceErrorKind::Conflict);
    }

    #[test]
    fn maps_spi_idempotency_conflict_to_http_conflict() {
        let mapped = map_memory_spi_error(MemorySpiError::IdempotencyConflict {
            idempotency_key: "idem-1".to_string(),
        });
        assert_eq!(mapped.kind, MemoryServiceErrorKind::Conflict);
    }

    #[test]
    fn hides_spi_port_failure_details() {
        let mapped = map_memory_spi_error(MemorySpiError::PortOperationFailed {
            port: "MemoryRecordStorePort".to_string(),
            message: "database password leaked by provider".to_string(),
        });
        assert_eq!(mapped.kind, MemoryServiceErrorKind::Storage);
        assert_eq!(mapped.detail, STORAGE_ERROR_DETAIL);
        assert!(
            !mapped.detail.contains("password"),
            "the masking mapper is the single boundary that must never forward a provider message"
        );
    }

    #[test]
    fn never_forwards_raw_store_error_text() {
        let mapped = map_native_sql_store_error(NativeSqlStoreError::InvariantViolation {
            message: "postgres://user:secret@db/memory".to_string(),
        });
        assert_eq!(mapped.detail, STORAGE_ERROR_DETAIL);
    }
}
