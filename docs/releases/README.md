# SDKWork Memory Release And Rollback

Status: active release Canon

Owner: SDKWork Memory release maintainers

Updated: 2026-07-21

## Release Matrix

| Runtime target | Profile binding | Workflow evidence | Rollback unit |
| --- | --- | --- | --- |
| Server archive | fixed standalone | build/test logs, archive digest, SBOM, provenance | previous server archive and config |
| Container | fixed standalone or fixed cloud | immutable OCI digest, image SBOM, attestation | previous digest and deployment descriptor |
| Browser PC | fixed cloud | deterministic ZIP digest, SPDX SBOM, provenance, OIDC attestation | previous ZIP/host route and public runtime config |

`sdkwork.workflow.json` is the workflow matrix authority. `agent.app.config.json` and `apps/sdkwork-memory-pc/agent.app.config.json` are the package projection authorities.

The Linux x64 server archive is built only on the workflow's Linux x64 runner and fails closed on other host platforms. Both server and container release builds use the locked Cargo graph with the production `otel` feature enabled; `pnpm test:release-features` is the matching merge and release gate.

## Candidate Build

```powershell
pnpm --dir apps/sdkwork-memory-pc check
pnpm --dir apps/sdkwork-memory-pc build:browser:cloud
pnpm check
pnpm verify
```

The PC packaging command must produce the same SHA-256 when run twice against identical `dist`, dependency graph, release timestamp, and source-tree state. Production assets exclude source maps. The ZIP contains `release-manifest.json`, `evidence/sbom.spdx.json`, and `evidence/provenance.json`.

## Publication Gates

1. Manifests, workflow, source config, topology, OpenAPI, SDK, pagination, database, and architecture checks pass.
2. Rust and PC test suites pass from the release commit.
3. Generated SDK publish checks pass for Open, App, and Backend TypeScript families.
4. Artifact SHA-256 matches the manifest package evidence.
5. GitHub OIDC artifact attestation/signature is created for the exact digest.
6. The immutable artifact is uploaded to the declared release URL.
7. Console and Admin smoke tests pass against the published runtime config.
8. Monitoring, stop conditions, and rollback owner are recorded.
9. PostgreSQL lifecycle/plugin conformance, the process-shared database pool contract and combined connection budget, container non-root health, and bounded load/soak checks pass from the release commit.
10. Only then may any server, container, PC package, or app publish status move from internal/draft to active.

Local hashes, reserved URLs, or `signatureState=pending-ci-attestation` do not satisfy gates 5-7.

## Rollout

- Deploy to an approved canary/test tenant first.
- Verify App and Backend API compatibility, authentication, pagination, job history, and provider health.
- Monitor HTTP error rate, p95/p99 latency, authz denials, job failures, provider degradation, and audit gaps.
- Stop promotion on cross-tenant evidence, elevated 5xx, contract errors, missing traces, or destructive-command duplication.

## Rollback

| Target | Action |
| --- | --- |
| Browser | restore previous immutable ZIP/route and matching runtime config; invalidate affected CDN entrypoints |
| Server | redeploy the previous archive and compatible config |
| Container | redeploy the previous immutable image digest and descriptor |
| Database | follow `docs/runbooks/RUNBOOK-migration-rollback.md`; never infer rollback from an application artifact |

After rollback, rerun health, Console, Admin, tenant-isolation, and job-history smoke checks. Record the restored artifact digest, config version, migration state, incident link, and residual forward-fix work.
