#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createReadStream, existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { validateProcessSharedDatabasePool } from "../../sdkwork-specs/tools/check-process-shared-database-pool.mjs";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const strict = process.env.SDKWORK_RELEASE_VALIDATION === "strict";
const failures = [];
const warnings = [];

const manifest = readJson("agent.app.config.json");
const workflow = readJson("agent.workflow.json");
const dockerfile = readText("deployments/docker/Dockerfile");
const deployment = readText("deployments/kubernetes/deployment.yaml");
const migrationJob = readText("deployments/kubernetes/migration-job.yaml");
const processPoolValidation = validateProcessSharedDatabasePool(root);

if (manifest?.schemaVersion !== 3 || manifest?.kind !== "agent.app") {
  fail("agent.app.config.json must use schemaVersion 3 and kind agent.app");
}

const publishStatus = manifest?.publish?.status ?? "UNKNOWN";
const contractState = manifest?.artifacts?.installConfig?.metadata?.contractState ?? "unknown";
const productionClaimed = publishStatus === "ACTIVE" || contractState === "production-ready";
const packages = manifest?.artifacts?.installConfig?.packages ?? [];
const enabledPackages = packages.filter((pkg) => pkg.enabled !== false);
const targets = new Map((workflow?.targets ?? []).map((target) => [target.id, target]));

requireExecutableReleaseLifecycle(workflow);
requireCanonicalDockerfile(dockerfile);
requireContainerTarget(targets);
rejectFalseCandidateClaims();
requireProductionDatabasePoolContract();

if (strict) {
  if (!productionClaimed || publishStatus !== "ACTIVE" || contractState !== "production-ready") {
    fail("strict release validation requires ACTIVE publication and production-ready contract state");
  }
  if (enabledPackages.length === 0) {
    fail("strict release validation requires at least one enabled immutable package");
  }
  for (const pkg of enabledPackages) await validateEnabledPackage(pkg);
  rejectDeploymentPlaceholders();
} else if (!productionClaimed) {
  if (enabledPackages.length > 0) {
    fail("release-candidate manifests must keep publication packages disabled until immutable evidence exists");
  }
  warnings.push("release candidate is correctly blocked from production publication");
}

report();

function requireExecutableReleaseLifecycle(config) {
  if (config?.security?.sbomRequired !== true || config?.security?.signingRequired !== true) {
    fail("workflow must require both SBOM generation and artifact signing");
  }
  const signCommands = (config?.lifecycle?.sign ?? []).map((step) => step.run ?? "");
  const sbomCommands = (config?.lifecycle?.sbom ?? []).map((step) => step.run ?? "");
  if (!signCommands.some((command) => command.includes("workflow-supply-chain-evidence.mjs sign"))) {
    fail("workflow sign phase must execute the Memory detached-signature producer");
  }
  if (!sbomCommands.some((command) => command.includes("workflow-supply-chain-evidence.mjs attest"))) {
    fail("workflow SBOM phase must create byte-bound SBOM, provenance, checksum, and artifact evidence");
  }
}

function requireCanonicalDockerfile(value) {
  for (const requiredText of [
    "cargo build --release --locked -p sdkwork-api-memory-standalone-gateway --features otel",
    'RUN test "$(od -An -tx1 -N4 target/release/sdkwork-api-memory-standalone-gateway',
    'RUN test "$(od -An -tx1 -N4 /usr/local/bin/sdkwork-api-memory-standalone-gateway',
    "/workspace/sdkwork-memory/target/release/sdkwork-api-memory-standalone-gateway",
    "USER 10001:10001",
    'CMD ["sdkwork-api-memory-standalone-gateway"]',
  ]) {
    if (!value.includes(requiredText)) fail(`Dockerfile is missing canonical runtime declaration: ${requiredText}`);
  }
  if (value.includes("sdkwork-memory-standalone-gateway")) {
    fail("Dockerfile still references the retired nonexistent gateway package");
  }
}

