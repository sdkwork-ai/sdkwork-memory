//! Background workers: learning job queue, eval runs, provider health probes.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use sdkwork_memory_contract::{
    MemoryExtractionRequest, MemoryLearningJob, MemoryMigrationJobRequest, MemoryOpenApi,
    MemoryOpenApiRequestContext, MemoryRetentionJobRequest, MemoryRetrievalRequest,
    MemoryServiceError,
};
use sdkwork_memory_plugin_native_sql::{
    ConsolidateDuplicateRecordsCommand, FinishLearningJobCommand, InsertLearningJobCommand,
    NativeSqlClaimedEvalRun, NativeSqlEvalRunRow, NativeSqlLearningJobRow, NativeSqlMemoryStore,
    UpdateEvalRunStateCommand,
};
use sdkwork_memory_spi::MemoryScopeContext;
use sdkwork_utils_rust::is_blank;
use serde::Deserialize;
use serde_json::Value;

use crate::open_api::OpenMemoryService;
use crate::platform;

/// Spawns all background workers and returns a shutdown sender.
///
/// The caller MUST keep the sender alive and call `send(true)` during
/// graceful shutdown so that all workers drain in-flight work and exit.
/// Dropping the sender also causes receivers to observe a closed channel,
/// but an explicit `send(true)` ensures a cleaner shutdown with logged
/// confirmation from each worker.
pub fn spawn_background_workers(service: Arc<OpenMemoryService>) -> MemoryBackgroundWorkers {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let join_handles = vec![
        crate::outbox_publisher::spawn_outbox_publisher(service.store.clone(), shutdown_rx.clone()),
        spawn_learning_job_worker(service.clone(), shutdown_rx.clone()),
        spawn_eval_run_worker(service.clone(), shutdown_rx.clone()),
        spawn_provider_health_probe(service, shutdown_rx),
    ];
    MemoryBackgroundWorkers {
        shutdown_tx,
        join_handles,
    }
}

/// Process-owner handle for the Memory background plane.
///
/// API_ASSEMBLY_SPEC §6.2.1 (lines 744-748) makes task *shutdown* a host
/// concern: the module hands the host both the stop trigger and the drain
/// handles. A bare stop trigger would force the host to guess a sleep duration,
/// which either abandons committed work or needlessly delays every rollout.
/// Handing back the `JoinHandle`s lets the host await real completion under a
/// bounded budget instead.
///
/// The safety net for work that outlives the budget is the database lease, not
/// a longer sleep: every job and outbox row this plane claims carries a
/// `lease_owner`/`lease_token` fence, so abandoned work is re-claimable by
/// another replica once its lease expires. The host therefore only has to
/// bound *how long* it waits, never *whether* the work can be recovered.
#[must_use = "dropping this handle leaves the background plane running until process exit"]
pub struct MemoryBackgroundWorkers {
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    join_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl MemoryBackgroundWorkers {
    /// Signals every worker to stop accepting new work.
    ///
    /// Work already committed to is not interrupted: each worker finishes the
    /// batch it is currently executing and then observes the stop trigger at
    /// the top of its next loop iteration.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Awaits actual worker completion, bounded by `budget`.
    ///
    /// Returns `true` when every worker finished inside the budget. On expiry
    /// the remaining workers are aborted and `false` is returned so the caller
    /// can report the incomplete drain; their abandoned leases expire and the
    /// work becomes re-claimable. The budget is enforced across the whole set,
    /// not per worker, so a long tail cannot extend total shutdown time.
    pub async fn drain(&mut self, budget: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + budget;
        let mut drained = true;
        for mut handle in self.join_handles.drain(..) {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                drained = false;
                handle.abort();
                continue;
            }
            if tokio::time::timeout(remaining, &mut handle).await.is_err() {
                drained = false;
                handle.abort();
            }
        }
        drained
    }
}

fn spawn_learning_job_worker(
    service: Arc<OpenMemoryService>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let worker_id = match platform::next_numeric_id() {
            Ok(id) => format!("memory-learning-{id}"),
            Err(error) => {
                tracing::error!(error = %error.detail, "memory learning worker ID initialization failed");
                return;
            }
        };
        let lease_duration_seconds =
            platform::read_env_u64("SDKWORK_MEMORY_JOB_LEASE_DURATION_SECS", 900).max(30);
        let poll_interval =
            platform::read_env_u64("SDKWORK_MEMORY_JOB_POLL_INTERVAL_SECS", 2).max(1);
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("memory learning job worker shutting down");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(poll_interval)) => {
                    if let Err(error) = process_learning_job_batch(
                        service.clone(),
                        &worker_id,
                        lease_duration_seconds,
                    ).await {
                        tracing::warn!(error = %error, "memory learning job worker batch failed");
                    }
                }
            }
        }
    })
}

fn spawn_eval_run_worker(
    service: Arc<OpenMemoryService>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let worker_id = match platform::next_numeric_id() {
            Ok(id) => format!("memory-eval-{id}"),
            Err(error) => {
                tracing::error!(error = %error.detail, "memory eval worker ID initialization failed");
                return;
            }
        };
        let lease_duration_seconds =
            platform::read_env_u64("SDKWORK_MEMORY_EVAL_LEASE_DURATION_SECS", 900).max(30);
        let poll_interval =
            platform::read_env_u64("SDKWORK_MEMORY_EVAL_POLL_INTERVAL_SECS", 5).max(1);
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("memory eval run worker shutting down");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(poll_interval)) => {
                    if let Err(error) = process_eval_run_batch(
                        service.clone(),
                        &worker_id,
                        lease_duration_seconds,
                    ).await {
                        tracing::warn!(error = %error, "memory eval run worker batch failed");
                    }
                }
            }
        }
    })
}

fn spawn_provider_health_probe(
    service: Arc<OpenMemoryService>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let poll_interval =
            platform::read_env_u64("SDKWORK_MEMORY_PROVIDER_HEALTH_PROBE_SECS", 60).max(1);
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("memory provider health probe shutting down");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(poll_interval)) => {
                    if let Err(error) = probe_provider_bindings(&service).await {
                        tracing::warn!(error = %error, "memory provider health probe failed");
                    }
                }
            }
        }
    })
}

