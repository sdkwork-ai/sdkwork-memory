# Kubernetes deployment

Owner: sdkwork-memory

Unified-process Memory API server manifests for cloud-hosted deployment.

## Files

- `deployment.yaml` — `sdkwork-api-memory-standalone-gateway` Deployment (`deploymentProfile=cloud`, 2 replicas, pod anti-affinity, startup/readiness/liveness probes, graceful shutdown, securityContext)
- `migration-job.yaml` — one-shot database migration Job (`db-migrate` subcommand, `SDKWORK_DATABASE_AUTO_MIGRATE=true`)
- `service.yaml` — ClusterIP service exposing port 8080 (Prometheus scrape annotations on `/metrics`)
- `servicemonitor.yaml` — Prometheus Operator scrape config for `/metrics` (optional when operator is installed)
- `prometheus-rules.yaml` — Prometheus Operator alert rules (health, authz, quota, outbox, latency, migration Job, pod restarts)
- `hpa.yaml` — CPU/memory autoscaler (min 2, max 2 — capped by the PostgreSQL connection budget; see the formula comment inside)
- `pdb.yaml` — Pod disruption budget (`minAvailable: 1`)
- `networkpolicy.yaml` — default-deny ingress/egress; only ingress-nginx → gateway:8080 in, DNS/PostgreSQL/Redis/OTLP out
- `backup-cronjob.yaml` — daily `pg_dump` CronJob (postgres:16-alpine) to the `sdkwork-memory-backups` PVC, 7-dump rotation; restore steps in `docs/runbooks/backup-restore.md` section 4
- `ingress.yaml` — Public ingress for `/apps/sdkwork-memory`
- `secret.example.yaml` — Example Secret manifests for the unified workspace database and Drive export (S3-compatible object store)
- `external-secret.example.yaml` — External Secrets Operator (v1) example sourcing the same Secrets from the cluster secret store; prefer it for production

## Prerequisites

- Container image built from `deployments/docker/Dockerfile` (ships `/app/database` lifecycle assets)
- Secret `sdkwork-memory-database` with key `database-url` — the single unified workspace PostgreSQL identity. The Memory runtime, the embedded IAM readiness/audit plane, and the Drive-backed export adapter all resolve their database identity from this one `SDKWORK_DATABASE_URL` value (`DATABASE_SPEC_PROCESS_SHARED_POOL` section 6), so no separate IAM database secret is provisioned.
- Optional Secret `sdkwork-memory-drive` when privacy exports upload through SDKWork Drive:
  - `s3-endpoint`, `s3-region`, `s3-bucket`, `s3-access-key-id`, `s3-secret-access-key` — production object store credentials

Secret provisioning, in order of preference:

1. **External Secrets Operator (production default)** — apply `external-secret.example.yaml`; it materializes the Secrets above from your cluster secret store (Vault / cloud secret manager) via a ClusterSecretStore. Keep values out of `kubectl apply` output and get rotation + audit for free.
2. **Platform secret manager** — use `secret.example.yaml` as a template and provision real values through your platform's secret tooling for clusters without ESO.

Do not apply both management modes for the same Secret name.

## Apply

```bash
kubectl apply -f deployments/kubernetes/migration-job.yaml
kubectl wait --for=condition=complete job/sdkwork-memory-db-migrate --timeout=300s
kubectl apply -f deployments/kubernetes/
```

Runtime pods set `SDKWORK_DATABASE_AUTO_MIGRATE=false`; run the migration Job before rolling out new schema versions.

The Deployment ships `OTEL_EXPORTER_OTLP_ENDPOINT` (placeholder `http://otel-collector.observability:4318` — point it at your cluster's collector; the exporter uses OTLP/HTTP, so port 4318). The runtime enables OTLP export only when the variable is non-blank; the release image is built with the `otel` feature, and there is no separate tracing-enabled switch. Keep the 4318 egress rule in `networkpolicy.yaml` in sync. Apply `prometheus-rules.yaml` when Prometheus Operator is installed.

`/readyz` checks Memory database connectivity, the current canonical Memory schema, and IAM PostgreSQL when `SDKWORK_MEMORY_ENVIRONMENT=production`. Outbox publishing uses `FOR UPDATE SKIP LOCKED` on PostgreSQL so multiple replicas do not double-publish events.
