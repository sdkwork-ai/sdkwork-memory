use sdkwork_api_memory_assembly::{
    assemble_api_router_from_env, run_database_migrate_only,
};
use sdkwork_api_memory_standalone_gateway::init_tracing;
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

    if matches!(std::env::args().nth(1).as_deref(), Some("db-migrate")) {
        if let Err(error) = run_database_migrate_only().await {
            exit_with_error("db-migrate", error);
        }
        return;
    }

    let bind_address = std::env::var("SDKWORK_MEMORY_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned());

    // The assembly owns service construction, route composition, readiness,
    // and background workers; the listener projects `.router` and keeps the
    // worker shutdown handle for graceful drain (API_ASSEMBLY_SPEC §6.1).
    let app = match assemble_api_router_from_env().await {
        Ok(app) => app,
        Err(error) => exit_with_error("bootstrap", error),
    };

    let listener = match tokio::net::TcpListener::bind(&bind_address).await {
        Ok(listener) => listener,
        Err(error) => exit_with_error("bind", format!("{bind_address}: {error}")),
    };
    tracing::info!("sdkwork-api-memory-standalone-gateway listening on {bind_address}");

    let worker_shutdown_tx = app.worker_shutdown_tx;
    if let Err(error) = axum::serve(listener, app.router)
        .with_graceful_shutdown(shutdown_signal(worker_shutdown_tx))
        .await
    {
        exit_with_error("serve", error);
    }
}

async fn shutdown_signal(worker_shutdown_tx: tokio::sync::watch::Sender<bool>) {
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

    tracing::info!("sdkwork-api-memory-standalone-gateway shutdown signal received");

    // Trigger graceful shutdown of all background workers.
    let _ = worker_shutdown_tx.send(true);

    // Give workers a bounded grace period to drain in-flight work.
    tokio::time::sleep(Duration::from_secs(3)).await;
    tracing::info!("sdkwork-api-memory-standalone-gateway background workers shutdown complete");
}