async fn process_learning_job_batch(
    service: Arc<OpenMemoryService>,
    worker_id: &str,
    lease_duration_seconds: u64,
) -> Result<(), String> {
    let _ = service
        .store
        .requeue_stale_running_learning_jobs(lease_duration_seconds)
        .await
        .map_err(|error| error.to_string())?;
    let lease_token = platform::next_numeric_id()
        .map_err(|error| error.detail)?
        .to_string();
    let concurrency = platform::read_env_u64("SDKWORK_MEMORY_JOB_CONCURRENCY", 8).clamp(1, 32);
    let jobs = service
        .store
        .claim_queued_learning_jobs(
            i32::try_from(concurrency).unwrap_or(8),
            worker_id,
            &lease_token,
            lease_duration_seconds,
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut tasks = tokio::task::JoinSet::new();
    for job in jobs {
        tasks.spawn(process_claimed_learning_job(
            service.clone(),
            job,
            lease_duration_seconds,
        ));
    }
    while let Some(result) = tasks.join_next().await {
        result.map_err(|error| format!("memory learning job task failed: {error}"))?;
    }
    Ok(())
}

async fn process_claimed_learning_job(
    service: Arc<OpenMemoryService>,
    job: NativeSqlLearningJobRow,
    lease_duration_seconds: u64,
) {
    let (Some(lease_owner), Some(lease_token)) =
        (job.lease_owner.as_deref(), job.lease_token.as_deref())
    else {
        tracing::error!(job_uuid = %job.job_uuid, "claimed learning job is missing lease identity");
        return;
    };
    let heartbeat_seconds = (lease_duration_seconds / 3).max(1);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(heartbeat_seconds));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let execution = execute_learning_job(&service, &job);
    tokio::pin!(execution);
    let outcome = loop {
        tokio::select! {
            result = &mut execution => break Some(result),
            _ = heartbeat.tick() => {
                match service.store.renew_learning_job_lease(
                    job.tenant_id,
                    &job.job_uuid,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                ).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(job_uuid = %job.job_uuid, "learning job execution fenced after lease loss");
                        break None;
                    }
                    Err(error) => {
                        tracing::error!(job_uuid = %job.job_uuid, error = %error, "learning job lease renewal failed");
                        break None;
                    }
                }
            }
        }
    };
    let Some(outcome) = outcome else {
        return;
    };
    let (state, result_json, error_json) = match outcome {
        Ok(result) => ("succeeded", Some(result), None),
        Err(error) => {
            tracing::warn!(
                tenant_id = job.tenant_id,
                job_uuid = %job.job_uuid,
                job_type = %job.job_type,
                error = %error,
                "memory learning job failed"
            );
            (
                "failed",
                None,
                Some(serde_json::json!({ "message": error }).to_string()),
            )
        }
    };
    match service
        .store
        .finish_learning_job(FinishLearningJobCommand {
            tenant_id: job.tenant_id,
            job_uuid: &job.job_uuid,
            lease_owner,
            lease_token,
            state,
            result_json: result_json.as_deref(),
            error_json: error_json.as_deref(),
        })
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => tracing::warn!(job_uuid = %job.job_uuid, "learning job completion was fenced"),
        Err(error) => {
            tracing::error!(job_uuid = %job.job_uuid, error = %error, "learning job completion persistence failed")
        }
    }
}

fn assert_learning_job_space_id(
    job: &NativeSqlLearningJobRow,
    space_id: i64,
) -> Result<(), String> {
    match job.space_id {
        Some(job_space_id) if job_space_id != space_id => Err(format!(
            "learning job space_id {job_space_id} does not match input space_id {space_id}"
        )),
        None => Err(format!(
            "learning job {job_uuid} missing space_id; scope mutations require an explicit space",
            job_uuid = job.job_uuid
        )),
        Some(_) => Ok(()),
    }
}

fn parse_background_job_actor_id(input_json: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(input_json)
        .ok()
        .and_then(|value| {
            value
                .get("actorId")
                .or_else(|| value.get("actor_id"))
                .and_then(|id| {
                    id.as_u64()
                        .or_else(|| id.as_i64().and_then(|v| u64::try_from(v.max(0)).ok()))
                })
        })
}

fn background_job_context(
    job: &NativeSqlLearningJobRow,
    input_json: &str,
) -> MemoryOpenApiRequestContext {
    MemoryOpenApiRequestContext::for_background_job(
        u64::try_from(job.tenant_id.max(0)).unwrap_or(0),
        parse_background_job_actor_id(input_json),
    )
}

