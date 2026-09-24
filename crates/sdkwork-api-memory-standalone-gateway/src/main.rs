use sdkwork_api_memory_assembly::{
    assemble_api_router_retaining_background_from_env, run_database_migrate_only,
};
use sdkwork_api_memory_standalone_gateway::init_tracing;
use sdkwork_web_bootstrap::ApiModuleRegistry;
use std::process;
use tokio::signal;
use tokio::time::Duration;

fn exit_with_error(context: &str, message: impl std::fmt::Display) -> ! {
    tracing::error!(context, error = %message, "fatal startup failure");
    eprintln!("FATAL [{context}]: {message}");
    process::exit(1);
}

#[tokio::main]
async fn main() {
    init_tracing();

    // A Memory server process must declare which environment it serves. The
    // resolver otherwise falls back to `development`, which silently disables
    // authorization hardening, tenant isolation policy, Redis-backed rate
    // limiting, and request deadlines (see
    // `require_explicit_memory_environment` for the incident this closes).
    if let Err(error) = sdkwork_memory_contract::require_explicit_memory_environment() {
        exit_with_error("environment", error);
    }    // SDKWork Process-Shared Database Pool Standard section 4: the process entrypoint enables
    // strict process-local pool reuse before the first pool creation, so every module embedded in
    // this process (Memory repositories, the database host lifecycle, and the Drive compatibility
    // adapter) reuses one process pool for one normalized database identity instead of opening a
    // private pool each. The call must precede any `*_from_env()` bootstrap.
    sdkwork_database_sqlx::enable_process_shared_database_pool();

    if matches!(std::env::args().nth(1).as_deref(), Some("db-migrate")) {
        if let Err(error) = run_database_migrate_only().await {
            exit_with_error("db-migrate", error);
        }
        return;
    }

    let bind_address = std::env::var("SDKWORK_MEMORY_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned());

    // The assembly returns one host-neutral contribution that already owns the complete route
    // surface, the owner's route manifest, the process endpoints, and the per-surface Web
    // Framework layers (app-api, backend-api, and open-api select distinct auth profiles).
    // The host therefore composes it as a module and serves `ComposedApiAssembly::router`
    // without applying a second framework layer (API_ASSEMBLY_SPEC §6.1).
    let (assembly, mut background_workers) =
        match assemble_api_router_retaining_background_from_env().await {
            Ok(assembly) => assembly,
            Err(error) => exit_with_error("bootstrap", error),
        };

    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_modules(vec![assembly]);
    let app = module_registry
        .try_compose("SDKWork Memory API")
        .unwrap_or_else(|error| panic!("SDKWork Memory API module composition failed: {error}"));

    let listener = match tokio::net::TcpListener::bind(&bind_address).await {
        Ok(listener) => listener,
        Err(error) => exit_with_error("bind", format!("{bind_address}: {error}")),
    };
    tracing::info!("sdkwork-api-memory-standalone-gateway listening on {bind_address}");

    // Shutdown order matters. `axum::serve(..).await` returns only after the
    // listener stopped accepting and every in-flight request finished, so the
    // HTTP plane is fully drained before the background plane starts draining.
    // Running the two drains in sequence (rather than concurrently with the
    // background plane going first) keeps the worst-case shutdown time the sum
    // of two known budgets instead of an open-ended overlap, and it lets
    // workers keep completing batches while requests are still being served.
    let serve_result = axum::serve(listener, app.router)
        .with_graceful_shutdown(wait_for_shutdown_signal())
        .await;

    // The HTTP plane is drained; now stop admitting background work and wait
    // for real completion under a bounded budget.
    let drain_budget = background_drain_budget();
    background_workers.shutdown();
    if background_workers.drain(drain_budget).await {
        tracing::info!(
            drain_budget_seconds = drain_budget.as_secs(),
            "sdkwork-api-memory-standalone-gateway background workers drained"
        );
    } else {
        tracing::warn!(
            drain_budget_seconds = drain_budget.as_secs(),
            "sdkwork-api-memory-standalone-gateway drain budget expired with workers still running; \
             abandoned work keeps its database lease and becomes re-claimable by another replica \
             after the lease expires"
        );
    }

    if let Err(error) = serve_result {
        exit_with_error("serve", error);
    }
}

/// Budget for draining the Memory background plane on shutdown.
///
/// Must stay below the orchestrator's `terminationGracePeriodSeconds` so the
/// process reports an incomplete drain itself instead of being killed
/// mid-write. Work that outlives the budget is not lost: every job and outbox
/// row carries a database lease (`lease_owner` / `lease_token`), so the next
/// replica re-claims it once the lease expires. That is why a bounded wait is
/// safe here and an unbounded one is not necessary.
// The drain budget must cover a typical in-flight worker batch (learning
// extraction calls external LLM providers; outbox delivery has its own
// timeout). 25 s aborted nearly every batch mid-flight on rollout, leaving
// rows running until lease expiry (15 minutes) — a per-deployment processing
// hole on every restart. 4 minutes covers the documented provider timeouts;
// SIGTERM handlers still get the k8s grace period (60 s default) plus this
// budget only if the deployment raises terminationGracePeriodSeconds to match
// (see deployments/kubernetes/deployment.yaml).
const DEFAULT_BACKGROUND_DRAIN_SECONDS: u64 = 240;

/// Resolves the shutdown drain budget from
/// `SDKWORK_MEMORY_SHUTDOWN_DRAIN_SECS`, falling back to
/// [`DEFAULT_BACKGROUND_DRAIN_SECONDS`].
fn background_drain_budget() -> Duration {
    let seconds = std::env::var("SDKWORK_MEMORY_SHUTDOWN_DRAIN_SECS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_BACKGROUND_DRAIN_SECONDS);
    Duration::from_secs(seconds)
}

/// Resolves on SIGINT / SIGTERM.
///
/// Deliberately takes no worker handle: `axum::serve(..).with_graceful_shutdown`
/// requires a `'static` future, and the background plane must survive until the
/// HTTP plane has finished draining. The host therefore keeps the worker handle
/// in `main` and drains it after `serve(..).await` returns.
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::warn!(%error, "failed to install Ctrl+C handler; ignoring");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to install SIGTERM handler; ignoring");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!(
        "sdkwork-api-memory-standalone-gateway shutdown signal received; \
         draining in-flight requests"
    );
}
