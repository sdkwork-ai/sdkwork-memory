# RUNBOOK: Token and Key Rotation

Status: active  
Owner: SDKWork Memory operators  
Application: sdkwork-memory  
Specs: SECURITY_SPEC.md, DOCUMENTATION_SPEC.md

## Scope

Rotate IAM database credentials, memory database URLs, API keys, and container secrets for SDKWork Memory.

## Signals

- Credential expiry alerts from secret manager
- Auth failure spike (`memory_authz_denied_total`, HTTP 401/403)
- Failed login or API key validation in IAM logs

## Commands

```powershell
# Verify service health after rotation
curl -s http://localhost:8080/readyz
curl -s http://localhost:8080/metrics | Select-String memory_health_status

# Run database migration job (Kubernetes)
kubectl apply -f deployments/kubernetes/migration-job.yaml
kubectl wait --for=condition=complete job/sdkwork-memory-db-migrate --timeout=300s
```

## Procedure

1. Rotate secrets in the platform secret store (`sdkwork-memory-database`, plus `sdkwork-memory-drive` when Drive-backed exports are enabled).
2. Rolling restart API server deployment (`deployments/kubernetes/deployment.yaml`).
3. Confirm `/readyz` returns 200 and `memory_health_status` gauge is `1`.
4. Smoke test app API with dual-token auth and open API with API key.

## Rollback

Restore previous secret version and redeploy prior container digest per `deployments/runbooks/rollout.md`.

## Escalation

Platform security on-call → Memory service owner → SDKWork IAM team.
