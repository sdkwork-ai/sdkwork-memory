# SDKWork Memory Rollout Runbook

## Preconditions

- Container image digest is pinned in the release manifest.
- `SDKWORK_MEMORY_ENVIRONMENT=production` and `SDKWORK_MEMORY_DEPLOYMENT_PROFILE=cloud`.
- One unified workspace database secret exists in the target namespace. The Memory service, the embedded IAM readiness/audit plane, and the Drive-backed export adapter all resolve their PostgreSQL identity from the same `SDKWORK_DATABASE_URL`, so no per-module database secret is required.
- For Drive-backed exports: use the same `SDKWORK_DATABASE_URL` and configure either `SDKWORK_MEMORY_DRIVE_OBJECT_STORE_ROOT` (local) or `SDKWORK_MEMORY_DRIVE_S3_*` credentials (production S3-compatible storage).
- Database migrations completed via release Job (`deployments/kubernetes/migration-job.yaml`) or `sdkwork-api-memory-standalone-gateway db-migrate`.

## Rollout

1. Apply migration Job and wait for completion before Deployment rollout.
2. Apply Kubernetes manifests: Service, Deployment, PDB, HPA, Ingress.
3. Verify `/healthz` returns `ok` on each pod.
4. Verify `/readyz` returns `ok` after database connectivity is established.
5. Verify `/metrics` exposes HTTP request counters and domain counters (`memory_health_status`, `memory_quota_exceeded_total`) for Prometheus scraping.
6. Run smoke checks against app-api, open-api, and backend-api health surfaces.
7. Monitor structured logs for `memory domain outbox event published` and storage errors.
8. When OTLP is enabled, confirm traces arrive at the collector with `http_request` spans.

## Rollback

1. Roll Deployment to the previous image digest.
2. Confirm `/readyz` on all pods.
3. If schema migration was forward-only, restore database from backup before traffic cutover.

## Post-deploy verification

- `pnpm check` and integration smoke tests against staging.
- Confirm error responses use `application/problem+json` with numeric `code` and `traceId` (no `requestId`).
- Confirm export job retrieve returns `exportRef` only, not inline payload.
- For Drive exports, confirm `memory.export.drive_upload_completed` outbox events and `drive://nodes/{id}` references (not legacy local artifact staging).
- Run `pnpm release:evidence` before tagging a release to refresh SBOM and container checksum in `sdkwork.app.config.json`.
