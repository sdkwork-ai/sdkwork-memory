# Docker Deployment

Build, verify, load, and run the SDKWork Memory API server container from the repository root:

```powershell
$env:SDKWORK_PACKAGE_VERSION = "0.1.0-dev"
$env:SDKWORK_CONTAINER_LOAD_IMAGE = "true"
node scripts/package-container-oci.mjs
docker run --rm --network host -p 8080:8080 `
  -e SDKWORK_MEMORY_ENVIRONMENT=development `
  -e SDKWORK_MEMORY_CONFIG_PROFILE=development `
  -e SDKWORK_MEMORY_DEPLOYMENT_PROFILE=standalone `
  -e SDKWORK_MEMORY_DEV_AUTH_BYPASS=true `
  -e SDKWORK_DATABASE_AUTO_MIGRATE=true `
  -e SDKWORK_DATABASE_URL=postgres://sdkwork:sdkwork@127.0.0.1:5432/sdkwork_ai_dev `
  registry.sdkwork.com/apps/sdkwork-memory:0.1.0-dev
```

Memory is a server-authoritative module, so the container resolves the workspace PostgreSQL
profile. A SQLite URL is rejected at startup with an actionable diagnostic instead of being
silently accepted (ENVIRONMENT_SPEC section 7.2); `--network host` lets the container reach a
PostgreSQL instance on the host.

The packaging script constructs a bounded allowlist context containing the Memory workspace and its locked sibling Rust path dependencies. Direct builds from the application directory are unsupported because they cannot resolve that dependency closure. The script always emits and validates the OCI archive; validation checks the `linux/amd64` image config, non-root user, gateway command, bounded layer descriptors, and the ELF header of the gateway extracted from the runtime layer. `SDKWORK_CONTAINER_LOAD_IMAGE=true` additionally loads the same cached image into the local Docker daemon for smoke testing.

The image exposes `SDKWORK_MEMORY_APPLICATION_PUBLIC_INGRESS_BIND` on `0.0.0.0:8080`, ships `/app/database` lifecycle assets, runs as UID/GID `10001:10001`, and defaults `SDKWORK_MEMORY_ENVIRONMENT=production` when no overrides are supplied. Development smoke runs may opt into runtime migration as shown above. Production runtime containers keep auto-migrate disabled and start only after the `db-migrate` job succeeds. Startup schema preflight prevents workers from starting against an unmigrated database, and `/readyz` rejects later loss of the canonical lease, commercial, or search schema.

The release OCI contains SBOM and provenance attestations; Docker Desktop's legacy `docker load` importer cannot be used as the archive verifier because it does not support the attestation multi-index.

For local development without Docker, use `pnpm dev`, which loads `etc/topology/standalone.development.env` through `scripts/memory-dev.mjs`.
