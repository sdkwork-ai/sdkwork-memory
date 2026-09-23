-- Reverse 0007_memory_commercial_management for sqlite.
-- Drops tables in reverse dependency order to satisfy foreign key constraints
-- when PRAGMA foreign_keys = ON.

DROP TABLE IF EXISTS ai_commercial_readiness_snapshot;
DROP TABLE IF EXISTS ai_relation_rebuild_job;
DROP TABLE IF EXISTS ai_policy_assignment;
DROP TABLE IF EXISTS ai_capability_binding;
DROP TABLE IF EXISTS ai_memory_binding;
DROP TABLE IF EXISTS ai_subject;
DROP TABLE IF EXISTS ai_policy;
DROP TABLE IF EXISTS ai_edge;
DROP TABLE IF EXISTS ai_entity;
