use std::sync::Arc;

use sdkwork_drive_storage_contract::{DriveObjectStoreError, DriveStorageProviderKind};
use sdkwork_drive_storage_local::LocalDriveObjectStore;
use sdkwork_drive_storage_s3::{S3DriveObjectStore, S3StoreConfig};
use sdkwork_utils_rust::is_blank;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct MemoryDriveStorageProvider {
    pub provider_kind: String,
    pub endpoint_url: String,
    pub region: Option<String>,
    pub bucket: String,
    pub path_style: bool,
    pub strict_tls: bool,
    pub credential_ref: Option<String>,
}

pub enum MemoryDriveObjectStore {
    Local(Arc<LocalDriveObjectStore>),
    S3(Arc<S3DriveObjectStore>),
}

pub async fn load_memory_drive_storage_provider(
    pool: &PgPool,
    provider_id: &str,
) -> Result<MemoryDriveStorageProvider, String> {
    let row = sqlx::query(
        "SELECT provider_kind, endpoint_url, region, bucket, path_style, strict_tls, credential_ref
         FROM dr_drive_storage_provider WHERE id = $1",
    )
    .bind(provider_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| format!("read memory drive storage provider failed: {error}"))?
    .ok_or_else(|| format!("memory drive storage provider not found: {provider_id}"))?;

    Ok(MemoryDriveStorageProvider {
        provider_kind: row.get("provider_kind"),
        endpoint_url: row.get("endpoint_url"),
        region: row.get("region"),
        bucket: row.get("bucket"),
        path_style: row.get("path_style"),
        strict_tls: row.get("strict_tls"),
        credential_ref: row.get("credential_ref"),
    })
}

pub async fn build_memory_drive_object_store(
    provider: &MemoryDriveStorageProvider,
    local_root: &str,
) -> Result<MemoryDriveObjectStore, String> {
    let provider_kind = DriveStorageProviderKind::try_from_str(provider.provider_kind.as_str())
        .ok_or_else(|| {
            format!(
                "unsupported memory drive storage provider kind: {}",
                provider.provider_kind
            )
        })?;

    match provider_kind {
        DriveStorageProviderKind::LocalFilesystem => {
            if is_blank(Some(local_root)) {
                return Err(
                    "SDKWORK_MEMORY_DRIVE_OBJECT_STORE_ROOT is required for local filesystem drive export"
                        .to_string(),
                );
            }
            Ok(MemoryDriveObjectStore::Local(Arc::new(
                LocalDriveObjectStore::new(local_root),
            )))
        }
        kind if provider_supports_s3_object_store(&kind) => {
            let store = S3DriveObjectStore::new(
                S3StoreConfig::from_provider_parts(
                    provider.provider_kind.as_str(),
                    provider.endpoint_url.as_str(),
                    provider.region.as_deref(),
                    provider.bucket.as_str(),
                    provider.path_style,
                    provider.credential_ref.as_deref(),
                    Some(provider.strict_tls),
                )
                .map_err(map_object_store_config_error)?,
            )
            .await
            .map_err(map_object_store_config_error)?;
            Ok(MemoryDriveObjectStore::S3(Arc::new(store)))
        }
        other => Err(format!(
            "memory drive export does not support storage provider kind: {}",
            other.as_str()
        )),
    }
}

fn provider_supports_s3_object_store(provider_kind: &DriveStorageProviderKind) -> bool {
    matches!(
        provider_kind,
        DriveStorageProviderKind::S3Compatible
            | DriveStorageProviderKind::AliyunOss
            | DriveStorageProviderKind::TencentCos
            | DriveStorageProviderKind::HuaweiObs
            | DriveStorageProviderKind::VolcengineTos
            | DriveStorageProviderKind::GoogleCloudStorage
            | DriveStorageProviderKind::Custom(_)
    )
}

fn map_object_store_config_error(error: DriveObjectStoreError) -> String {
    error.message
}
