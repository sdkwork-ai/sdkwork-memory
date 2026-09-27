-- Reverse of 0014_claim_order_and_keyset_indexes: drop the claim-order and
-- keyset-list indexes added for the worker and list hot paths.

DROP INDEX IF EXISTS idx_ai_learning_job_claim_order;
DROP INDEX IF EXISTS idx_ai_eval_run_claim_order;
DROP INDEX IF EXISTS idx_ai_retrieval_trace_tenant_id;
DROP INDEX IF EXISTS idx_ai_retrieval_trace_space_id;
DROP INDEX IF EXISTS idx_ai_event_space_uuid;
DROP INDEX IF EXISTS idx_ai_candidate_space_uuid;
DROP INDEX IF EXISTS idx_ai_record_space_user;
