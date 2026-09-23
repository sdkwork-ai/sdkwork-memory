use std::sync::Arc;

use sdkwork_database_config::workspace_database::{
    normalize_workspace_postgres_url, reject_retired_database_env, resolve_workspace_database_url,
    workspace_postgres_env_is_configured,
};
use sdkwork_database_config::DatabaseConfig;
use sdkwork_drive_config::DatabaseConfig as DriveDatabaseConfig;
use sdkwork_drive_workspace_service::infrastructure::sql::connect_postgres_database_and_install_schema;
use sdkwork_memory_spi::MemoryDriveExportUploader;
use sdkwork_utils_rust::is_blank;
use sqlx::PgPool;

use crate::object_store::{build_memory_drive_object_store, load_memory_drive_storage_provider};
use crate::uploader::DriveUploaderMemoryExportAdapter;

const DEFAULT_MEMORY_DRIVE_PROVIDER_ID: &str = "sdkwork-memory-local";
const DEFAULT_MEMORY_DRIVE_BUCKET: &str = "memory";
const SEEDED_BY: &str = "sdkwork-memory";

/// Build the Drive-backed export uploader when a workspace PostgreSQL profile is configured.
///
/// Drive is a PostgreSQL-only module, so this integration is PostgreSQL-only as well: the
/// SQLite client-local profile keeps exports in-process and simply reports `None` here. The
/// connection is taken from the installed process pool
/// (SDKWork Process-Shared Database Pool Standard sections 1 and 2) instead of opening a private
/// pool, so one OS process still owns exactly one pool per normalized database identity.
pub async fn bootstrap_memory_drive_export_uploader_from_env(
) -> Result<Option<Arc<dyn MemoryDriveExportUploader>>, String> {
    reject_retired_database_env().map_err(|error| error.to_string())?;
    if !workspace_postgres_env_is_configured() {
        return Ok(None);
    }
    let database_url = resolve_workspace_database_url().map_err(|error| error.to_string())?;

    let object_store_root = std::env::var("SDKWORK_MEMORY_DRIVE_OBJECT_STORE_ROOT")
        .or_else(|_| std::env::var("SDKWORK_DRIVE_OBJECT_STORE_ROOT"))
        .unwrap_or_default();

    let pool = connect_memory_drive_pool(&database_url).await?;
    let provider =
        load_memory_drive_storage_provider(&pool, DEFAULT_MEMORY_DRIVE_PROVIDER_ID).await?;
    let object_store =
        build_memory_drive_object_store(&provider, object_store_root.as_str()).await?;
    Ok(Some(Arc::new(DriveUploaderMemoryExportAdapter::new(
        pool,
        object_store,
    ))))
}

/// Install the Drive core schema and return the process-shared PostgreSQL handle.
async fn connect_memory_drive_pool(database_url: &str) -> Result<PgPool, String> {
    let normalized =
        normalize_workspace_postgres_url(database_url).map_err(|error| error.to_string())?;
    // Forward the canonical process budget so Drive cannot introduce a second, differently sized
    // capacity claim; the framework still owns the effective allocation (section 5).
    let process_budget = DatabaseConfig::from_env("MEMORY")
        .map_err(|error| error.to_string())?
        .max_connections;
    let drive_config =
        DriveDatabaseConfig::from_url_with_max_connections(normalized.as_str(), process_budget)
            .map_err(|error| error.to_string())?;

    let pool = connect_postgres_database_and_install_schema(&drive_config)
        .await
        .map_err(|error| format!("connect memory drive database failed: {error}"))?;
    seed_default_drive_storage_provider(&pool).await?;
    Ok(pool)
}

