#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const failures = [];
const warnings = [];

function readText(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath} must exist`);
    return '';
  }
  return fs.readFileSync(absolutePath, 'utf8');
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function assert(condition, message) {
  if (!condition) {
    failures.push(message);
  }
}

function assertDirectory(relativePath) {
  assert(fs.existsSync(path.join(repoRoot, relativePath)), `${relativePath}/ must exist`);
}

function assertCargoDependsOnWebFramework(relativeCrateToml) {
  const text = readText(relativeCrateToml);
  assert(
    text.includes('sdkwork-web-axum.workspace = true')
      || text.includes('sdkwork-web-axum = {'),
    `${relativeCrateToml} must depend on sdkwork-web-axum per WEB_FRAMEWORK_SPEC.md`,
  );
}

const requiredDirectories = [
  'apis',
  'apps',
  'crates',
  'sdks',
  'database',
  'deployments',
  'etc',
  'scripts',
  'docs',
  'tests',
  '.sdkwork',
  'specs',
];

for (const directory of requiredDirectories) {
  assertDirectory(directory);
}

assert(fs.existsSync(path.join(repoRoot, 'sdkwork.app.config.json')), 'sdkwork.app.config.json must exist');
assert(fs.existsSync(path.join(repoRoot, 'sdkwork.workflow.json')), 'sdkwork.workflow.json must exist');
assert(fs.existsSync(path.join(repoRoot, 'package.json')), 'package.json must exist per PNPM_SCRIPT_SPEC.md');
assert(
  fs.existsSync(path.join(repoRoot, '.github/workflows/package.yml')),
  '.github/workflows/package.yml must exist per GITHUB_WORKFLOW_SPEC.md',
);

const packageJson = readJson('package.json');
for (const script of ['dev', 'build', 'test', 'check', 'verify', 'clean']) {
  assert(packageJson.scripts?.[script], `package.json must expose pnpm ${script}`);
}
assert(
  packageJson.scripts?.['check:architecture-alignment'],
  'package.json must expose pnpm check:architecture-alignment',
);
assert(packageJson.scripts?.['topology:validate'], 'package.json must expose pnpm topology:validate');
assert(
  packageJson.scripts?.['check:pnpm-script-standard'],
  'package.json must expose pnpm check:pnpm-script-standard',
);
assert(
  packageJson.scripts?.['check:agent-workflow-standard'],
  'package.json must expose pnpm check:agent-workflow-standard',
);
assert(packageJson.dependencies?.['@sdkwork/app-topology'], 'package.json must declare @sdkwork/app-topology');

const cargoToml = readText('Cargo.toml');
assert(cargoToml.includes('sdkwork-web-core'), 'Cargo.toml must declare sdkwork-web-core');
assert(cargoToml.includes('sdkwork-web-axum'), 'Cargo.toml must declare sdkwork-web-axum');
assert(cargoToml.includes('sdkwork-iam-web-adapter'), 'Cargo.toml must declare sdkwork-iam-web-adapter');
assert(cargoToml.includes('sdkwork-database-config'), 'Cargo.toml must declare sdkwork-database-config');
assert(cargoToml.includes('sdkwork-database-sqlx'), 'Cargo.toml must declare sdkwork-database-sqlx');
assert(cargoToml.includes('sdkwork-database-repository'), 'Cargo.toml must declare sdkwork-database-repository');
assert(cargoToml.includes('sdkwork-utils-rust'), 'Cargo.toml must declare sdkwork-utils-rust');
assert(cargoToml.includes('sdkwork-id-core'), 'Cargo.toml must declare sdkwork-id-core');
assert(cargoToml.includes('sdkwork-api-memory-standalone-gateway'), 'Cargo.toml must include sdkwork-api-memory-standalone-gateway');
assert(cargoToml.includes('sdkwork-routes-memory-support'), 'Cargo.toml must include sdkwork-routes-memory-support');
assert(!cargoToml.includes('sdkwork-discovery'), 'sdkwork-discovery is not required until RPC services exist');

const runtimeEnvSource = readText('crates/sdkwork-memory-contract/src/runtime_env.rs');
assert(
  runtimeEnvSource.includes('memory_use_dev_inline_auth_resolver'),
  'runtime_env must gate dev inline auth resolver',
);
assert(
  runtimeEnvSource.includes('SDKWORK_MEMORY_DEV_AUTH_BYPASS'),
  'runtime_env must honor SDKWORK_MEMORY_DEV_AUTH_BYPASS',
);
assert(
  runtimeEnvSource.includes('sdkwork_utils_rust::parse_bool'),
  'runtime_env must use sdkwork-utils-rust parse_bool for env flags',
);

const memoryContractToml = readText('crates/sdkwork-memory-contract/Cargo.toml');
assert(
  memoryContractToml.includes('sdkwork-utils-rust'),
  'memory-contract Cargo.toml must declare sdkwork-utils-rust for env flag parsing',
);

const webAuthFixture = readText('crates/sdkwork-memory-test-support/src/web_auth.rs');
assert(
  webAuthFixture.includes('auth_token_jwt'),
  'test-support must provide JWT dual-token fixtures for web-framework parsers',
);
assert(
  webAuthFixture.includes('encode_unsigned_test_jwt'),
  'test-support access tokens must use JWT fixtures with dev environment claims',
);
assert(
  fs.existsSync(path.join(repoRoot, 'crates/sdkwork-memory-test-support/tests/web_auth_contract.rs')),
  'test-support must verify JWT fixture contract against web-framework parsers',
);

const forbiddenInlineDualTokenPattern = /Bearer tenant_id=/;
for (const relativePath of [
  'crates/sdkwork-memory-integration-tests/tests/app_api_mvp_flow.rs',
  'crates/sdkwork-memory-integration-tests/tests/app_backend_api_smoke_test.rs',
  'crates/sdkwork-memory-integration-tests/tests/backend_api_admin_flow.rs',
  'crates/sdkwork-memory-integration-tests/tests/governance_and_audit_flow.rs',
  'crates/sdkwork-routes-memory-app-api/tests/app_web_framework_routes.rs',
  'crates/sdkwork-routes-memory-backend-api/tests/backend_web_framework_routes.rs',
]) {
  const source = readText(relativePath);
  assert(
    !forbiddenInlineDualTokenPattern.test(source),
    `${relativePath} must not use legacy semicolon claim-string dual tokens`,
  );
  assert(
    source.includes('sdkwork_memory_test_support::web_auth')
      || source.includes('memory_auth_token_bearer'),
    `${relativePath} must use sdkwork-memory-test-support web_auth JWT fixtures`,
  );
}

const openWebBootstrap = readText('crates/sdkwork-routes-memory-open-api/src/web_bootstrap.rs');
assert(
  openWebBootstrap.includes('memory_web_auth_mode_from_env'),
  'open-api web bootstrap must use shared memory web auth mode',
);
assert(
  !openWebBootstrap.includes('SDKWORK_DATABASE_URL").is_ok()'),
  'open-api web bootstrap must not gate auth on DATABASE_URL presence',
);

const memoryWebAuth = readText('crates/sdkwork-routes-memory-support/src/lib.rs');
assert(
  memoryWebAuth.includes('ProductionFailClosedResolver'),
  'routes-memory-support must provide production fail-closed auth resolver',
);
assert(
  memoryWebAuth.includes('resolve_access_token'),
  'production fail-closed resolver must reject access-token auth',
);

const memoryProblem = readText('crates/sdkwork-routes-memory-support/src/problem.rs');
assert(
  memoryProblem.includes('MemoryApiProblem'),
  'routes-memory-support must provide shared MemoryApiProblem responses',
);
assert(
  readText('crates/sdkwork-routes-memory-open-api/src/error.rs').includes('sdkwork_routes_memory_support'),
  'open-api router must reuse shared problem responses',
);

for (const relativePath of [
  'crates/sdkwork-routes-memory-app-api/src/routes.rs',
  'crates/sdkwork-routes-memory-backend-api/src/routes.rs',
]) {
  const source = readText(relativePath);
  assert(
    !source.includes('Json<serde_json::Value>') && !source.includes('Query<serde_json::Value>'),
    `${relativePath} must use typed contract DTO extractors instead of serde_json::Value`,
  );
}

const backendPorts = readText('crates/sdkwork-memory-contract/src/backend_ports.rs');
assert(
  !backendPorts.includes('serde_json::Value'),
  'backend_ports must not expose untyped serde_json::Value in MemoryBackendApi',
);
const appPorts = readText('crates/sdkwork-memory-contract/src/app_ports.rs');
assert(
  !appPorts.includes('serde_json::Value'),
  'app_ports must not expose untyped serde_json::Value in MemoryAppApi',
);
assert(
  readText('crates/sdkwork-memory-contract/src/admin_dto.rs').includes('MemoryIndex'),
  'memory-contract must declare admin DTO types aligned to backend OpenAPI',
);

assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/admin_tables.rs').includes(
    'capabilities_json',
  ),
  'native-sql admin tables must persist provider binding capabilities_json',
);

const adminTables = readText('plugins/sdkwork-memory-plugin-native-sql/src/admin_tables.rs');
const nativeSqlStore = readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs');
assert(
  nativeSqlStore.includes('database/ddl/baseline/postgres/0001_memory_baseline.sql') &&
    nativeSqlStore.includes('database/ddl/baseline/sqlite/0001_memory_baseline.sql'),
  'native-sql compatibility bootstrap must consume the consolidated application-root baseline for both engines',
);
for (const engine of ['postgres', 'sqlite']) {
  assert(
    fs.existsSync(
      path.join(repoRoot, `database/ddl/baseline/${engine}/0001_memory_baseline.sql`),
    ),
    `consolidated ${engine} baseline must exist per DATABASE_FRAMEWORK_SPEC section 584`,
  );
}
for (const [needle, message] of [
  ['implementation_profile_id', 'native-sql admin tables must persist index implementation_profile_id'],
  ['provider_binding_id', 'native-sql admin tables must persist index provider_binding_id'],
  ['fusion_policy_json', 'native-sql admin tables must persist retrieval fusion_policy_json'],
  ['rerank_policy_json', 'native-sql admin tables must persist retrieval rerank_policy_json'],
  ['config_json', 'native-sql admin tables must persist implementation profile config_json'],
  ['rollout_json', 'native-sql admin tables must persist implementation profile rollout_json'],
  ['endpoint_ref', 'native-sql admin tables must persist provider binding endpoint_ref'],
]) {
  assert(adminTables.includes(needle), message);
}

const backendAdminApi = readText(
  'crates/sdkwork-intelligence-memory-service/src/backend_admin_api.rs',
);
assert(
  backendAdminApi.includes('optional_i64_as_u64(row.implementation_profile_id)'),
  'backend admin service must map index implementation_profile_id from SQL rows',
);
assert(
  backendAdminApi.includes('decode_optional_json(row.fusion_policy_json.as_deref())'),
  'backend admin service must map retrieval fusion_policy from SQL rows',
);

const k8sDeployment = readText('deployments/kubernetes/deployment.yaml');
assert(k8sDeployment.includes('path: /healthz'), 'k8s probes must target /healthz');
assert(
  k8sDeployment.includes('path: /readyz'),
  'k8s readiness probe must target /readyz for database-backed readiness',
);
assert(
  readText('crates/sdkwork-api-memory-assembly/src/bootstrap.rs').includes('bootstrap_memory_runtime_from_env'),
  'memory assembly bootstrap must use unified memory runtime bootstrap',
);
assert(
  /\.route\(\s*["']\/readyz["']/.test(
    readText('crates/sdkwork-api-memory-assembly/src/bootstrap.rs'),
  ),
  'memory assembly must expose the /readyz readiness endpoint for the thin gateway projection',
);
assert(
  readText('crates/sdkwork-routes-memory-support/src/readiness.rs').includes(
    'memory_dependency_ready_check',
  ),
  'composite readiness must validate IAM and Redis dependencies',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'FOR UPDATE SKIP LOCKED',
  ),
  'native-sql outbox claim must use SKIP LOCKED for multi-replica publish safety',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    "publish_state = 'processing'",
  ),
  'native-sql outbox claim must move rows to processing before external delivery',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'ack_outbox_delivery_success',
  ),
  'native-sql outbox must ack successful delivery before marking published',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/outbox_delivery.rs').includes(
    'deliver_outbox_event',
  ),
  'outbox publisher must deliver events through a dedicated adapter',
);
{
  // The metrics handler lives in the assembly bootstrap, delimited by the next public
  // entrypoint. Resolve the anchors explicitly so a future rename fails loudly here instead of
  // degrading into a silent pass.
  const assemblyBootstrap = readText('crates/sdkwork-api-memory-assembly/src/bootstrap.rs');
  const metricsStart = assemblyBootstrap.indexOf('async fn metrics');
  const metricsEnd = assemblyBootstrap.indexOf('pub async fn assemble_api_router_from_env');
  assert(
    metricsStart !== -1 && metricsEnd > metricsStart,
    'memory assembly bootstrap must declare the metrics handler ahead of its router entrypoint',
  );
  assert(
    !assemblyBootstrap.slice(metricsStart, metricsEnd).includes('ready_check'),
    'metrics endpoint must not depend on database readiness probes',
  );
}
assert(
  k8sDeployment.includes('startupProbe:'),
  'k8s deployment must define startupProbe before readiness/liveness',
);
assert(
  k8sDeployment.includes('podAntiAffinity'),
  'k8s deployment must spread standalone-gateway replicas across nodes',
);
assert(
  readText('crates/sdkwork-api-memory-assembly/src/bootstrap.rs').includes('route("/metrics"'),
  'memory assembly must expose /metrics for Prometheus scraping',
);
assert(
  readText('deployments/kubernetes/service.yaml').includes('prometheus.io/scrape'),
  'k8s service must annotate Prometheus scrape target for /metrics',
);
assert(
  readText('deployments/kubernetes/migration-job.yaml').includes('db-migrate'),
  'k8s migration job must run standalone-gateway db-migrate subcommand',
);
assert(
  readText('crates/sdkwork-routes-memory-support/src/metrics.rs').includes(
    'memory_metric_environment_label',
  ),
  'memory metrics must export canonical environment label per OBSERVABILITY_SPEC',
);
assert(
  readText('crates/sdkwork-routes-memory-support/src/correlation.rs').includes('http_request'),
  'correlation middleware must emit tracing spans for request correlation',
);
assert(
  readText('crates/sdkwork-api-memory-standalone-gateway/src/observability.rs').includes(
    'OTEL_EXPORTER_OTLP_ENDPOINT',
  ),
  'standalone-gateway must support optional OTLP tracing per OBSERVABILITY_SPEC',
);
assert(
  fs.existsSync(path.join(repoRoot, 'deployments/kubernetes/prometheus-rules.yaml')),
  'k8s PrometheusRule manifests must exist for production alerting',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/domain_metrics.rs').includes(
    'memory_quota_exceeded_total',
  ),
  'domain metrics must export memory_quota_exceeded_total counter',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs').includes(
    'memory.export.drive_upload_completed',
  ),
  'app backend must upload drive exports through SDKWork Drive and emit completion events',
);
assert(
  cargoToml.includes('sdkwork-memory-drive'),
  'Cargo.toml must declare sdkwork-memory-drive integration crate',
);
assert(
  fs.existsSync(path.join(repoRoot, 'tools/check_sdkwork_memory_release_readiness.mjs')),
  'release readiness gate script must exist for commercial deployment',
);
assert(
  readText('scripts/release/workflow-supply-chain-evidence.mjs').includes(
    'createSbomProvenanceAndEvidence',
  ) &&
    readText('scripts/release/workflow-supply-chain-evidence.mjs').includes(
      'verifyReleaseEvidence',
    ),
  'release workflow must create and verify byte-bound SBOM, provenance, checksum, and signature evidence',
);
assert(
  readText('scripts/package-release-artifact.mjs').includes('release-manifest.json'),
  'release packaging script must emit release manifest metadata',
);
assert(
  k8sDeployment.includes('SDKWORK_DATABASE_AUTO_MIGRATE'),
  'k8s deployment must disable runtime auto-migrate (use migration Job)',
);
assert(
  readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/runtime.rs').includes(
    'bootstrap_memory_runtime_from_env',
  ),
  'repository-sqlx must expose unified memory runtime bootstrap',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/privacy.rs').includes(
    'forget_all_records_in_space',
  ),
  'native-sql privacy module must implement space-scoped forget',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/open_api.rs').includes(
    'create_canonical_memory_atomic_with_quota',
  ),
  'create_memory must enforce per-space record quotas per PERFORMANCE_SPEC',
);
assert(
  backendAdminApi.includes('supersede_canonical_memory_atomic_with_quota') &&
    !backendAdminApi.includes('.supersede_record_open_api('),
  'backend supersede must dispatch through the atomic quota-aware record-store port',
);
const nativeCanonicalSource = readText(
  'plugins/sdkwork-memory-plugin-native-sql/src/canonical_data.rs',
);
assert(
  nativeCanonicalSource.includes('command.superseded_journal') &&
    nativeCanonicalSource.includes('command.created_journal') &&
    nativeCanonicalSource.includes('remove_record_fts_on_tx('),
  'native-sql supersede must commit both journals and remove the old search projection transactionally',
);
const candidatePromotionSource = readText(
  'crates/sdkwork-intelligence-memory-service/src/candidate_promotion.rs',
);
assert(
  candidatePromotionSource.includes('promote_candidate_atomic_with_quota_and_journal'),
  'candidate promotion must dispatch through the journaled atomic runtime candidate port',
);
assert(
  candidatePromotionSource.includes('retrieve_candidate_detail('),
  'candidate promotion must recover evidence and targets through the provider-neutral runtime detail port',
);
assert(
  !candidatePromotionSource.includes('NativeSqlCandidateDetailRow') &&
    !candidatePromotionSource.includes('retrieve_candidate_detail_for_tenant'),
  'candidate promotion service must not depend on Native SQL candidate detail projections',
);
assert(
  !candidatePromotionSource.includes('.promote_and_approve_candidate'),
  'candidate promotion service must not bypass the runtime candidate port',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'append_journal_on_tx(self, &mut tx, command.scope, journal)',
  ),
  'native-sql candidate promotion must append outbox/audit before its transaction commits',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/tenant_quota.rs').includes(
    'quota_exceeded',
  ),
  'tenant quota module must map exhaustion to quota_exceeded problem code',
);
assert(
  readText('crates/sdkwork-routes-memory-support/src/problem.rs').includes('QuotaExceeded'),
  'router problem mapping must translate quota_exceeded to HTTP 429',
);
const appBackendSource = readText(
  'crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs',
);
const openApiServiceSource = readText(
  'crates/sdkwork-intelligence-memory-service/src/open_api.rs',
);
assert(
  appBackendSource.includes('.list_candidates(ListMemoryCandidatesQuery') &&
    openApiServiceSource.includes('.list_candidates(ListMemoryCandidatesQuery'),
  'candidate HTTP lists must dispatch through the provider-neutral runtime candidate port',
);
for (const [sourceName, source] of [
  ['app_backend_api.rs', appBackendSource],
  ['open_api.rs', openApiServiceSource],
]) {
  assert(
    !source.includes('NativeSqlCandidateRow') &&
      !source.includes('.list_candidates_for_tenant(') &&
      !source.includes('.retrieve_candidate_for_tenant('),
    `${sourceName} must not consume Native SQL candidate projections directly`,
  );
}
const referenceRuntimeSource = readText(
  'plugins/sdkwork-memory-plugin-reference-profiles/src/reference_runtime.rs',
);
assert(
  referenceRuntimeSource.includes('candidate_listing_index') &&
    referenceRuntimeSource.includes('candidate_space_listing_index') &&
    referenceRuntimeSource.includes('index.range((lower_bound, Unbounded))'),
  'reference candidate listing must use incrementally maintained ordered indexes',
);
assert(
  appBackendSource.includes('create_space_atomic_with_quota'),
  'create_space must dispatch through atomic user-space quota admission',
);
assert(
  !appBackendSource.includes('assert_user_space_quota') &&
    !appBackendSource.includes('.create_space_record('),
  'create_space must not split user-space quota observation from the space insert',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/space_data.rs').includes(
    'SPACE_QUOTA_LOCK_VERSION',
  ),
  'native-sql space creation must serialize quota admission inside its transaction',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs').includes(
    'collect_export_payload_for_spaces',
  ),
  'app backend must produce inline export payloads per PRIVACY_SPEC',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/native_sql_phase1_runtime.rs').includes(
    'validate_native_sql_phase1_ports',
  ),
  'native-sql plugin must validate required SPI ports at runtime',
);
assert(
  readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/runtime.rs').includes(
    'validate_native_sql_port_builders',
  ),
  'memory runtime bootstrap must validate native-sql port builders before serving',
);
assert(
  readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/bootstrap.rs').includes(
    'NativeSqlPhase1Runtime::connect',
  ),
  'data plane bootstrap must materialize store through NativeSqlPhase1Runtime',
);
assert(
  readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/bootstrap.rs').includes(
    'connect_without_migration',
  ) ||
    readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/bootstrap.rs').includes(
      'open_native_sql_store_from_pool',
    ),
  'postgres data plane bootstrap must skip duplicate store migrations after database-host bootstrap',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/open_api.rs').includes(
    'from_phase1_runtime',
  ),
  'service layer must construct from NativeSqlPhase1Runtime without store clone',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'list_candidates_for_tenant',
  ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
      'uuid > ?',
    ),
  'candidate list store query must support cursor pagination via stable uuid ordering',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'list_spaces_for_tenant',
  ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
      'list_habits_for_tenant',
    ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
      'list_audit_logs_for_tenant',
    ),
  'tenant list store queries must support cursor pagination for spaces, habits, and audit logs',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/admin_tables.rs').includes(
    'list_ai_indexes_for_tenant',
  ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/admin_tables.rs').includes(
      'ORDER BY id ASC',
    ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
      'list_retrieval_traces_for_tenant',
    ) &&
    readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
      'ORDER BY id DESC',
    ),
  'admin config and retrieval trace list queries must support cursor pagination',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'list_record_sources_for_memory',
  ) &&
    readText('crates/sdkwork-routes-memory-app-api/src/routes.rs').includes(
      'ListMemorySourcesQuery',
    ),
  'memory record source list must wire OpenAPI cursor pagination query params',
);
assert(
  readText('tools/materialize_phase1_contracts.mjs').includes(
    'resolvePreservedReleaseChecksum',
  ),
  'materialize script must preserve release SHA-256 checksum across manifest regeneration',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/access.rs').includes(
    'assert_actor_can_access_space_i64',
  ) &&
    readText('crates/sdkwork-intelligence-memory-service/src/open_api.rs').includes(
      'retrieve_candidate',
    ) &&
    readText('crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs').includes(
      'retrieve_habit',
    ),
  'candidate and habit endpoints must enforce actor space access checks',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'owner_subject_id = ?',
  ) &&
    readText('crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs').includes(
      'actor_scope.as_deref()',
    ),
  'space list queries must scope results to actor-owned user spaces',
);
assert(
  k8sDeployment.includes('SDKWORK_MEMORY_ENVIRONMENT'),
  'k8s deployment must set SDKWORK_MEMORY_ENVIRONMENT',
);
assert(
  k8sDeployment.includes('SDKWORK_MEMORY_APP_ROOT'),
  'k8s deployment must set SDKWORK_MEMORY_APP_ROOT',
);

const dockerfile = readText('deployments/docker/Dockerfile');
assert(
  dockerfile.includes(
    'COPY --from=builder /workspace/sdkwork-memory/database /app/database',
  ),
  'docker image must ship database lifecycle assets from the application workspace',
);
assert(dockerfile.includes('SDKWORK_MEMORY_APP_ROOT=/app'), 'docker image must set SDKWORK_MEMORY_APP_ROOT');
const dockerReadme = readText('deployments/docker/README.md');
assert(
  dockerReadme.includes('node scripts/package-container-oci.mjs')
    && !dockerReadme.includes('docker build -f'),
  'Docker documentation must use the bounded sibling-aware OCI packager',
);
const containerPackager = readText('scripts/package-container-oci.mjs');
assert(
  containerPackager.includes('maxContextBytes = 256 * 1024 * 1024')
    && containerPackager.includes('maxContextFiles = 50_000'),
  'OCI packaging must enforce byte and file-count context bounds',
);
assert(
  containerPackager.includes('config?.architecture !== "amd64"')
    && containerPackager.includes('config?.os !== "linux"')
    && containerPackager.includes('config?.config?.User !== "10001:10001"')
    && containerPackager.includes('OCI runtime binary is not an ELF executable'),
  'OCI packaging must verify its platform, non-root identity, and runtime ELF content',
);
assert(
  readText('crates/sdkwork-intelligence-memory-service/src/open_api.rs')
    .includes('.verify_canonical_schema()'),
  'service readiness must reject stale or missing canonical database schema',
);

const databaseManifest = readJson('database/database.manifest.json');
assert(databaseManifest.tablePrefix === 'ai_', 'database manifest tablePrefix must be ai_');

const workflow = readJson('sdkwork.workflow.json');
const dependencyIds = new Set((workflow.dependencies || []).map((dependency) => dependency.id));
for (const dependencyId of [
  'sdkwork-appbase',
  'sdkwork-database',
  'sdkwork-web-framework',
  'sdkwork-utils',
  'sdkwork-id',
  'sdkwork-sdk-generator',
  'sdkwork-app-topology',
]) {
  assert(dependencyIds.has(dependencyId), `sdkwork.workflow.json must declare ${dependencyId}`);
}
assert(!dependencyIds.has('sdkwork-discovery'), 'sdkwork.workflow.json must not declare sdkwork-discovery until RPC exists');

const routerCrates = [
  'crates/sdkwork-routes-memory-open-api/Cargo.toml',
  'crates/sdkwork-routes-memory-app-api/Cargo.toml',
  'crates/sdkwork-routes-memory-backend-api/Cargo.toml',
];

for (const routerCrate of routerCrates) {
  assertCargoDependsOnWebFramework(routerCrate);
  const crateName = path.basename(path.dirname(routerCrate));
  assert(
    fs.existsSync(path.join(repoRoot, `crates/${crateName}/src/web_bootstrap.rs`)),
    `${crateName} must provide web_bootstrap.rs`,
  );
  assert(
    fs.existsSync(path.join(repoRoot, `crates/${crateName}/src/manifest.rs`)),
    `${crateName} must provide manifest.rs`,
  );
  assert(
    fs.existsSync(path.join(repoRoot, `crates/${crateName}/README.md`)),
    `${crateName} must provide README.md`,
  );
  assert(
    fs.existsSync(path.join(repoRoot, `crates/${crateName}/specs/component.spec.json`)),
    `${crateName} must provide specs/component.spec.json`,
  );
}

for (const routeTest of [
  'crates/sdkwork-routes-memory-open-api/tests/open_api_routes.rs',
  'crates/sdkwork-routes-memory-open-api/tests/open_web_framework_routes.rs',
  'crates/sdkwork-routes-memory-open-api/tests/open_openapi_routes.rs',
  'crates/sdkwork-routes-memory-app-api/tests/app_api_routes.rs',
  'crates/sdkwork-routes-memory-app-api/tests/app_web_framework_routes.rs',
  'crates/sdkwork-routes-memory-app-api/tests/app_openapi_routes.rs',
  'crates/sdkwork-routes-memory-backend-api/tests/backend_api_routes.rs',
  'crates/sdkwork-routes-memory-backend-api/tests/backend_web_framework_routes.rs',
  'crates/sdkwork-routes-memory-backend-api/tests/backend_openapi_routes.rs',
]) {
  assert(fs.existsSync(path.join(repoRoot, routeTest)), `${routeTest} must exist`);
}

assert(
  fs.existsSync(path.join(repoRoot, 'deployments/docker/Dockerfile')),
  'deployments/docker/Dockerfile must exist per DEPLOYMENT_SPEC.md',
);
assert(
  fs.existsSync(path.join(repoRoot, 'scripts/memory-dev.mjs')),
  'scripts/memory-dev.mjs must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'scripts/cargo-test-workspace.mjs')),
  'scripts/cargo-test-workspace.mjs must exist for reliable Windows workspace test linking',
);
assert(
  packageJson.scripts?.verify === 'pnpm exec sdkwork-app verify',
  'package.json verify must remain the canonical sdkwork-app facade',
);
assert(
  packageJson.scripts?.test === 'pnpm exec sdkwork-app test',
  'package.json test must remain the canonical sdkwork-app facade',
);
assert(
  packageJson.scripts?.['_sdkwork:verify']?.includes('scripts/cargo-test-workspace.mjs'),
  'package.json _sdkwork:verify must run scripts/cargo-test-workspace.mjs',
);
assert(
  packageJson.scripts?.['_sdkwork:test']?.includes('scripts/cargo-test-workspace.mjs'),
  'package.json _sdkwork:test must run scripts/cargo-test-workspace.mjs',
);
const releaseFeatureTest =
  'cargo test --locked -p sdkwork-api-memory-standalone-gateway --features otel';
assert(
  packageJson.scripts?.['test:release-features'] === releaseFeatureTest,
  'package.json must compile and test the production gateway otel feature',
);
assert(
  packageJson.scripts?.['_sdkwork:verify']?.includes('pnpm test:release-features')
    && packageJson.scripts?.['_sdkwork:test']?.includes('pnpm test:release-features'),
  'root test and verify hooks must include production gateway feature coverage',
);

const appRoutesSource = readText('crates/sdkwork-routes-memory-app-api/src/routes.rs');
assert(
  appRoutesSource.includes('Json<MemoryReviewRequest>'),
  'app-api routes must deserialize MemoryReviewRequest for review operations',
);
assert(
  !appRoutesSource.includes('Json<Value>'),
  'app-api routes must not accept untyped serde_json::Value request bodies',
);

const backendRoutesSource = readText('crates/sdkwork-routes-memory-backend-api/src/routes.rs');
assert(
  backendRoutesSource.includes('Json<MemoryReviewRequest>'),
  'backend-api routes must deserialize MemoryReviewRequest for review operations',
);

const apiServerSmoke = readText('crates/sdkwork-memory-integration-tests/tests/api_server_smoke_test.rs');
assert(
  apiServerSmoke.includes('memory_auth_token_bearer'),
  'standalone-gateway smoke test must verify production fail-closed behavior for app-api dual tokens',
);

const repositorySqlxToml = readText('crates/sdkwork-intelligence-memory-repository-sqlx/Cargo.toml');
assert(
  repositorySqlxToml.includes('sdkwork-database-sqlx'),
  'repository-sqlx crate must depend on sdkwork-database-sqlx',
);
assert(
  repositorySqlxToml.includes('sdkwork-database-repository'),
  'repository-sqlx crate must depend on sdkwork-database-repository per DATABASE_SPEC.md',
);
assert(
  repositorySqlxToml.includes('sdkwork-utils-rust'),
  'repository-sqlx crate must depend on sdkwork-utils-rust',
);
assert(
  readText('crates/sdkwork-intelligence-memory-repository-sqlx/src/db.rs').includes('sdkwork_utils_rust::is_blank'),
  'repository-sqlx db bootstrap must use sdkwork-utils-rust is_blank for engine validation',
);
assert(
  repositorySqlxToml.includes('migrate'),
  'repository-sqlx sqlx dependency must enable migrate feature',
);

const serviceToml = readText('crates/sdkwork-intelligence-memory-service/Cargo.toml');
assert(
  serviceToml.includes('sdkwork-utils-rust'),
  'service crate must depend on sdkwork-utils-rust for shared utility helpers',
);
assert(
  serviceToml.includes('sdkwork-id-core'),
  'service crate must depend on sdkwork-id-core for snowflake id generation',
);

const componentSpec = readJson('specs/component.spec.json');
const sdkDependencyIds = new Set((componentSpec.contracts?.sdkDependencies ?? []).map((item) => item.workspace));
for (const workspace of [
  'sdkwork-web-framework',
  'sdkwork-database',
  'sdkwork-utils',
  'sdkwork-appbase',
  'sdkwork-id',
  'sdkwork-sdk-generator',
]) {
  assert(
    sdkDependencyIds.has(workspace),
    `specs/component.spec.json must declare sdkDependencies workspace ${workspace}`,
  );
}

assert(!sdkDependencyIds.has('sdkwork-discovery'), 'component spec must not declare sdkwork-discovery until RPC exists');

assert(fs.existsSync(path.join(repoRoot, '.env.example')), '.env.example must exist');
assert(
  readText('AGENTS.md').includes('## Task-Specific Standards')
    && readText('AGENTS.md').includes('PAGINATION_SPEC.md'),
  'AGENTS.md must route list/search work to PAGINATION_SPEC.md without copying the normative body',
);
assert(
  readText('AGENTS.md').includes('check-pagination.mjs'),
  'AGENTS.md must reference check-pagination.mjs verification',
);
assert(fs.existsSync(path.join(repoRoot, '.sdkwork/.gitignore')), '.sdkwork/.gitignore must exist');
assert(fs.existsSync(path.join(repoRoot, 'docs/topology-standard.md')), 'docs/topology-standard.md must exist');
assert(
  fs.existsSync(path.join(repoRoot, 'scripts/lib/memory-topology.mjs')),
  'scripts/lib/memory-topology.mjs must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'scripts/generate-memory-sdk.mjs')),
  'scripts/generate-memory-sdk.mjs must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'sdks/standardize-memory-sdk-family.mjs')),
  'sdks/standardize-memory-sdk-family.mjs must exist',
);

const topologySpec = readJson('specs/topology.spec.json');
assert(topologySpec.schemaVersion === 5, 'specs/topology.spec.json schemaVersion must be 5');
assert(topologySpec.archetype === 'application-http-gateway', 'topology archetype must be application-http-gateway');
const defaultProfileIds = [
  topologySpec.defaults?.developmentProfileId,
  topologySpec.defaults?.productionProfileId,
  topologySpec.defaults?.desktopBuildProfileId,
];
for (const profileId of defaultProfileIds) {
  assert(profileId, 'topology defaults must declare development, production, and desktop build profile ids');
  assert(
    topologySpec.profileFiles?.[profileId],
    `specs/topology.spec.json must declare profileFiles.${profileId}`,
  );
  assert(
    fs.existsSync(path.join(repoRoot, topologySpec.profileFiles[profileId])),
    `${topologySpec.profileFiles[profileId]} must exist`,
  );
}
for (const [profileId, profileFile] of Object.entries(topologySpec.profileFiles ?? {})) {
  assert(
    fs.existsSync(path.join(repoRoot, profileFile)),
    `topology profile ${profileId} must resolve existing file ${profileFile}`,
  );
}
assert(
  fs.existsSync(path.join(repoRoot, 'sdks/test/verify-sdk-ownership-boundaries.test.mjs')),
  'sdks/test/verify-sdk-ownership-boundaries.test.mjs must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'tools/verify_sdkwork_structure.ps1')),
  'tools/verify_sdkwork_structure.ps1 must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'tools/verify_openapi_operation_ids.ps1')),
  'tools/verify_openapi_operation_ids.ps1 must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, 'deployments/deploy.yaml')),
  'deployments/deploy.yaml must exist per SDKWORK_DEPLOY_SPEC.md',
);
assert(
  fs.existsSync(path.join(repoRoot, 'crates/sdkwork-memory-drive/src/object_store.rs')),
  'crates/sdkwork-memory-drive/src/object_store.rs must resolve Drive object stores from provider metadata',
);
const memoryDriveToml = readText('crates/sdkwork-memory-drive/Cargo.toml');
assert(
  memoryDriveToml.includes('sdkwork-drive-storage-s3'),
  'sdkwork-memory-drive must depend on sdkwork-drive-storage-s3 for production object store support',
);
assert(
  readText('crates/sdkwork-routes-memory-support/src/principal.rs').includes('parse_int'),
  'routes-memory-support must parse IAM principal ids via sdkwork-utils-rust',
);
assert(
  fs.existsSync(path.join(repoRoot, 'crates/sdkwork-memory-drive/Cargo.toml')),
  'crates/sdkwork-memory-drive must exist for Drive-backed export integration',
);
assert(
  fs.existsSync(path.join(repoRoot, 'deployments/kubernetes/secret.example.yaml')),
  'deployments/kubernetes/secret.example.yaml must document production secret keys',
);
assert(
  fs.existsSync(path.join(repoRoot, 'deployments/kubernetes/service.yaml')),
  'deployments/kubernetes/service.yaml must exist per DEPLOYMENT_SPEC.md',
);
assert(
  fs.existsSync(path.join(repoRoot, 'tests/contract/database-framework.contract.test.mjs')),
  'tests/contract/database-framework.contract.test.mjs must exist',
);
assert(
  fs.existsSync(path.join(repoRoot, '.github/workflows/verify.yml')),
  '.github/workflows/verify.yml must exist per GITHUB_WORKFLOW_SPEC.md',
);

const authorityManifest = readJson('apis/authority-manifest.json');
for (const surface of authorityManifest.surfaces ?? []) {
  assert(surface.authorityPath, 'authority manifest surface must declare authorityPath');
  assert(surface.sdkPath, 'authority manifest surface must declare sdkPath');
  assert(
    fs.existsSync(path.join(repoRoot, surface.authorityPath)),
    `${surface.authorityPath} must exist`,
  );
  assert(fs.existsSync(path.join(repoRoot, surface.sdkPath)), `${surface.sdkPath} must exist`);
}

const sdkFamilyRoots = [
  'sdks/sdkwork-memory-sdk',
  'sdks/sdkwork-memory-app-sdk',
  'sdks/sdkwork-memory-backend-sdk',
];
for (const familyRoot of sdkFamilyRoots) {
  const manifest = readJson(path.join(familyRoot, 'sdk-manifest.json'));
  assert(manifest.standardProfile === 'sdkwork-v3', `${familyRoot} must declare standardProfile sdkwork-v3`);
  assert(manifest.generatedOutput, `${familyRoot} must declare generatedOutput`);
}

const routeManifestPaths = [
  'sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json',
  'sdks/_route-manifests/app-api/sdkwork-routes-memory-app-api.route-manifest.json',
  'sdks/_route-manifests/backend-api/sdkwork-routes-memory-backend-api.route-manifest.json',
];

for (const relativePath of routeManifestPaths) {
  const manifest = readJson(relativePath);
  for (const route of manifest.routes ?? []) {
    assert(
      route.requestContext === 'WebRequestContext',
      `${relativePath} route ${route.method} ${route.path} must declare WebRequestContext`,
    );
    assert(
      ['open-api', 'app-api', 'backend-api'].includes(route.apiSurface),
      `${relativePath} route ${route.method} ${route.path} must declare canonical apiSurface`,
    );
  }
}

assert(componentSpec.component.type === 'web-backend-service', 'component type must be web-backend-service');
assert(componentSpec.component.domain === 'intelligence', 'component domain must be intelligence');
assert(componentSpec.component.capability === 'memory', 'component capability must be memory');

const canonicalSpecs = (componentSpec.canonicalSpecs || []).map((entry) => entry.file);
for (const specFile of [
  'WEB_FRAMEWORK_SPEC.md',
  'WEB_BACKEND_SPEC.md',
  'DATABASE_SPEC.md',
  'DEPLOYMENT_SPEC.md',
  'DRIVE_SPEC.md',
  'SDK_SPEC.md',
  'SDK_WORKSPACE_GENERATION_SPEC.md',
  'TEST_SPEC.md',
]) {
  assert(canonicalSpecs.includes(specFile), `specs/component.spec.json must reference ${specFile}`);
}

const crateComponentSpecs = [
  'crates/sdkwork-memory-contract/specs/component.spec.json',
  'crates/sdkwork-memory-spi/specs/component.spec.json',
  'crates/sdkwork-memory-profile-resolver/specs/component.spec.json',
  'crates/sdkwork-routes-memory-support/specs/component.spec.json',
  'crates/sdkwork-intelligence-memory-service/specs/component.spec.json',
  'crates/sdkwork-intelligence-memory-repository-sqlx/specs/component.spec.json',
  'crates/sdkwork-memory-database-host/specs/component.spec.json',
  'crates/sdkwork-api-memory-standalone-gateway/specs/component.spec.json',
];
for (const relativePath of crateComponentSpecs) {
  assert(fs.existsSync(path.join(repoRoot, relativePath)), `${relativePath} must exist`);
}

const requiredGeneratedSdkRoots = [
  'sdks/sdkwork-memory-sdk/sdkwork-memory-sdk-typescript/generated/server-openapi',
  'sdks/sdkwork-memory-app-sdk/sdkwork-memory-app-sdk-typescript/generated/server-openapi',
  'sdks/sdkwork-memory-backend-sdk/sdkwork-memory-backend-sdk-typescript/generated/server-openapi',
];
for (const relativePath of requiredGeneratedSdkRoots) {
  assert(fs.existsSync(path.join(repoRoot, relativePath)), `${relativePath} must exist`);
  for (const requiredFile of ['sdkwork-sdk.json', 'package.json', 'src/index.ts']) {
    assert(
      fs.existsSync(path.join(repoRoot, relativePath, requiredFile)),
      `${relativePath}/${requiredFile} must exist`,
    );
  }
}

const openapiPaths = [
  'sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json',
  'sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json',
  'sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json',
];

for (const relativePath of openapiPaths) {
  const openapi = readJson(relativePath);
  let hasSurface = false;
  for (const pathItem of Object.values(openapi.paths ?? {})) {
    for (const operation of Object.values(pathItem ?? {})) {
      if (operation && typeof operation === 'object' && operation.operationId) {
        assert(
          operation['x-sdkwork-request-context'] === 'WebRequestContext',
          `${relativePath} operation ${operation.operationId} must declare WebRequestContext`,
        );
        assert(
          ['open-api', 'app-api', 'backend-api'].includes(operation['x-sdkwork-api-surface']),
          `${relativePath} operation ${operation.operationId} must declare canonical x-sdkwork-api-surface`,
        );
        hasSurface = true;
      }
    }
  }
  if (!hasSurface) {
    assert(false, `${relativePath} must declare x-sdkwork-api-surface on operations`);
  }
}

const abuseSensitiveOperationIds = [
  'forgetRequests.create',
  'exportJobs.create',
  'memories.delete',
  'retentionJobs.create',
  'migrationJobs.create',
  'indexes.rebuild',
];

function collectOpenApiOperations(relativePath) {
  const openapi = readJson(relativePath);
  const operations = [];
  for (const [pathKey, pathItem] of Object.entries(openapi.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (
        operation
        && typeof operation === 'object'
        && operation.operationId
        && ['get', 'post', 'patch', 'delete'].includes(method)
      ) {
        operations.push({ method, path: pathKey, operation });
      }
    }
  }
  return operations;
}

for (const operationId of abuseSensitiveOperationIds) {
  for (const relativePath of openapiPaths) {
    const match = collectOpenApiOperations(relativePath).find(
      (entry) => entry.operation.operationId === operationId,
    );
    if (match) {
      assert(
        match.operation['x-sdkwork-rate-limit-tier'] === 'authCritical',
        `${relativePath} operation ${operationId} must declare authCritical x-sdkwork-rate-limit-tier`,
      );
    }
  }
}

const openApiRelativePath = 'sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json';
for (const entry of collectOpenApiOperations(openApiRelativePath)) {
  if (!['post', 'patch', 'delete'].includes(entry.method)) {
    continue;
  }
  const expectedTier =
    entry.operation.operationId === 'memories.delete' ? 'authCritical' : 'openApiDefault';
  assert(
    entry.operation['x-sdkwork-rate-limit-tier'] === expectedTier,
    `${openApiRelativePath} ${entry.method.toUpperCase()} ${entry.path} must declare ${expectedTier} rate limit tier`,
  );
}

for (const relativePath of [
  'crates/sdkwork-routes-memory-app-api/src/http_route_manifest.rs',
  'crates/sdkwork-routes-memory-open-api/src/http_route_manifest.rs',
  'crates/sdkwork-routes-memory-backend-api/src/http_route_manifest.rs',
]) {
  const manifest = readText(relativePath);
  assert(
    manifest.includes('RateLimitTier::'),
    `${relativePath} must materialize rate limit tiers for web-framework enforcement`,
  );
}

const requiredSkeletonPaths = [
  'apis/README.md',
  'apis/authority-manifest.json',
  'apis/open-api/intelligence/memory/README.md',
  'apis/app-api/intelligence/memory/README.md',
  'apis/backend-api/intelligence/memory/README.md',
  'apis/rpc/README.md',
  'deployments/docker/README.md',
  'deployments/kubernetes/README.md',
  'deployments/runbooks/README.md',
  'etc/README.md',
  'scripts/README.md',
  'apps/README.md',
  'specs/topology.spec.json',
];

for (const relativePath of requiredSkeletonPaths) {
  assert(
    fs.existsSync(path.join(repoRoot, relativePath)),
    `${relativePath} must exist per SDKWORK_WORKSPACE_SPEC.md skeleton`,
  );
}

// Initialization state: `baselineStrategy` is `baseline-plus-migrations`, so each declared
// engine commits one consolidated baseline and migrations/ holds only post-baseline deltas.
// Those deltas may legitimately be empty (DATABASE_FRAMEWORK_SPEC section 353). When a migration
// is present, its declared reversibility must agree with whether it ships a down file
// (DATABASE_SPEC section 586), so a down file is never required unconditionally.
for (const engine of ['postgres', 'sqlite']) {
  const baselinePath = path.join(
    repoRoot,
    `database/ddl/baseline/${engine}/0001_memory_baseline.sql`,
  );
  if (!fs.existsSync(baselinePath)) {
    failures.push(`database/ddl/baseline/${engine}/0001_memory_baseline.sql must exist`);
  }

  const migrationDir = path.join(repoRoot, 'database/migrations', engine);
  if (!fs.existsSync(migrationDir)) {
    failures.push(`database/migrations/${engine}/ must exist as the post-baseline change venue`);
    continue;
  }
  const migrationSqlFiles = fs
    .readdirSync(migrationDir)
    .filter((name) => name.endsWith('.sql'))
    .sort();
  for (const upFile of migrationSqlFiles.filter((name) => name.endsWith('.up.sql'))) {
    const upText = fs
      .readFileSync(path.join(migrationDir, upFile), 'utf8')
      .replace(/\r\n/gu, '\n');
    const reversible = /^-- reversible: (true|false)[ \t]*$/mu.exec(upText)?.[1];
    const hasDown = migrationSqlFiles.includes(upFile.replace(/\.up\.sql$/u, '.down.sql'));
    assert(
      reversible === 'true' || reversible === 'false',
      `database/migrations/${engine}/${upFile} must declare "-- reversible: true|false"`,
    );
    if (reversible === 'true') {
      assert(
        hasDown,
        `database/migrations/${engine}/${upFile} declares reversible: true and must ship its paired .down.sql`,
      );
    } else {
      assert(
        !hasDown,
        `database/migrations/${engine}/${upFile} declares reversible: false and must NOT ship a misleading .down.sql`,
      );
    }
  }

  const pluginMigrationDir = path.join(
    repoRoot,
    'plugins/sdkwork-memory-plugin-native-sql/migrations',
    engine,
  );
  if (fs.existsSync(pluginMigrationDir)) {
    assert(
      fs.readdirSync(pluginMigrationDir).every((name) => !name.endsWith('.sql')),
      `plugins/sdkwork-memory-plugin-native-sql/migrations/${engine} must not remain a second SQL authority`,
    );
  }
}

const forbiddenDirectUploadPatterns = [
  /std::fs::write/,
  /tokio::fs::write/,
  /aws_sdk_s3::/,
  /put_object\(/,
];
for (const relativePath of [
  'crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs',
  'crates/sdkwork-intelligence-memory-service/src/open_api.rs',
  'crates/sdkwork-memory-drive/src/uploader.rs',
]) {
  const source = readText(relativePath);
  for (const pattern of forbiddenDirectUploadPatterns) {
    assert(
      !pattern.test(source),
      `${relativePath} must route file uploads through sdkwork-memory-drive, not direct storage APIs (${pattern})`,
    );
  }
}

assert(
  readText('crates/sdkwork-intelligence-memory-service/src/open_api.rs').includes(
    'sensitivity_read_scope',
  ),
  'open-api memory list must apply SQL-backed sensitivity read scope before pagination',
);
assert(
  readText('plugins/sdkwork-memory-plugin-native-sql/src/store.rs').includes(
    'RECORD_SENSITIVITY_FILTER_SQL',
  ),
  'native SQL store must filter sensitivity tiers in list_record_details queries',
);

if (failures.length > 0) {
  process.stderr.write(
    `Architecture alignment failed:\n${failures.map((failure) => `- ${failure}`).join('\n')}\n`,
  );
  if (warnings.length > 0) {
    process.stderr.write(
      `Warnings:\n${warnings.map((warning) => `- ${warning}`).join('\n')}\n`,
    );
  }
  process.exit(1);
}

if (warnings.length > 0) {
  process.stdout.write(
    `Architecture alignment passed with warnings:\n${warnings.map((warning) => `- ${warning}`).join('\n')}\n`,
  );
} else {
  process.stdout.write('Architecture alignment passed\n');
}