function requireContainerTarget(targetMap) {
  const target = targetMap.get("container-x64-cloud-container-oci");
  if (
    !target
    || target.profileBinding !== "fixed"
    || target.deploymentProfile !== "cloud"
    || target.runtimeTarget !== "container"
    || target.profile !== "container"
    || target.platform !== "container"
    || !target.formats?.includes("oci")
    || !target.artifactPath
  ) {
    fail("workflow must define a byte-bound container-x64-cloud-container-oci target");
  }
}

function rejectFalseCandidateClaims() {
  const serialized = JSON.stringify(manifest);
  if (!productionClaimed && /Production Ready|production-ready Memory service/u.test(serialized)) {
    fail("release-candidate manifest still contains production-ready marketing claims");
  }
  for (const pkg of packages) {
    if (pkg.enabled === false && /@sha256:[a-f0-9]{64}$/u.test(pkg.url ?? "")) {
      fail(`disabled candidate package ${pkg.id} must not retain a stale immutable digest claim`);
    }
  }
}

function requireProductionDatabasePoolContract() {
  if (processPoolValidation.ok) return;
  const messages = processPoolValidation.failures
    .map((failure) => `process-shared database pool: ${failure}`);
  if (strict || productionClaimed) {
    for (const message of messages) fail(message);
  } else {
    warnings.push(...messages);
  }
}

async function validateEnabledPackage(pkg) {
  const target = targets.get(pkg.id);
  if (!target) {
    fail(`enabled package ${pkg.id} has no matching workflow target`);
    return;
  }
  if (!/^oci:\/\/[^\s]+@sha256:[a-f0-9]{64}$/u.test(pkg.url ?? "")) {
    fail(`enabled container package ${pkg.id} must use an immutable oci://...@sha256 URL`);
  }
  if (!/^[a-f0-9]{64}$/u.test(pkg.checksum ?? "")) {
    fail(`enabled package ${pkg.id} must declare a 64-character artifact checksum`);
  }
  const evidencePath = join(root, ".sdkwork", "evidence", `${pkg.id}.json`);
  if (!existsSync(evidencePath)) {
    fail(`enabled package ${pkg.id} is missing canonical artifact evidence`);
    return;
  }
  const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
  const artifactPath = join(root, evidence.artifactPath ?? "");
  if (!existsSync(artifactPath)) {
    fail(`artifact evidence for ${pkg.id} references a missing artifact`);
    return;
  }
  const digest = await sha256File(artifactPath);
  if (evidence.digest?.value !== digest && evidence.digest !== `sha256:${digest}`) {
    fail(`artifact evidence for ${pkg.id} does not match artifact bytes`);
  }
  if (evidence.sourceCommit !== gitHead()) {
    fail(`artifact evidence for ${pkg.id} is stale for the current source commit`);
  }
  for (const key of ["sbom", "provenance", "signature"]) {
    if (!evidence[key]) fail(`artifact evidence for ${pkg.id} is missing ${key}`);
  }
}

function rejectDeploymentPlaceholders() {
  for (const [path, value] of [
    ["deployments/kubernetes/deployment.yaml", deployment],
    ["deployments/kubernetes/migration-job.yaml", migrationJob],
  ]) {
    if (/<release-build-digest>|:latest@sha256:/u.test(value)) {
      fail(`${path} must reference the approved immutable release digest without placeholders or latest tags`);
    }
  }
}

function readJson(path) {
  const value = readText(path);
  if (!value) return null;
  try {
    return JSON.parse(value);
  } catch (error) {
    fail(`${path} is not valid JSON: ${error.message}`);
    return null;
  }
}

function readText(path) {
  const absolutePath = join(root, path);
  if (!existsSync(absolutePath)) {
    fail(`${path} must exist`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function gitHead() {
  return execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
}

function sha256File(path) {
  return new Promise((resolveDigest, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(path);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolveDigest(hash.digest("hex")));
  });
}

function fail(message) {
  failures.push(message);
}

function report() {
  for (const warning of warnings) console.warn(`[release-readiness] warning: ${warning}`);
  for (const failure of failures) console.error(`[release-readiness] error: ${failure}`);
  if (failures.length > 0) {
    console.error(`[release-readiness] failed (${failures.length} error(s))`);
    process.exit(1);
  }
  console.log(`[release-readiness] passed (${strict ? "strict production" : "candidate"} mode)`);
}