/// Ensure the system-owned Memory export storage provider exists.
///
/// The row is system-scoped: `tenant_id` keeps the schema default instead of claiming a business
/// tenant, and `created_by`/`updated_by` record the seeding application.
async fn seed_default_drive_storage_provider(pool: &PgPool) -> Result<(), String> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM dr_drive_storage_provider WHERE id = $1")
            .bind(DEFAULT_MEMORY_DRIVE_PROVIDER_ID)
            .fetch_optional(pool)
            .await
            .map_err(|error| format!("read memory drive storage provider failed: {error}"))?;
    if exists.is_some() {
        return Ok(());
    }

    if let Some(s3_endpoint) = std::env::var("SDKWORK_MEMORY_DRIVE_S3_ENDPOINT")
        .ok()
        .filter(|value| !is_blank(Some(value.as_str())))
    {
        return seed_s3_drive_storage_provider(pool, s3_endpoint.as_str()).await;
    }

    sqlx::query(
        "INSERT INTO dr_drive_storage_provider (
            id, provider_kind, name, endpoint_url, region, bucket, path_style,
            strict_tls, credential_ref, server_side_encryption_mode, default_storage_class,
            status, version, created_by, updated_by
        ) VALUES (
            $1, 'local_filesystem', $2, 'file://localhost', 'local', $2, TRUE, TRUE,
            'plain:local:local', NULL, NULL, 'active', 1, $3, $3
        )",
    )
    .bind(DEFAULT_MEMORY_DRIVE_PROVIDER_ID)
    .bind(DEFAULT_MEMORY_DRIVE_BUCKET)
    .bind(SEEDED_BY)
    .execute(pool)
    .await
    .map_err(|error| format!("seed memory drive storage provider failed: {error}"))?;
    Ok(())
}

async fn seed_s3_drive_storage_provider(pool: &PgPool, endpoint: &str) -> Result<(), String> {
    let region = std::env::var("SDKWORK_MEMORY_DRIVE_S3_REGION")
        .or_else(|_| std::env::var("SDKWORK_DRIVE_S3_REGION"))
        .unwrap_or_else(|_| "us-east-1".to_string());
    let bucket = std::env::var("SDKWORK_MEMORY_DRIVE_S3_BUCKET")
        .unwrap_or_else(|_| DEFAULT_MEMORY_DRIVE_BUCKET.to_string());
    let credential_ref = std::env::var("SDKWORK_MEMORY_DRIVE_S3_CREDENTIAL_REF")
        .or_else(|_| std::env::var("SDKWORK_DRIVE_S3_CREDENTIAL_REF"))
        .ok()
        .filter(|value| !is_blank(Some(value.as_str())));
    let path_style = std::env::var("SDKWORK_MEMORY_DRIVE_S3_PATH_STYLE")
        .ok()
        .map(|value| env_flag_enabled(value.as_str()))
        .unwrap_or(true);
    let strict_tls = std::env::var("SDKWORK_MEMORY_DRIVE_S3_STRICT_TLS")
        .ok()
        .map(|value| env_flag_enabled(value.as_str()))
        .unwrap_or(!endpoint.to_ascii_lowercase().starts_with("http://"));
    let credential_ref = credential_ref.unwrap_or_else(|| "env:sdkwork-drive-s3".to_string());

    sqlx::query(
        "INSERT INTO dr_drive_storage_provider (
            id, provider_kind, name, endpoint_url, region, bucket, path_style,
            strict_tls, credential_ref, server_side_encryption_mode, default_storage_class,
            status, version, created_by, updated_by
        ) VALUES (
            $1, 's3_compatible', 'Memory Export S3', $2, $3, $4, $5, $6,
            $7, NULL, NULL, 'active', 1, $8, $8
        )",
    )
    .bind(DEFAULT_MEMORY_DRIVE_PROVIDER_ID)
    .bind(endpoint)
    .bind(region)
    .bind(bucket)
    .bind(path_style)
    .bind(strict_tls)
    .bind(credential_ref)
    .bind(SEEDED_BY)
    .execute(pool)
    .await
    .map_err(|error| format!("seed memory drive s3 storage provider failed: {error}"))?;
    Ok(())
}

fn env_flag_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}
