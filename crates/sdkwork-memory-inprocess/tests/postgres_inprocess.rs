//! Optional PostgreSQL smoke test for the in-process memory runtime.
//!
//! Runs only when `SDKWORK_MEMORY_DATABASE_URL` or `SDKWORK_DATABASE_URL`
//! points at a reachable PostgreSQL database. The test bootstraps the memory
//! data plane (applying memory migrations), creates a personal space, and runs
//! one retrieval through the same service method the App API uses.

use sdkwork_memory_inprocess::InProcessMemoryRuntime;

#[tokio::test]
async fn postgres_inprocess_runtime_bootstraps_creates_space_and_retrieves() {
    let Some(database_url) = std::env::var("SDKWORK_DATABASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        eprintln!("skipping in-process memory Postgres test: set SDKWORK_DATABASE_URL");
        return;
    };
    std::env::set_var("SDKWORK_DATABASE_URL", &database_url);

    let runtime = InProcessMemoryRuntime::from_env()
        .await
        .expect("bootstrap in-process memory runtime")
        .expect("memory runtime must be configured");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let space = runtime
        .create_space(100_001, Some(7), format!("in-process smoke {nonce}"))
        .await
        .expect("create memory space");
    let result = runtime
        .retrieve(
            100_001,
            Some(7),
            space.space_id,
            "refund preferences",
            Some(3),
            Some(256),
        )
        .await
        .expect("retrieve memory");
    assert!(
        result.hits.is_empty(),
        "a fresh space must not return memory hits"
    );
}