async fn execute_learning_job(
    service: &OpenMemoryService,
    job: &NativeSqlLearningJobRow,
) -> Result<String, String> {
    let input = job
        .input_json
        .as_deref()
        .ok_or_else(|| "learning job input_json is missing".to_string())?;
    let open_context = background_job_context(job, input);
    let result = match job.job_type.as_str() {
        "extract" | "extraction" => {
            let request: MemoryExtractionRequest = serde_json::from_str(input)
                .map_err(|error| format!("extraction input decode failed: {error}"))?;
            let space_id =
                platform::space_id_i64(request.space_id).map_err(|error| error.detail)?;
            assert_learning_job_space_id(job, space_id)?;
            if open_context.actor_id.is_none() {
                return Err(
                    "extraction background job requires actorId in input_json for authorization"
                        .to_string(),
                );
            }
            OpenMemoryService::execute_extraction_work(service, open_context, request)
                .await
                .map_err(|error| error.detail)?
                .to_string()
        }
        "consolidation" => {
            let request: MemoryExtractionRequest = serde_json::from_str(input)
                .map_err(|error| format!("consolidation input decode failed: {error}"))?;
            let tenant_id = job.tenant_id;
            let space_id =
                platform::space_id_i64(request.space_id).map_err(|error| error.detail)?;
            assert_learning_job_space_id(job, space_id)?;
            let scope = MemoryScopeContext {
                tenant_id,
                space_id,
                organization_id: None,
                user_id: None,
            };
            let consolidation = service
                .store
                .consolidate_duplicate_records_in_scope_detailed(
                    ConsolidateDuplicateRecordsCommand {
                        scope: &scope,
                        operation_id: &job.job_uuid,
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            serde_json::json!({
                "mergedDuplicates": consolidation.superseded_records,
                "supersededDuplicates": consolidation.superseded_records,
                "transferredSources": consolidation.transferred_sources,
                "deduplicatedSources": consolidation.deduplicated_sources,
                "consolidationMode": "identity_bounded_supersession",
                "spaceId": request.space_id
            })
            .to_string()
        }
        "retention" => {
            let request: MemoryRetentionJobRequest = serde_json::from_str(input)
                .map_err(|error| format!("retention input decode failed: {error}"))?;
            let tenant_id = job.tenant_id;
            let space_id = request
                .space_id
                .map(platform::space_id_i64)
                .transpose()
                .map_err(|error| error.detail)?
                .ok_or_else(|| "retention input missing spaceId".to_string())?;
            assert_learning_job_space_id(job, space_id)?;
            let scope = MemoryScopeContext {
                tenant_id,
                space_id,
                organization_id: None,
                user_id: None,
            };
            let dry_run = request.dry_run.unwrap_or(false);
            let deleted = service
                .store
                .purge_expired_records_for_scope(&scope, dry_run)
                .await
                .map_err(|error| error.to_string())?;
            serde_json::json!({ "deletedRecords": deleted, "dryRun": dry_run }).to_string()
        }
        "index_rebuild" | "index_sync" => {
            let payload: Value = serde_json::from_str(input)
                .map_err(|error| format!("index rebuild input decode failed: {error}"))?;
            let index_id = payload
                .get("indexId")
                .and_then(Value::as_u64)
                .ok_or_else(|| "index rebuild input missing indexId".to_string())?;
            let index_row = service
                .store
                .retrieve_mem_index_for_tenant(job.tenant_id, &index_id.to_string())
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("memory index {index_id} not found for tenant"))?;
            let rebuilt = if let Some(space_id) = index_row.space_id {
                service
                    .store
                    .rebuild_record_search_indexes_for_space(job.tenant_id, space_id)
                    .await
                    .map_err(|error| error.to_string())?
            } else {
                service
                    .store
                    .rebuild_all_record_search_indexes(job.tenant_id)
                    .await
                    .map_err(|error| error.to_string())?
            };
            let _ = service
                .store
                .update_mem_index_for_tenant(
                    job.tenant_id,
                    &index_id.to_string(),
                    Some("active"),
                    None,
                    Some(platform::current_timestamp().as_str()),
                    None,
                    None,
                )
                .await;
            serde_json::json!({
                "indexId": index_id,
                "spaceId": index_row.space_id,
                "rebuiltRecords": rebuilt
            })
            .to_string()
        }
        "migration" => {
            let request: MemoryMigrationJobRequest = serde_json::from_str(input)
                .map_err(|error| format!("migration input decode failed: {error}"))?;
            let preference_id = i64::try_from(service.next_id().map_err(|error| error.detail)?)
                .map_err(|_| "generated preference id out of range".to_string())?;
            let result = crate::implementation_migration::execute_implementation_profile_migration(
                &service.store,
                preference_id,
                job.tenant_id,
                &request,
                &service.core_runtime.profile().profile_id,
                service.active_implementation_kind_code(),
            )
            .await
            .map_err(|error| error.detail)?;
            serde_json::to_string(&result)
                .map_err(|error| format!("migration result encode failed: {error}"))?
        }
        other => return Err(format!("unsupported learning job type: {other}")),
    };
    Ok(result)
}

async fn process_eval_run_batch(
    service: Arc<OpenMemoryService>,
    worker_id: &str,
    lease_duration_seconds: u64,
) -> Result<(), String> {
    let _ = service
        .store
        .requeue_stale_running_eval_runs(lease_duration_seconds)
        .await
        .map_err(|error| error.to_string())?;
    let lease_token = platform::next_numeric_id()
        .map_err(|error| error.detail)?
        .to_string();
    let concurrency = platform::read_env_u64("SDKWORK_MEMORY_EVAL_CONCURRENCY", 4).clamp(1, 16);
    let runs = service
        .store
        .claim_queued_eval_runs(
            i32::try_from(concurrency).unwrap_or(4),
            worker_id,
            &lease_token,
            lease_duration_seconds,
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut tasks = tokio::task::JoinSet::new();
    for run in runs {
        tasks.spawn(process_claimed_eval_run(
            service.clone(),
            run,
            lease_duration_seconds,
        ));
    }
    while let Some(result) = tasks.join_next().await {
        result.map_err(|error| format!("memory eval task failed: {error}"))?;
    }
    Ok(())
}

async fn process_claimed_eval_run(
    service: Arc<OpenMemoryService>,
    run: NativeSqlClaimedEvalRun,
    lease_duration_seconds: u64,
) {
    let heartbeat_seconds = (lease_duration_seconds / 3).max(1);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(heartbeat_seconds));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let execution = evaluate_claimed_run(&service, &run);
    tokio::pin!(execution);
    let outcome = loop {
        tokio::select! {
            result = &mut execution => break Some(result),
            _ = heartbeat.tick() => {
                match service.store.renew_eval_run_lease(
                    run.tenant_id,
                    &run.eval_run_uuid,
                    &run.lease_owner,
                    &run.lease_token,
                    lease_duration_seconds,
                ).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(eval_run_uuid = %run.eval_run_uuid, "eval run execution fenced after lease loss");
                        break None;
                    }
                    Err(error) => {
                        tracing::error!(eval_run_uuid = %run.eval_run_uuid, error = %error, "eval run lease renewal failed");
                        break None;
                    }
                }
            }
        }
    };
    let (state, metrics, result) = match outcome {
        Some(Ok(outcome)) => outcome,
        Some(Err(error)) => {
            tracing::error!(eval_run_uuid = %run.eval_run_uuid, error = %error, "eval run execution failed before completion");
            return;
        }
        None => return,
    };
    match service
        .store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id: run.tenant_id,
            eval_run_uuid: &run.eval_run_uuid,
            lease_owner: &run.lease_owner,
            lease_token: &run.lease_token,
            state,
            metrics_json: metrics.as_deref(),
            result_json: Some(&result),
        })
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(eval_run_uuid = %run.eval_run_uuid, "eval run completion was fenced")
        }
        Err(error) => {
            tracing::error!(eval_run_uuid = %run.eval_run_uuid, error = %error, "eval run completion persistence failed")
        }
    }
}

