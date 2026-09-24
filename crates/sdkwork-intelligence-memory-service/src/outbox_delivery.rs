use sdkwork_memory_plugin_native_sql::NativeSqlScopedOutboxEvent;
use sdkwork_utils_rust::{format_datetime, is_blank};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxDeliveryMode {
    Disabled,
    Http,
}

pub struct OutboxDeliveryConfig {
    pub mode: OutboxDeliveryMode,
    pub webhook_url: Option<String>,
    /// Stored for diagnostics and configuration inspection; the actual
    /// timeout is baked into `http_client` at construction time.
    pub timeout_seconds: u64,
    pub max_retries: u32,
    /// Shared HTTP client with connection pooling — created once and reused
    /// across all outbox delivery attempts to avoid per-request overhead.
    http_client: tokio::sync::OnceCell<reqwest::Client>,
}

impl OutboxDeliveryConfig {
    pub fn from_env() -> Self {
        let mode = std::env::var("SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .map(|value| match value.as_str() {
                "http" => OutboxDeliveryMode::Http,
                _ => OutboxDeliveryMode::Disabled,
            })
            .unwrap_or(OutboxDeliveryMode::Disabled);
        let webhook_url = std::env::var("SDKWORK_MEMORY_OUTBOX_DELIVERY_URL")
            .ok()
            .filter(|value| !is_blank(Some(value.as_str())))
            .and_then(|value| {
                match crate::endpoint_validation::validate_outbound_url(value.trim()) {
                    Ok(()) => Some(value),
                    Err(reason) => {
                        tracing::error!(reason = %reason, "outbox delivery URL rejected at startup");
                        None
                    }
                }
            });
        let timeout_seconds = std::env::var("SDKWORK_MEMORY_OUTBOX_DELIVERY_TIMEOUT_SECS")
            .ok()
            .and_then(|value| sdkwork_utils_rust::parse_int(&value))
            .unwrap_or(10)
            .clamp(1, 60) as u64;
        let max_retries = std::env::var("SDKWORK_MEMORY_OUTBOX_MAX_RETRIES")
            .ok()
            .and_then(|value| sdkwork_utils_rust::parse_int(&value))
            .unwrap_or(5) as u32;
        Self {
            mode,
            webhook_url,
            timeout_seconds,
            max_retries,
            http_client: tokio::sync::OnceCell::new(),
        }
    }

    pub fn production_ready(&self) -> bool {
        matches!(self.mode, OutboxDeliveryMode::Http) && self.webhook_url.is_some()
    }
}

/// Returns true when outbox HTTP delivery is configured for production-like environments.
pub fn production_outbox_delivery_ready() -> bool {
    if !crate::platform::is_production_like_environment() {
        return true;
    }
    OutboxDeliveryConfig::from_env().production_ready()
}

pub async fn validate_outbox_runtime_config() -> Result<(), String> {
    if !crate::platform::is_production_like_environment() {
        return Ok(());
    }
    let config = OutboxDeliveryConfig::from_env();
    if !matches!(config.mode, OutboxDeliveryMode::Http) {
        return Err(
            "SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE=http is required in production-like environments"
                .to_string(),
        );
    }
    let url = config.webhook_url.as_deref().ok_or_else(|| {
        "SDKWORK_MEMORY_OUTBOX_DELIVERY_URL must be a valid approved HTTPS endpoint in production-like environments"
            .to_string()
    })?;
    crate::endpoint_validation::build_pinned_http_client(
        url,
        std::time::Duration::from_secs(config.timeout_seconds),
        1,
    )
    .await
    .map(|_| ())
    .map_err(|reason| format!("outbox delivery endpoint rejected: {reason}"))
}

pub fn build_cloud_event_envelope(row: &NativeSqlScopedOutboxEvent) -> serde_json::Value {
    let payload = serde_json::from_str::<serde_json::Value>(&row.outbox.payload_json)
        .unwrap_or_else(|_| serde_json::json!({ "raw": row.outbox.payload_json }));
    serde_json::json!({
        "specversion": "1.0",
        "id": row.outbox.outbox_id,
        "type": row.outbox.event_type,
        "source": "sdkwork-memory",
        "time": format_datetime(sdkwork_utils_rust::now(), None),
        "tenantId": row.tenant_id.to_string(),
        "subject": row.outbox.aggregate_id,
        "data": {
            "aggregateType": row.outbox.aggregate_type,
            "aggregateId": row.outbox.aggregate_id,
            "eventVersion": row.outbox.event_version,
            "payload": payload,
        }
    })
}

pub async fn deliver_outbox_event(
    row: &NativeSqlScopedOutboxEvent,
    config: &OutboxDeliveryConfig,
) -> Result<(), String> {
    match config.mode {
        OutboxDeliveryMode::Disabled => Err("outbox delivery is disabled".to_string()),
        OutboxDeliveryMode::Http => {
            let envelope = build_cloud_event_envelope(row);
            let url = config.webhook_url.as_deref().ok_or_else(|| {
                "SDKWORK_MEMORY_OUTBOX_DELIVERY_URL is required when delivery mode is http"
                    .to_string()
            })?;
            let client = config
                .http_client
                .get_or_try_init(|| async {
                    crate::endpoint_validation::build_pinned_http_client(
                        url,
                        std::time::Duration::from_secs(config.timeout_seconds),
                        16,
                    )
                    .await
                    .map(|(_, client)| client)
                })
                .await?;
            let delivery_started = std::time::Instant::now();
            let response = client
                .post(url)
                .header("content-type", "application/json")
                .json(&envelope)
                .send()
                .await;
            crate::domain_metrics::memory_domain_metrics().record_outbox_delivery_completed(
                crate::platform::elapsed_millis_i64(delivery_started),
            );
            let response =
                response.map_err(|error| format!("outbox delivery request failed: {error}"))?;
            if response.status().is_success() {
                tracing::info!(
                    tenant_id = row.tenant_id,
                    outbox_id = %row.outbox.outbox_id,
                    event_type = %row.outbox.event_type,
                    delivery_mode = "http",
                    status = %response.status(),
                    "memory domain outbox event delivered"
                );
                Ok(())
            } else {
                Err(format!(
                    "outbox delivery webhook returned {}",
                    response.status()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_memory_plugin_native_sql::NativeSqlMemoryOutboxEvent;

    #[test]
    fn cloud_event_envelope_includes_tenant_and_event_type() {
        let row = NativeSqlScopedOutboxEvent {
            tenant_id: 100_001,
            lease_owner: None,
            lease_token: None,
            lease_expires_at: None,
            outbox: NativeSqlMemoryOutboxEvent {
                outbox_id: "9001".to_string(),
                aggregate_type: "ai_record".to_string(),
                aggregate_id: "rec-1".to_string(),
                event_type: "memory.record.created".to_string(),
                event_version: "1".to_string(),
                payload_json: r#"{"memoryId":"rec-1"}"#.to_string(),
                publish_state: "processing".to_string(),
                published_at: None,
                retry_count: 0,
            },
        };
        let envelope = build_cloud_event_envelope(&row);
        assert_eq!(envelope["type"], "memory.record.created");
        assert_eq!(envelope["tenantId"], "100001");
        assert_eq!(envelope["data"]["aggregateId"], "rec-1");
    }
}
