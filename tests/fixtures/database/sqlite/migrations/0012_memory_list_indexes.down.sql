-- Reverse 0012_memory_list_indexes for sqlite.
DROP INDEX IF EXISTS idx_ai_outbox_claim_order;
DROP INDEX IF EXISTS idx_ai_learning_job_history;
DROP INDEX IF EXISTS idx_ai_audit_resource_id;
DROP INDEX IF EXISTS idx_ai_retrieval_trace_space_created;
DROP INDEX IF EXISTS idx_ai_record_space_uuid;
