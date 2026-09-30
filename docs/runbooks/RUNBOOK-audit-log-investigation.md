# RUNBOOK: Audit Log Investigation

Status: active  
Owner: SDKWork Memory operators  
Specs: OBSERVABILITY_SPEC.md, PRIVACY_SPEC.md

## Scope

Investigate memory governance actions (forget, export, feedback, backend admin changes).

## Data sources

- `ai_audit_log` table
- Backend API audit list endpoints
- Domain outbox events (`ai_outbox_event`)

## Procedure

1. Collect tenant id, time window, and request id from customer report.
2. Query audit logs filtered by tenant and action type (`feedback.create`, `forget.request.start` for an interrupted run's running row, `forget.request.create` for its terminal row, etc.)..
3. Correlate with retrieval traces when feedback targets `retrieval` resources.
4. Preserve evidence; do not export raw payloads containing PII to unsecured channels.

## Commands

```powershell
cargo test -p sdkwork-memory-integration-tests governance
```

## Escalation

Memory service owner → privacy/legal for regulated data requests.