async fn evaluate_claimed_run(
    service: &OpenMemoryService,
    run: &NativeSqlClaimedEvalRun,
) -> Result<(&'static str, Option<String>, String), String> {
    let row = service
        .store
        .retrieve_mem_eval_run_for_tenant(run.tenant_id, &run.eval_run_uuid)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("claimed eval run {} no longer exists", run.eval_run_uuid))?;
    Ok(match run.eval_type.as_str() {
        "retrieval_quality" => {
            match run_retrieval_quality_eval(service, run.tenant_id, &row).await {
                Ok(output) => ("succeeded", Some(output.metrics), output.result),
                Err(error) => (
                    "failed",
                    None,
                    serde_json::json!({
                        "evalType": "retrieval_quality",
                        "status": "failed",
                        "reason": error,
                    })
                    .to_string(),
                ),
            }
        }
        other => (
            "skipped",
            None,
            serde_json::json!({
                "evalType": other,
                "status": "skipped",
                "reason": "eval type is not implemented"
            })
            .to_string(),
        ),
    })
}

async fn run_retrieval_quality_eval(
    service: &OpenMemoryService,
    tenant_id: i64,
    row: &NativeSqlEvalRunRow,
) -> Result<RetrievalQualityEvalOutput, String> {
    let config_json = row.result_json.as_deref().ok_or_else(|| {
        if row.dataset_ref.is_some() {
            "datasetRef resolution is not configured; retrieval_quality requires inline config.cases"
                .to_string()
        } else {
            "retrieval_quality requires inline config.cases".to_string()
        }
    })?;
    let config: RetrievalQualityEvalConfig = serde_json::from_str(config_json)
        .map_err(|error| format!("retrieval quality config is invalid: {error}"))?;
    validate_retrieval_quality_config(&config)?;

    let retrieval_profile_id = row
        .profile_ref
        .as_deref()
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "profileRef must be a numeric retrieval profile id".to_string())
        })
        .transpose()?;
    if let Some(profile_id) = retrieval_profile_id {
        let profile_exists = service
            .store
            .retrieve_mem_retrieval_profile_for_tenant(tenant_id, &profile_id.to_string())
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        if !profile_exists {
            return Err(format!(
                "retrieval profile {profile_id} was not found for the eval tenant"
            ));
        }
    }

    let tenant_id =
        u64::try_from(tenant_id).map_err(|_| "eval tenant id must be non-negative".to_string())?;
    let mut scores = Vec::with_capacity(config.cases.len());
    let mut case_results = Vec::with_capacity(config.cases.len());
    for case in &config.cases {
        let space_id = parse_json_id(&case.space_id, "cases[].spaceId")?;
        let expected_memory_ids = case
            .expected_memory_ids
            .iter()
            .map(|value| parse_json_id(value, "cases[].expectedMemoryIds[]"))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let top_k = case.top_k.unwrap_or(10);
        let started_at = Instant::now();
        let retrieval = MemoryOpenApi::create_retrieval(
            service,
            MemoryOpenApiRequestContext::for_backend_surface(tenant_id, None),
            MemoryRetrievalRequest {
                query: case.query.clone(),
                space_ids: vec![space_id],
                actor_id: None,
                retrieval_profile_id,
                memory_types: None,
                filters: None,
                top_k,
                context_budget_tokens: config.context_budget_tokens,
                include_trace: Some(false),
            },
        )
        .await
        .map_err(|error| {
            format!(
                "retrieval case {} failed: {}",
                platform::stable_query_hash(&case.query),
                error.detail
            )
        })?;
        let latency_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let retrieved_ids = retrieval
            .hits
            .iter()
            .filter_map(|hit| hit.memory_id)
            .collect::<Vec<_>>();
        let score = score_retrieval_case(
            &expected_memory_ids,
            &retrieved_ids,
            usize::try_from(top_k).unwrap_or(usize::MAX),
            retrieval.degraded,
            latency_ms,
        );
        case_results.push(serde_json::json!({
            "queryHash": platform::stable_query_hash(&case.query),
            "spaceId": space_id.to_string(),
            "topK": top_k,
            "expectedCount": expected_memory_ids.len(),
            "retrievedCount": score.retrieved_count,
            "retrievedRelevantCount": score.retrieved_relevant_count,
            "recallAtK": score.recall_at_k,
            "precisionAtK": score.precision_at_k,
            "ndcgAtK": score.ndcg_at_k,
            "reciprocalRank": score.reciprocal_rank,
            "latencyMs": score.latency_ms,
            "degraded": score.degraded,
        }));
        scores.push(score);
    }

    let case_count = scores.len() as f64;
    let recall_at_k = scores.iter().map(|score| score.recall_at_k).sum::<f64>() / case_count;
    let precision_at_k = scores.iter().map(|score| score.precision_at_k).sum::<f64>() / case_count;
    let mean_ndcg_at_k = scores.iter().map(|score| score.ndcg_at_k).sum::<f64>() / case_count;
    let hit_rate_at_k = scores
        .iter()
        .filter(|score| score.retrieved_relevant_count > 0)
        .count() as f64
        / case_count;
    let mean_reciprocal_rank = scores
        .iter()
        .map(|score| score.reciprocal_rank)
        .sum::<f64>()
        / case_count;
    let degraded_case_count = scores.iter().filter(|score| score.degraded).count();
    let degraded_rate = degraded_case_count as f64 / case_count;
    let p95_latency_ms = percentile_95_nearest_rank(
        &scores
            .iter()
            .map(|score| score.latency_ms)
            .collect::<Vec<_>>(),
    );
    let quality_gate_passed = config.thresholds.as_ref().map(|thresholds| {
        thresholds
            .min_recall_at_k
            .is_none_or(|minimum| recall_at_k >= minimum)
            && thresholds
                .min_hit_rate_at_k
                .is_none_or(|minimum| hit_rate_at_k >= minimum)
            && thresholds
                .min_mean_reciprocal_rank
                .is_none_or(|minimum| mean_reciprocal_rank >= minimum)
            && thresholds
                .min_precision_at_k
                .is_none_or(|minimum| precision_at_k >= minimum)
            && thresholds
                .min_mean_ndcg_at_k
                .is_none_or(|minimum| mean_ndcg_at_k >= minimum)
            && thresholds
                .max_degraded_rate
                .is_none_or(|maximum| degraded_rate <= maximum)
            && thresholds
                .max_p95_latency_ms
                .is_none_or(|maximum| p95_latency_ms <= maximum)
    });
    let metrics = serde_json::json!({
        "caseCount": scores.len(),
        "recallAtK": recall_at_k,
        "precisionAtK": precision_at_k,
        "meanNdcgAtK": mean_ndcg_at_k,
        "hitRateAtK": hit_rate_at_k,
        "meanReciprocalRank": mean_reciprocal_rank,
        "p95LatencyMs": p95_latency_ms,
        "degradedCaseCount": degraded_case_count,
        "degradedRate": degraded_rate,
        "qualityGatePassed": quality_gate_passed,
    });
    let result = serde_json::json!({
        "evalType": "retrieval_quality",
        "status": "completed",
        "datasetRef": row.dataset_ref,
        "profileRef": row.profile_ref,
        "thresholds": config.thresholds,
        "qualityGatePassed": quality_gate_passed,
        "cases": case_results,
    });
    Ok(RetrievalQualityEvalOutput {
        metrics: metrics.to_string(),
        result: result.to_string(),
    })
}

