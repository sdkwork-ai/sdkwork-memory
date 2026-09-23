use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_plugin_native_sql::NativeSqlMemoryStore;
use sdkwork_memory_spi::{
    MemoryDriveExportUploadRequest, MemoryDriveExportUploadResult, MemoryDriveExportUploader,
    MemorySpiResult,
};
use sdkwork_utils_rust::sha256_hash;

#[derive(Debug, Default)]
pub struct RecordingMemoryDriveExportUploader {
    pub uploads: std::sync::Mutex<Vec<MemoryDriveExportUploadRequest>>,
}

#[async_trait]
impl MemoryDriveExportUploader for RecordingMemoryDriveExportUploader {
    async fn upload_export(
        &self,
        request: MemoryDriveExportUploadRequest,
    ) -> MemorySpiResult<MemoryDriveExportUploadResult> {
        let checksum = sha256_hash(&request.body);
        let drive_node_id = format!("memory-export-{}", request.export_job_id);
        let drive_object_ref = format!("drive://nodes/{drive_node_id}");
        let result = MemoryDriveExportUploadResult {
            drive_object_ref: drive_object_ref.clone(),
            drive_node_id: drive_node_id.clone(),
            checksum_sha256_hex: format!("sha256:{checksum}"),
        };
        self.uploads
            .lock()
            .expect("recording drive export uploader lock")
            .push(request);
        Ok(result)
    }
}

pub fn open_memory_service_with_drive(store: NativeSqlMemoryStore) -> OpenMemoryService {
    OpenMemoryService::new(store)
        .with_drive_export_uploader(Arc::new(RecordingMemoryDriveExportUploader::default()))
}
