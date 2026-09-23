import assert from "node:assert/strict";
import { createPublicKey, generateKeyPairSync, verify as verifyBytes } from "node:crypto";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  narrowTargets,
  packageTargetsForProfile,
  parseSignProfileArguments,
  resolveWorkflowCli,
  signProfileArtifacts,
  SUPPORTED_DEPLOYMENT_PROFILES,
} from "./sign-release-profile.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const CONFIG_NAME = "sdkwork.workflow.json";

function signingEnv(privateKeyPem) {
  return { SDKWORK_RELEASE_SIGNING_PRIVATE_KEY: privateKeyPem };
}

function privateKeyPem() {
  return generateKeyPairSync("ed25519").privateKey.export({ format: "pem", type: "pkcs8" }).toString();
}

/**
 * Cryptographically re-verifies the envelope this repository writes, independently of the
 * producer's own helper, so a passing test means the detached signature really binds the
 * artifact bytes.
 */
function assertEnvelopeSignsArtifact({ root, signaturePath, artifactPath }) {
  const envelope = JSON.parse(readFileSync(signaturePath, "utf8"));
  const digest = createHash("sha256").update(readFileSync(artifactPath)).digest("hex");
  assert.equal(envelope.digest, `sha256:${digest}`);
  assert.equal(envelope.kind, "sdkwork.artifact.detached-signature");
  const verified = verifyBytes(
    null,
    Buffer.from(envelope.signedMessage, "utf8"),
    createPublicKey(envelope.publicKeyPem),
    Buffer.from(envelope.signatureBase64, "base64"),
  );
  assert.equal(verified, true, "detached signature must verify against the published public key");
  return envelope;
}

test("rejects an unsupported deployment profile and unknown arguments", () => {
  assert.throws(
    () => parseSignProfileArguments(["--deployment-profile", "private"]),
    /--deployment-profile must be one of standalone, cloud/u,
  );
  assert.throws(
    () => parseSignProfileArguments(["--deployment-profile", "standalone", "--force"]),
    /unknown argument: --force/u,
  );
  assert.throws(() => parseSignProfileArguments(["--deployment-profile"]), /is required/u);
  assert.deepEqual(
    parseSignProfileArguments(["--deployment-profile", "cloud"]),
    { deploymentProfile: "cloud", targetIds: [] },
  );
});

test("exposes exactly the deployment profiles the release authority supports", () => {
  assert.deepEqual([...SUPPORTED_DEPLOYMENT_PROFILES], ["standalone", "cloud"]);
});

test("fails when a requested target does not belong to the profile", () => {
  const targets = [{ packageId: "linux-x64-standalone-server-tar-gz" }];
  assert.throws(
    () => narrowTargets({ targets, targetIds: ["web-universal-cloud-browser-zip"], deploymentProfile: "standalone" }),
    /no package target web-universal-cloud-browser-zip/u,
  );
  assert.deepEqual(
    narrowTargets({ targets, targetIds: [], deploymentProfile: "standalone" }),
    targets,
  );
});

test("resolves real package targets for each profile from the canonical workflow config", () => {
  const standalone = packageTargetsForProfile({ deploymentProfile: "standalone", root: repositoryRoot });
  assert.ok(standalone.length >= 1, "standalone must select at least one packaged target");
  for (const target of standalone) {
    assert.equal(target.deploymentProfile, "standalone");
    assert.match(target.packageId, /^[a-z0-9-]+$/u);
    assert.ok(target.artifactPath.length > 0);
  }

  const cloud = packageTargetsForProfile({ deploymentProfile: "cloud", root: repositoryRoot });
  assert.ok(cloud.length >= 1, "cloud must select at least one packaged target");
  for (const target of cloud) {
    assert.equal(target.deploymentProfile, "cloud");
  }

  const ids = new Set([...standalone, ...cloud].map((target) => target.packageId));
  assert.equal(ids.size, standalone.length + cloud.length, "profiles must not select the same package id twice");
});