const MAX_RETRIEVAL_EVAL_CASES: usize = 1_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetrievalQualityEvalConfig {
    cases: Vec<RetrievalQualityEvalCase>,
    #[serde(default = "default_eval_context_budget_tokens")]
    context_budget_tokens: i32,
    #[serde(default)]
    thresholds: Option<RetrievalQualityThresholds>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetrievalQualityEvalCase {
    space_id: Value,
    query: String,
    expected_memory_ids: Vec<Value>,
    #[serde(default)]
    top_k: Option<i32>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetrievalQualityThresholds {
    #[serde(default)]
    min_recall_at_k: Option<f64>,
    #[serde(default)]
    min_hit_rate_at_k: Option<f64>,
    #[serde(default)]
    min_mean_reciprocal_rank: Option<f64>,
    #[serde(default)]
    min_precision_at_k: Option<f64>,
    #[serde(default)]
    min_mean_ndcg_at_k: Option<f64>,
    #[serde(default)]
    max_degraded_rate: Option<f64>,
    #[serde(default)]
    max_p95_latency_ms: Option<u64>,
}

struct RetrievalQualityEvalOutput {
    metrics: String,
    result: String,
}

#[derive(Debug, PartialEq)]
struct RetrievalCaseScore {
    retrieved_count: usize,
    retrieved_relevant_count: usize,
    recall_at_k: f64,
    precision_at_k: f64,
    ndcg_at_k: f64,
    reciprocal_rank: f64,
    latency_ms: u64,
    degraded: bool,
}

fn default_eval_context_budget_tokens() -> i32 {
    4_096
}

fn validate_retrieval_quality_config(config: &RetrievalQualityEvalConfig) -> Result<(), String> {
    if config.cases.is_empty() || config.cases.len() > MAX_RETRIEVAL_EVAL_CASES {
        return Err(format!(
            "retrieval quality cases must contain between 1 and {MAX_RETRIEVAL_EVAL_CASES} entries"
        ));
    }
    crate::retrieval_profile::validate_retrieval_limits(1, config.context_budget_tokens)
        .map_err(|error| error.detail)?;
    for case in &config.cases {
        if case.query.trim().is_empty() {
            return Err("retrieval quality case query must not be blank".to_string());
        }
        let _ = parse_json_id(&case.space_id, "cases[].spaceId")?;
        if case.expected_memory_ids.is_empty() {
            return Err("retrieval quality case expectedMemoryIds must not be empty".to_string());
        }
        let expected = case
            .expected_memory_ids
            .iter()
            .map(|value| parse_json_id(value, "cases[].expectedMemoryIds[]"))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        if expected.len() != case.expected_memory_ids.len() {
            return Err(
                "retrieval quality case expectedMemoryIds must not contain duplicates".to_string(),
            );
        }
        crate::retrieval_profile::validate_retrieval_limits(
            case.top_k.unwrap_or(10),
            config.context_budget_tokens,
        )
        .map_err(|error| error.detail)?;
    }
    if let Some(thresholds) = &config.thresholds {
        let values = [
            thresholds.min_recall_at_k,
            thresholds.min_hit_rate_at_k,
            thresholds.min_mean_reciprocal_rank,
            thresholds.min_precision_at_k,
            thresholds.min_mean_ndcg_at_k,
            thresholds.max_degraded_rate,
        ];
        if values.iter().all(Option::is_none) && thresholds.max_p95_latency_ms.is_none() {
            return Err("retrieval quality thresholds must define at least one gate".to_string());
        }
        if values
            .iter()
            .flatten()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(
                "retrieval quality thresholds must be finite values from 0 to 1".to_string(),
            );
        }
        if thresholds.max_p95_latency_ms == Some(0) {
            return Err("retrieval quality maxP95LatencyMs must be greater than 0".to_string());
        }
    }
    Ok(())
}

pub(crate) fn validate_retrieval_quality_eval_request(
    config: Option<&Value>,
) -> Result<(), String> {
    let config = config.ok_or_else(|| {
        "retrieval_quality requires inline config.cases; datasetRef-only resolution is not configured"
            .to_string()
    })?;
    let config: RetrievalQualityEvalConfig = serde_json::from_value(config.clone())
        .map_err(|error| format!("retrieval quality config is invalid: {error}"))?;
    validate_retrieval_quality_config(&config)
}

fn parse_json_id(value: &Value, field: &str) -> Result<u64, String> {
    match value {
        Value::String(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{field} must be an unsigned integer string or number")),
        Value::Number(value) => value
            .as_u64()
            .ok_or_else(|| format!("{field} must be an unsigned integer string or number")),
        _ => Err(format!(
            "{field} must be an unsigned integer string or number"
        )),
    }
}

fn score_retrieval_case(
    expected_memory_ids: &std::collections::BTreeSet<u64>,
    retrieved_memory_ids: &[u64],
    top_k: usize,
    degraded: bool,
    latency_ms: u64,
) -> RetrievalCaseScore {
    let mut seen = std::collections::BTreeSet::new();
    let mut retrieved_relevant_count = 0usize;
    let mut first_relevant_rank = None;
    let mut discounted_cumulative_gain = 0.0;
    for memory_id in retrieved_memory_ids {
        if !seen.insert(*memory_id) {
            continue;
        }
        let rank = seen.len();
        if rank > top_k {
            break;
        }
        if !expected_memory_ids.contains(memory_id) {
            continue;
        }
        retrieved_relevant_count += 1;
        first_relevant_rank.get_or_insert(rank);
        discounted_cumulative_gain += 1.0 / ((rank + 1) as f64).log2();
    }
    let retrieved_count = seen.len().min(top_k);
    let ideal_relevant_count = expected_memory_ids.len().min(top_k);
    let ideal_discounted_cumulative_gain = (1..=ideal_relevant_count)
        .map(|rank| 1.0 / ((rank + 1) as f64).log2())
        .sum::<f64>();
    RetrievalCaseScore {
        retrieved_count,
        retrieved_relevant_count,
        recall_at_k: retrieved_relevant_count as f64 / expected_memory_ids.len() as f64,
        precision_at_k: if retrieved_count == 0 {
            0.0
        } else {
            retrieved_relevant_count as f64 / retrieved_count as f64
        },
        ndcg_at_k: if ideal_discounted_cumulative_gain == 0.0 {
            0.0
        } else {
            discounted_cumulative_gain / ideal_discounted_cumulative_gain
        },
        reciprocal_rank: first_relevant_rank
            .map(|rank| 1.0 / rank as f64)
            .unwrap_or(0.0),
        latency_ms,
        degraded,
    }
}

fn percentile_95_nearest_rank(latencies_ms: &[u64]) -> u64 {
    if latencies_ms.is_empty() {
        return 0;
    }
    let mut sorted = latencies_ms.to_vec();
    sorted.sort_unstable();
    let rank = sorted.len().saturating_mul(95).div_ceil(100).max(1);
    sorted[rank - 1]
}

async fn probe_provider_bindings(service: &OpenMemoryService) -> Result<(), String> {
    let Some(_lease) = service
        .store
        .try_acquire_provider_health_lease()
        .await
        .map_err(|error| error.to_string())?
    else {
        tracing::debug!("provider health probe skipped because another replica holds the lease");
        return Ok(());
    };

    let page_size = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;
    let page_size_usize = usize::try_from(page_size).unwrap_or(200);
    let mut tenant_cursor = None;
    loop {
        let tenants = service
            .store
            .list_tenant_ids_with_provider_bindings_page(tenant_cursor, page_size)
            .await
            .map_err(|error| error.to_string())?;
        let has_more_tenants = tenants.len() > page_size_usize;
        for tenant_id in tenants.iter().take(page_size_usize).copied() {
            let mut binding_cursor = None;
            loop {
                let rows = service
                    .store
                    .list_mem_provider_bindings_for_tenant(
                        tenant_id,
                        page_size,
                        binding_cursor.as_deref(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                let has_more_bindings = rows.len() > page_size_usize;
                probe_provider_binding_batch(service, tenant_id, rows.iter().take(page_size_usize))
                    .await;
                if !has_more_bindings {
                    break;
                }
                binding_cursor = rows
                    .get(page_size_usize.saturating_sub(1))
                    .map(|row| row.binding_uuid.clone());
            }
        }
        if !has_more_tenants {
            break;
        }
        tenant_cursor = tenants.get(page_size_usize.saturating_sub(1)).copied();
    }
    Ok(())
}

async fn probe_provider_binding_batch<'a>(
    service: &OpenMemoryService,
    tenant_id: i64,
    rows: impl Iterator<Item = &'a sdkwork_memory_plugin_native_sql::NativeSqlProviderBindingRow>,
) {
    let concurrency = usize::try_from(
        platform::read_env_u64("SDKWORK_MEMORY_PROVIDER_HEALTH_CONCURRENCY", 8).clamp(1, 32),
    )
    .unwrap_or(8);
    stream::iter(rows)
        .for_each_concurrent(concurrency, |row| async move {
            let health = probe_binding_endpoint(row.endpoint_ref.as_deref()).await;
            let now = platform::current_timestamp();
            if let Err(error) = service
                .store
                .update_mem_provider_binding_for_tenant(
                    tenant_id,
                    &row.binding_uuid,
                    None,
                    None,
                    Some(health.as_str()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(Some(now.as_str())),
                )
                .await
            {
                tracing::warn!(
                    tenant_id,
                    binding_id = %row.binding_uuid,
                    error = %error,
                    "provider health state update failed"
                );
            }
        })
        .await;
}

async fn probe_binding_endpoint(endpoint_ref: Option<&str>) -> String {
    let Some(url) = endpoint_ref.filter(|value| !is_blank(Some(value))) else {
        return "healthy".to_string();
    };

    if let Err(reason) = crate::endpoint_validation::validate_outbound_url(url) {
        tracing::warn!(
            reason = %reason,
            "provider health probe rejected unsafe endpoint URL"
        );
        return "unhealthy".to_string();
    }

    let (resolved_url, client) = match crate::endpoint_validation::build_pinned_http_client(
        url,
        Duration::from_secs(5),
        1,
    )
    .await
    {
        Ok(target) => target,
        Err(reason) => {
            tracing::warn!(reason = %reason, "provider health probe rejected endpoint resolution");
            return "unhealthy".to_string();
        }
    };
    match client.get(resolved_url).send().await {
        Ok(response) if response.status().is_success() => "healthy".to_string(),
        Ok(_) => "degraded".to_string(),
        Err(_) => "unhealthy".to_string(),
    }
}

pub async fn enqueue_learning_job(
    store: &NativeSqlMemoryStore,
    tenant_id: i64,
    job_id: u64,
    job_type: &str,
    space_id: Option<u64>,
    input_json: &str,
    priority: i32,
) -> Result<(), sdkwork_memory_plugin_native_sql::NativeSqlStoreError> {
    store
        .insert_learning_job(InsertLearningJobCommand {
            tenant_id,
            job_uuid: &job_id.to_string(),
            space_id: platform::optional_u64_as_i64(space_id).ok().flatten(),
            job_type,
            state: "queued",
            priority,
            idempotency_key: None,
            input_json: Some(input_json),
        })
        .await
}

pub fn learning_job_from_row(
    row: &NativeSqlLearningJobRow,
) -> MemoryServiceResult<MemoryLearningJob> {
    let job_id = crate::platform::parse_numeric_id(&row.job_uuid).ok_or_else(|| {
        MemoryServiceError::storage("learning job id must be numeric".to_string())
    })?;
    Ok(MemoryLearningJob {
        job_id,
        space_id: row
            .space_id
            .and_then(|value| u64::try_from(value.max(0)).ok()),
        job_type: row.job_type.clone(),
        state: row.state.clone(),
        priority: row.priority,
        result: row
            .result_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        error: row
            .error_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        started_at: row.started_at.clone(),
        finished_at: row.finished_at.clone(),
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
        version: Some(u64::try_from(row.version.max(0)).unwrap_or(0)),
    })
}

use sdkwork_memory_contract::MemoryServiceResult;

#[cfg(test)]
mod eval_tests {
    use super::*;

    #[test]
    fn retrieval_quality_config_accepts_string_ids_and_rejects_empty_datasets() {
        let config: RetrievalQualityEvalConfig = serde_json::from_value(serde_json::json!({
            "cases": [{
                "spaceId": "7",
                "query": "preferred editor",
                "expectedMemoryIds": ["11", 12],
                "topK": 20
            }],
            "thresholds": {
                "minRecallAtK": 0.8,
                "minPrecisionAtK": 0.7,
                "minMeanNdcgAtK": 0.75,
                "maxP95LatencyMs": 500,
                "maxDegradedRate": 0.0
            }
        }))
        .expect("valid retrieval eval config");
        validate_retrieval_quality_config(&config).expect("config must validate");

        let empty: RetrievalQualityEvalConfig =
            serde_json::from_value(serde_json::json!({"cases": []}))
                .expect("empty config decodes before semantic validation");
        assert!(validate_retrieval_quality_config(&empty)
            .unwrap_err()
            .contains("between 1 and"));
    }

    #[test]
    fn retrieval_quality_scoring_deduplicates_hits_and_computes_rank_metrics() {
        let expected = [20_u64, 30_u64].into_iter().collect();
        let score = score_retrieval_case(&expected, &[10, 20, 20, 30], 4, false, 17);
        let expected_ndcg =
            (1.0 / 3_f64.log2() + 1.0 / 4_f64.log2()) / (1.0 / 2_f64.log2() + 1.0 / 3_f64.log2());

        assert_eq!(score.retrieved_count, 3);
        assert_eq!(score.retrieved_relevant_count, 2);
        assert_eq!(score.recall_at_k, 1.0);
        assert_eq!(score.precision_at_k, 2.0 / 3.0);
        assert!((score.ndcg_at_k - expected_ndcg).abs() < f64::EPSILON);
        assert_eq!(score.reciprocal_rank, 0.5);
        assert_eq!(score.latency_ms, 17);
        assert!(!score.degraded);
    }

    #[test]
    fn retrieval_quality_scoring_does_not_reward_duplicate_relevant_hits() {
        let expected = [20_u64, 30_u64].into_iter().collect();
        let score_with_duplicates = score_retrieval_case(&expected, &[20, 20, 30], 3, false, 1);
        let score_without_duplicates = score_retrieval_case(&expected, &[20, 30], 3, false, 1);

        assert_eq!(score_with_duplicates, score_without_duplicates);
        assert_eq!(score_with_duplicates.ndcg_at_k, 1.0);
    }

    #[test]
    fn retrieval_quality_scoring_handles_empty_results_without_fabricated_scores() {
        let expected = [20_u64].into_iter().collect();
        let score = score_retrieval_case(&expected, &[], 10, true, 9);

        assert_eq!(score.retrieved_count, 0);
        assert_eq!(score.retrieved_relevant_count, 0);
        assert_eq!(score.recall_at_k, 0.0);
        assert_eq!(score.precision_at_k, 0.0);
        assert_eq!(score.ndcg_at_k, 0.0);
        assert_eq!(score.reciprocal_rank, 0.0);
        assert!(score.degraded);
    }

    #[test]
    fn p95_latency_uses_nearest_rank() {
        assert_eq!(percentile_95_nearest_rank(&[]), 0);
        assert_eq!(percentile_95_nearest_rank(&[10]), 10);
        assert_eq!(percentile_95_nearest_rank(&[100, 1, 2, 3, 4]), 100);
    }

    #[test]
    fn retrieval_quality_config_rejects_unknown_fields_and_invalid_gates() {
        let unknown = serde_json::from_value::<RetrievalQualityEvalConfig>(serde_json::json!({
            "cases": [{
                "spaceId": "7",
                "query": "q",
                "expectedMemoryIds": ["11"],
                "pretendScore": 1.0
            }]
        }));
        assert!(unknown.is_err());

        let invalid_quality_gate: RetrievalQualityEvalConfig =
            serde_json::from_value(serde_json::json!({
                "cases": [{
                    "spaceId": "7",
                    "query": "q",
                    "expectedMemoryIds": ["11"]
                }],
                "thresholds": {"minPrecisionAtK": 1.1}
            }))
            .expect("gate decodes before semantic validation");
        assert!(validate_retrieval_quality_config(&invalid_quality_gate)
            .unwrap_err()
            .contains("from 0 to 1"));

        let invalid_latency_gate: RetrievalQualityEvalConfig =
            serde_json::from_value(serde_json::json!({
                "cases": [{
                    "spaceId": "7",
                    "query": "q",
                    "expectedMemoryIds": ["11"]
                }],
                "thresholds": {"maxP95LatencyMs": 0}
            }))
            .expect("latency gate decodes before semantic validation");
        assert!(validate_retrieval_quality_config(&invalid_latency_gate)
            .unwrap_err()
            .contains("greater than 0"));
    }

    #[tokio::test]
    async fn retrieval_quality_eval_executes_the_commercial_retrieval_path() {
        let store = NativeSqlMemoryStore::new_in_memory_sqlite()
            .await
            .expect("in-memory store");
        store
            .create_space_record(
                1,
                7,
                &sdkwork_memory_plugin_native_sql::NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "tenant".to_string(),
                    owner_subject_id: "1".to_string(),
                    space_type: "workspace".to_string(),
                    display_name: "Eval Space".to_string(),
                    default_scope: "tenant".to_string(),
                },
            )
            .await
            .expect("eval space");
        let scope = MemoryScopeContext {
            tenant_id: 1,
            space_id: 7,
            organization_id: None,
            user_id: None,
        };
        store
            .create_record_open_api(
                &scope,
                "11",
                "tenant",
                "semantic",
                Some("editor"),
                Some("preference"),
                "prefers modal editor keybindings",
                "editor preference is modal keybindings",
                "internal",
                None,
            )
            .await
            .expect("expected eval memory");
        store
            .create_record_open_api(
                &scope,
                "12",
                "tenant",
                "semantic",
                Some("theme"),
                Some("preference"),
                "prefers a light color theme",
                "theme preference is light",
                "internal",
                None,
            )
            .await
            .expect("distractor eval memory");
        let service = OpenMemoryService::new(store);
        let row = NativeSqlEvalRunRow {
            eval_run_uuid: "501".to_string(),
            eval_type: "retrieval_quality".to_string(),
            state: "running".to_string(),
            dataset_ref: Some("inline-golden-v1".to_string()),
            profile_ref: None,
            metrics_json: None,
            result_json: Some(
                serde_json::json!({
                    "cases": [{
                        "spaceId": "7",
                        "query": "modal editor",
                        "expectedMemoryIds": ["11"],
                        "topK": 1
                    }],
                    "thresholds": {
                        "minRecallAtK": 1.0,
                        "minPrecisionAtK": 1.0,
                        "minMeanNdcgAtK": 1.0,
                        "minMeanReciprocalRank": 1.0,
                        "maxP95LatencyMs": 600000,
                        "maxDegradedRate": 0.0
                    }
                })
                .to_string(),
            ),
            started_at: Some(platform::current_timestamp()),
            finished_at: None,
            created_at: platform::current_timestamp(),
            updated_at: platform::current_timestamp(),
        };

        let output = run_retrieval_quality_eval(&service, 1, &row)
            .await
            .expect("real retrieval eval");
        let metrics: Value = serde_json::from_str(&output.metrics).expect("metrics json");
        let result: Value = serde_json::from_str(&output.result).expect("result json");
        assert_eq!(metrics["recallAtK"], 1.0);
        assert_eq!(metrics["precisionAtK"], 1.0);
        assert_eq!(metrics["meanNdcgAtK"], 1.0);
        assert_eq!(metrics["meanReciprocalRank"], 1.0);
        assert!(metrics["p95LatencyMs"].as_u64().is_some());
        assert_eq!(metrics["qualityGatePassed"], true);
        assert_eq!(result["status"], "completed");
        assert_eq!(result["cases"][0]["retrievedCount"], 1);
        assert_eq!(result["cases"][0]["precisionAtK"], 1.0);
        assert_eq!(result["cases"][0]["ndcgAtK"], 1.0);
        assert!(result["cases"][0]["latencyMs"].as_u64().is_some());
        assert!(result["cases"][0]["queryHash"]
            .as_str()
            .is_some_and(|value| !value.is_empty() && value != "modal editor"));
        assert!(result["cases"][0].get("query").is_none());
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::MemoryBackgroundWorkers;
    use std::time::{Duration, Instant};
    use tokio::sync::watch;

    /// Builds a drain handle over synthetic workers so the bounded-drain
    /// contract is verified without booting a database-backed product.
    fn handle_over(join_handles: Vec<tokio::task::JoinHandle<()>>) -> MemoryBackgroundWorkers {
        let (shutdown_tx, _rx) = watch::channel(false);
        MemoryBackgroundWorkers {
            shutdown_tx,
            join_handles,
        }
    }

    /// A worker that observes its shutdown receiver and exits.
    fn watchful_worker(mut rx: watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if *rx.borrow() {
                    return;
                }
            }
        })
    }

    #[tokio::test]
    async fn shutdown_stops_watchful_workers_and_drain_reports_completion() {
        let (shutdown_tx, rx) = watch::channel(false);
        let mut workers = MemoryBackgroundWorkers {
            shutdown_tx,
            join_handles: vec![watchful_worker(rx.clone()), watchful_worker(rx)],
        };

        workers.shutdown();
        assert!(
            workers.drain(Duration::from_secs(5)).await,
            "drain must report completion once every worker observed the stop trigger"
        );
    }

    #[tokio::test]
    async fn drain_stays_bounded_when_a_worker_overruns_its_budget() {
        let overrunning = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        let mut workers = handle_over(vec![overrunning]);

        let started = Instant::now();
        let drained = workers.drain(Duration::from_millis(20)).await;
        let elapsed = started.elapsed();

        assert!(
            !drained,
            "an overrunning worker must be reported as not drained"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "drain must return on the budget instead of waiting for the worker, elapsed={elapsed:?}"
        );
    }

    #[tokio::test]
    async fn drain_budget_is_enforced_across_the_whole_set_not_per_worker() {
        // Three workers that each overrun a 60ms budget. A per-worker budget
        // would take ~180ms; a set-wide deadline must stay near 60ms.
        let mut handles = Vec::new();
        for _ in 0..3 {
            handles.push(tokio::spawn(async {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }));
        }
        let mut workers = handle_over(handles);

        let started = Instant::now();
        let drained = workers.drain(Duration::from_millis(60)).await;
        let elapsed = started.elapsed();

        assert!(!drained);
        assert!(
            elapsed < Duration::from_millis(150),
            "the budget must be a set-wide deadline, elapsed={elapsed:?}"
        );
    }

    #[tokio::test]
    async fn drain_with_a_zero_budget_never_panics() {
        let worker = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        let mut workers = handle_over(vec![worker]);

        assert!(!workers.drain(Duration::ZERO).await);
    }

    #[tokio::test]
    async fn drain_with_no_workers_is_trivially_complete() {
        let mut workers = handle_over(Vec::new());
        assert!(workers.drain(Duration::from_secs(1)).await);
    }
}
