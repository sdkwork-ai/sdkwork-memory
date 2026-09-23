use async_trait::async_trait;
use sdkwork_drive_uploader_service::service::{
    DriveUploaderService, PrepareUploaderUploadCommand, SqlUploaderStore, UploadBytesCommand,
    UploaderActor, UploaderRetention, UploaderTarget,
};
use sdkwork_drive_workspace_service::DriveServiceError;
use sdkwork_memory_spi::{
    MemoryDriveExportUploadRequest, MemoryDriveExportUploadResult, MemoryDriveExportUploader,
    MemorySpiError, MemorySpiResult,
};
use sdkwork_utils_rust::{now, sha256_hash, to_unix_millis};
use sqlx::PgPool;

use crate::object_store::MemoryDriveObjectStore;

pub struct DriveUploaderMemoryExportAdapter {
    uploader: DriveUploaderService<SqlUploaderStore>,
    object_store: MemoryDriveObjectStore,
    app_id: String,
}

impl DriveUploaderMemoryExportAdapter {
    pub fn new(pool: PgPool, object_store: MemoryDriveObjectStore) -> Self {
        Self {
            uploader: DriveUploaderService::new(SqlUploaderStore::new(pool)),
            object_store,
            app_id: "sdkwork-memory".to_string(),
        }
    }
}

#[async_trait]
impl MemoryDriveExportUploader for DriveUploaderMemoryExportAdapter {
    async fn upload_export(
        &self,
        mut request: MemoryDriveExportUploadRequest,
    ) -> MemorySpiResult<MemoryDriveExportUploadResult> {
        if request.user_id.is_none() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryDriveExportUploader".to_string(),
                message: "drive export requires actor user_id".to_string(),
            });
        }

        let now_epoch_ms = to_unix_millis(now());
        let checksum = sha256_hash(&request.body);
        let body = std::mem::take(&mut request.body);
        let command = upload_command(&request, body, now_epoch_ms, &self.app_id);

        let completed = match &self.object_store {
            MemoryDriveObjectStore::Local(store) => {
                self.uploader.upload_bytes(store.as_ref(), command).await
            }
            MemoryDriveObjectStore::S3(store) => {
                self.uploader.upload_bytes(store.as_ref(), command).await
            }
        }
        .map_err(map_drive_service_error)?;

        Ok(MemoryDriveExportUploadResult {
            drive_object_ref: format!("drive://nodes/{}", completed.node_id),
            drive_node_id: completed.node_id,
            checksum_sha256_hex: format!("sha256:{checksum}"),
        })
    }
}

fn upload_command(
    request: &MemoryDriveExportUploadRequest,
    body: Vec<u8>,
    now_epoch_ms: i64,
    app_id: &str,
) -> UploadBytesCommand {
    let tenant_id = request.tenant_id.to_string();
    let user_id = request.user_id.expect("validated above").to_string();
    let export_job_id = request.export_job_id.to_string();
    let file_fingerprint = format!(
        "memory-export:{}:{}:{}",
        tenant_id, export_job_id, request.format
    );
    UploadBytesCommand {
        prepare: PrepareUploaderUploadCommand {
            id: format!("memory-export-item-{export_job_id}"),
            task_id: format!("memory-export-task-{export_job_id}"),
            tenant_id,
            organization_id: request.organization_id.map(|value| value.to_string()),
            actor: UploaderActor::User {
                user_id: user_id.clone(),
            },
            app_id: app_id.to_string(),
            app_resource_type: "memory_export".to_string(),
            app_resource_id: export_job_id,
            scene: Some("memory_export".to_string()),
            source: Some(request.drive_target_ref.clone()),
            upload_profile_code: export_upload_profile_code(&request.format),
            file_fingerprint,
            original_file_name: request.original_file_name.clone(),
            content_type: request.content_type.clone(),
            content_length: request.body.len() as i64,
            chunk_size_bytes: 8 * 1024 * 1024,
            target: UploaderTarget::AutoUploadSpace {
                parent_node_id: None,
            },
            retention: UploaderRetention::LongTerm,
            operator_id: user_id,
            now_epoch_ms,
        },
        body,
        uploaded_at_epoch_ms: now_epoch_ms,
    }
}

fn export_upload_profile_code(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "json" | "ndjson" => "document".to_string(),
        _ => "document".to_string(),
    }
}

fn map_drive_service_error(error: DriveServiceError) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: "MemoryDriveExportUploader".to_string(),
        message: format!("{error:?}"),
    }
}
