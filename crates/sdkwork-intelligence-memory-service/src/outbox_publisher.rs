use std::sync::Arc;
use std::time::Duration;

use sdkwork_memory_plugin_native_sql::{NativeSqlMemoryStore, NativeSqlScopedOutboxEvent};

use crate::outbox_delivery::{deliver_outbox_event, OutboxDeliveryConfig, OutboxDeliveryMode};
use crate::platform;

const DEFAULT_OUTBOX_CONCURRENCY: u64 = 16;
const MAX_OUTBOX_CONCURRENCY: u64 = 64;

pub fn spawn_outbox_publisher(
    store: Arc<NativeSqlMemoryStore>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let config = Arc::new(OutboxDeliveryConfig::from_env());
        if matches!(config.mode, OutboxDeliveryMode::Disabled) {
            tracing::warn!(
                "memory outbox publisher is disabled; pending events remain durable and unacknowledged"
            );
            return;
        }

        let publisher_id = match platform::next_numeric_id() {
            Ok(id) => format!("memory-outbox-{id}"),
            Err(error) => {
                tracing::error!(error = %error.detail, "memory outbox publisher ID initialization failed");
                return;
            }
        };
        let poll_interval =
            platform::read_env_u64("SDKWORK_MEMORY_OUTBOX_POLL_INTERVAL_SECS", 2).max(1);
        let delivery_concurrency = platform::read_env_u64(
            "SDKWORK_MEMORY_OUTBOX_DELIVERY_CONCURRENCY",
            DEFAULT_OUTBOX_CONCURRENCY,
        )
        .clamp(1, MAX_OUTBOX_CONCURRENCY);
        let lease_duration_seconds = platform::read_env_u64(
            "SDKWORK_MEMORY_OUTBOX_LEASE_DURATION_SECS",
            config.timeout_seconds.saturating_mul(2).max(30),
        )
        .max(config.timeout_seconds.saturating_add(5));

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("memory outbox publisher shutting down");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(poll_interval)) => {
                    if let Err(error) = store
                        .requeue_stale_processing_outbox_events(lease_duration_seconds)
                        .await
                    {
                        tracing::warn!(error = %error, "memory outbox expired lease requeue failed");
                    }

                    let lease_token = match platform::next_numeric_id() {
                        Ok(id) => id.to_string(),
                        Err(error) => {
                            tracing::error!(error = %error.detail, "memory outbox lease token generation failed");
                            continue;
                        }
                    };
                    let rows = match store
                        .claim_global_pending_outbox_events(
                            u32::try_from(delivery_concurrency).unwrap_or(16),
                            &publisher_id,
                            &lease_token,
                            lease_duration_seconds,
                        )
                        .await
                    {
                        Ok(rows) => rows,
                        Err(error) => {
                            crate::domain_metrics::memory_domain_metrics()
                                .record_outbox_publish_failed();
                            tracing::error!(error = %error, "memory outbox publisher claim failed");
                            continue;
                        }
                    };

                    let mut deliveries = tokio::task::JoinSet::new();
                    for row in rows {
                        deliveries.spawn(process_claimed_event(
                            store.clone(),
                            config.clone(),
                            row,
                            lease_duration_seconds,
                        ));
                    }
                    while let Some(result) = deliveries.join_next().await {
                        if let Err(error) = result {
                            crate::domain_metrics::memory_domain_metrics()
                                .record_outbox_delivery_failed();
                            tracing::error!(error = %error, "memory outbox delivery task failed");
                        }
                    }
                }
            }
        }
    })
}

async fn process_claimed_event(
    store: Arc<NativeSqlMemoryStore>,
    config: Arc<OutboxDeliveryConfig>,
    row: NativeSqlScopedOutboxEvent,
    lease_duration_seconds: u64,
) {
    let tenant_id = row.tenant_id;
    let outbox_id = row.outbox.outbox_id.clone();
    let Some(lease_owner) = row.lease_owner.clone() else {
        record_fenced_delivery(tenant_id, &outbox_id, "claim did not return a lease owner");
        return;
    };
    let Some(lease_token) = row.lease_token.clone() else {
        record_fenced_delivery(tenant_id, &outbox_id, "claim did not return a lease token");
        return;
    };

    let heartbeat_seconds = (lease_duration_seconds / 3).max(1);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(heartbeat_seconds));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let delivery = deliver_outbox_event(&row, &config);
    tokio::pin!(delivery);

    let delivery_result = loop {
        tokio::select! {
            result = &mut delivery => break Some(result),
            _ = heartbeat.tick() => {
                match store
                    .renew_outbox_delivery_lease(
                        tenant_id,
                        &outbox_id,
                        &lease_owner,
                        &lease_token,
                        lease_duration_seconds,
                    )
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        record_fenced_delivery(tenant_id, &outbox_id, "lease ownership was lost");
                        break None;
                    }
                    Err(error) => {
                        crate::domain_metrics::memory_domain_metrics()
                            .record_outbox_delivery_failed();
                        tracing::error!(
                            tenant_id,
                            outbox_id = %outbox_id,
                            error = %error,
                            "memory outbox lease renewal failed; delivery confirmation fenced"
                        );
                        break None;
                    }
                }
            }
        }
    };
    let Some(delivery_result) = delivery_result else {
        return;
    };

    match delivery_result {
        Ok(()) => match store
            .ack_outbox_delivery_success(tenant_id, &outbox_id, &lease_owner, &lease_token)
            .await
        {
            Ok(Some(_)) => {
                crate::domain_metrics::memory_domain_metrics().record_outbox_published();
            }
            Ok(None) => record_fenced_delivery(
                tenant_id,
                &outbox_id,
                "success acknowledgement lost lease ownership",
            ),
            Err(error) => {
                crate::domain_metrics::memory_domain_metrics().record_outbox_delivery_failed();
                tracing::error!(
                    tenant_id,
                    outbox_id = %outbox_id,
                    error = %error,
                    "memory outbox publish acknowledgement failed"
                );
            }
        },
        Err(error) => {
            crate::domain_metrics::memory_domain_metrics().record_outbox_delivery_failed();
            match store
                .record_outbox_delivery_failure(
                    tenant_id,
                    &outbox_id,
                    &lease_owner,
                    &lease_token,
                    config.max_retries,
                )
                .await
            {
                Ok(Some(updated)) if updated.publish_state == "failed" => {
                    crate::domain_metrics::memory_domain_metrics().record_outbox_dead_letter();
                    tracing::error!(
                        tenant_id,
                        outbox_id = %outbox_id,
                        retry_count = updated.retry_count,
                        error = %error,
                        "memory outbox event moved to dead-letter"
                    );
                }
                Ok(Some(updated)) => tracing::warn!(
                    tenant_id,
                    outbox_id = %outbox_id,
                    retry_count = updated.retry_count,
                    error = %error,
                    "memory outbox delivery failed and was scheduled for retry"
                ),
                Ok(None) => record_fenced_delivery(
                    tenant_id,
                    &outbox_id,
                    "failure acknowledgement lost lease ownership",
                ),
                Err(store_error) => tracing::error!(
                    tenant_id,
                    outbox_id = %outbox_id,
                    error = %store_error,
                    delivery_error = %error,
                    "memory outbox delivery failure persistence failed"
                ),
            }
        }
    }
}

fn record_fenced_delivery(tenant_id: i64, outbox_id: &str, reason: &str) {
    crate::domain_metrics::memory_domain_metrics().record_outbox_delivery_failed();
    tracing::warn!(
        tenant_id,
        outbox_id,
        reason,
        "memory outbox delivery result was fenced"
    );
}