test("signs every artifact of a profile with a verifiable detached signature", async () => {
  const root = mkdtempSync(join(tmpdir(), "sdkwork-memory-sign-profile-"));
  try {
    copyFileSync(resolve(repositoryRoot, CONFIG_NAME), join(root, CONFIG_NAME));
    const targets = [
      {
        packageId: "linux-x64-standalone-server-tar-gz",
        artifactPath: "deployments/artifacts/release/standalone.tar.gz",
        deploymentProfile: "standalone",
        runtimeTarget: "server",
        evidencePath: ".sdkwork/evidence/linux-x64-standalone-server-tar-gz.json",
      },
      {
        packageId: "extra-x64-standalone-sidecar-zip",
        artifactPath: "deployments/artifacts/release/sidecar.zip",
        deploymentProfile: "standalone",
        runtimeTarget: "server",
        evidencePath: ".sdkwork/evidence/extra-x64-standalone-sidecar-zip.json",
      },
    ];
    for (const target of targets) {
      mkdirSync(dirname(resolve(root, target.artifactPath)), { recursive: true });
      writeFileSync(resolve(root, target.artifactPath), `artifact bytes for ${target.packageId}`);
    }

    const env = signingEnv(privateKeyPem());
    const signed = await signProfileArtifacts({ deploymentProfile: "standalone", root, env, targets });

    assert.equal(signed.length, targets.length);
    assert.deepEqual(
      signed.map((entry) => entry.packageId).sort(),
      targets.map((target) => target.packageId).sort(),
    );
    for (const target of targets) {
      const signaturePath = resolve(root, ".sdkwork", "evidence", target.packageId, "artifact.sig.json");
      const envelope = assertEnvelopeSignsArtifact({
        root,
        signaturePath,
        artifactPath: resolve(root, target.artifactPath),
      });
      assert.equal(envelope.artifact, target.artifactPath);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("refuses to sign when the selected artifact is absent", async () => {
  const root = mkdtempSync(join(tmpdir(), "sdkwork-memory-sign-profile-missing-"));
  try {
    copyFileSync(resolve(repositoryRoot, CONFIG_NAME), join(root, CONFIG_NAME));
    const targets = [
      {
        packageId: "missing-x64-standalone-server-tar-gz",
        artifactPath: "deployments/artifacts/release/missing.tar.gz",
        deploymentProfile: "standalone",
        runtimeTarget: "server",
        evidencePath: ".sdkwork/evidence/missing-x64-standalone-server-tar-gz.json",
      },
    ];
    await assert.rejects(
      signProfileArtifacts({ deploymentProfile: "standalone", root, env: signingEnv(privateKeyPem()), targets }),
      /artifact does not exist/u,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("refuses to sign without real key material", async () => {
  const root = mkdtempSync(join(tmpdir(), "sdkwork-memory-sign-profile-nokey-"));
  try {
    copyFileSync(resolve(repositoryRoot, CONFIG_NAME), join(root, CONFIG_NAME));
    const targets = [
      {
        packageId: "nokey-x64-standalone-server-tar-gz",
        artifactPath: "deployments/artifacts/release/nokey.tar.gz",
        deploymentProfile: "standalone",
        runtimeTarget: "server",
        evidencePath: ".sdkwork/evidence/nokey-x64-standalone-server-tar-gz.json",
      },
    ];
    mkdirSync(dirname(resolve(root, targets[0].artifactPath)), { recursive: true });
    writeFileSync(resolve(root, targets[0].artifactPath), "artifact bytes");
    await assert.rejects(
      signProfileArtifacts({ deploymentProfile: "standalone", root, env: {}, targets }),
      /configure exactly one real release signing private key source/u,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("honours SDKWORK_WORKFLOW_CLI and reports a usable error without it", () => {
  const missing = resolve(repositoryRoot, "does-not-exist", "sdkwork-workflow.mjs");
  assert.throws(
    () => resolveWorkflowCli({ env: { SDKWORK_WORKFLOW_CLI: missing }, root: repositoryRoot }),
    /SDKWORK_WORKFLOW_CLI does not exist/u,
  );
  assert.throws(
    () => resolveWorkflowCli({ env: {}, root: mkdtempSync(join(tmpdir(), "sdkwork-memory-sign-cli-")) }),
    /cannot locate the SDKWork workflow CLI/u,
  );
  const cli = resolveWorkflowCli({ env: {}, root: repositoryRoot });
  assert.match(cli, /sdkwork-workflow\.mjs$/u);
});
