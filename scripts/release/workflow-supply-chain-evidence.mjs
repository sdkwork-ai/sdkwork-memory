#!/usr/bin/env node

import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign as signBytes,
  verify as verifyBytes,
} from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const repositoryRoot = resolve(dirname(modulePath), "..", "..");
const signatureDomain = "sdkwork-artifact-sha256:";

export async function signReleaseArtifact({ env = process.env, root = repositoryRoot } = {}) {
  const paths = resolveEvidencePaths(env, root);
  const privateKey = loadSigningKey(env);
  const digest = await sha256File(paths.artifactPath);
  const message = Buffer.from(`${signatureDomain}${digest}`, "utf8");
  const algorithm = ["ed25519", "ed448"].includes(privateKey.asymmetricKeyType) ? null : "sha256";
  const signature = signBytes(algorithm, message, privateKey);
  const publicKey = createPublicKey(privateKey);
  const publicKeyPem = publicKey.export({ format: "pem", type: "spki" }).toString();
  const publicKeyFingerprint = createHash("sha256")
    .update(publicKey.export({ format: "der", type: "spki" }))
    .digest("hex");
  const envelope = {
    schemaVersion: 1,
    kind: "sdkwork.artifact.detached-signature",
    algorithm: privateKey.asymmetricKeyType,
    hashAlgorithm: "SHA-256",
    signedMessage: `${signatureDomain}${digest}`,
    artifact: paths.artifactRelativePath,
    digest: `sha256:${digest}`,
    publicKeyFingerprint: `sha256:${publicKeyFingerprint}`,
    publicKeyPem,
    signatureBase64: signature.toString("base64"),
  };
  mkdirSync(paths.evidenceRoot, { recursive: true });
  writeFileSync(paths.signaturePath, `${JSON.stringify(envelope, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
  });
  return { ...paths, digest: `sha256:${digest}`, envelope };
}

export async function createSbomProvenanceAndEvidence({
  env = process.env,
  root = repositoryRoot,
  sourceCommit = gitHead(root),
  evidenceWriter = writeWorkflowEvidence,
  packageInventory = cargoPackages,
} = {}) {
  const paths = resolveEvidencePaths(env, root);
  if (!existsSync(paths.signaturePath)) {
    throw new Error(`detached signature is missing: ${paths.signaturePath}`);
  }
  const digest = await sha256File(paths.artifactPath);
  verifySignatureEnvelope(paths.signaturePath, digest, paths.artifactRelativePath);
  const version = required(env.SDKWORK_PACKAGE_VERSION, "SDKWORK_PACKAGE_VERSION");
  const runtimeTarget = required(env.SDKWORK_RUNTIME_TARGET, "SDKWORK_RUNTIME_TARGET");
  const deploymentProfile = required(env.SDKWORK_DEPLOYMENT_PROFILE, "SDKWORK_DEPLOYMENT_PROFILE");
  const sizeBytes = statSync(paths.artifactPath).size;
  const packages = packageInventory(root);
  const sbom = {
    spdxVersion: "SPDX-2.3",
    dataLicense: "CC0-1.0",
    SPDXID: "SPDXRef-DOCUMENT",
    name: `${paths.packageId}-${version}`,
    documentNamespace: `https://sdkwork.com/apps/sdkwork-memory/sbom/${version}/${paths.packageId}/${digest}`,
    creationInfo: {
      created: new Date().toISOString(),
      creators: ["Tool: scripts/release/workflow-supply-chain-evidence.mjs"],
    },
    packages: [
      {
        name: paths.packageId,
        versionInfo: version,
        SPDXID: "SPDXRef-Artifact",
        downloadLocation: "NOASSERTION",
        filesAnalyzed: false,
        checksums: [{ algorithm: "SHA256", checksumValue: digest }],
        externalRefs: [{
          referenceCategory: "OTHER",
          referenceType: "sdkwork-runtime-target",
          referenceLocator: `${deploymentProfile}/${runtimeTarget}`,
        }],
      },
      ...packages.map((pkg, index) => ({
        name: pkg.name,
        versionInfo: pkg.version,
        SPDXID: `SPDXRef-Cargo-${index + 1}`,
        downloadLocation: pkg.source ?? "NOASSERTION",
        filesAnalyzed: false,
        licenseConcluded: pkg.license ?? "NOASSERTION",
      })),
    ],
  };
  const provenance = {
    _type: "https://in-toto.io/Statement/v1",
    subject: [{ name: paths.artifactRelativePath, digest: { sha256: digest } }],
    predicateType: "https://slsa.dev/provenance/v1",
    predicate: {
      buildDefinition: {
        buildType: "https://sdkwork.com/buildtypes/github-workflow/v1",
        externalParameters: {
          packageId: paths.packageId,
          deploymentProfile,
          runtimeTarget,
          version,
        },
        internalParameters: { sourceCommit },
        resolvedDependencies: [{
          uri: "git+https://github.com/sdkwork-ai/sdkwork-memory",
          digest: { gitCommit: sourceCommit },
        }],
      },
      runDetails: {
        builder: { id: "https://github.com/sdkwork-ai/sdkwork-github-workflow" },
        metadata: { invocationId: String(env.GITHUB_RUN_ID ?? "local-validation") },
      },
    },
  };
  const checksums = {
    schemaVersion: 1,
    artifact: paths.artifactRelativePath,
    packageId: paths.packageId,
    sizeBytes,
    algorithm: "SHA-256",
    digest,
    sourceCommit,
  };
  mkdirSync(paths.evidenceRoot, { recursive: true });
  writeFileSync(paths.sbomPath, `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
  writeFileSync(paths.provenancePath, `${JSON.stringify(provenance)}\n`, "utf8");
  writeFileSync(paths.checksumPath, `${JSON.stringify(checksums, null, 2)}\n`, "utf8");
  evidenceWriter({ env, paths, root, sourceCommit });
  return { ...paths, digest: `sha256:${digest}` };
}

export async function verifyReleaseEvidence({
  env = process.env,
  root = repositoryRoot,
  sourceCommit = gitHead(root),
} = {}) {
  const paths = resolveEvidencePaths(env, root);
  for (const evidencePath of [paths.signaturePath, paths.sbomPath, paths.provenancePath, paths.checksumPath]) {
    if (!existsSync(evidencePath)) throw new Error(`release evidence is missing: ${evidencePath}`);
  }
  const digest = await sha256File(paths.artifactPath);
  verifySignatureEnvelope(paths.signaturePath, digest, paths.artifactRelativePath);
  const checksum = JSON.parse(readFileSync(paths.checksumPath, "utf8"));
  if (checksum.digest !== digest || checksum.sourceCommit !== sourceCommit) {
    throw new Error("checksum evidence does not match the artifact bytes and current source commit");
  }
  const provenance = JSON.parse(readFileSync(paths.provenancePath, "utf8"));
  if (provenance.subject?.[0]?.digest?.sha256 !== digest) {
    throw new Error("provenance subject does not match the artifact digest");
  }
  return { ...paths, digest: `sha256:${digest}` };
}

function resolveEvidencePaths(env, root) {
  const packageId = required(env.SDKWORK_PACKAGE_ID, "SDKWORK_PACKAGE_ID");
  const artifactRelativePath = safeRelativePath(
    required(env.SDKWORK_PACKAGE_ARTIFACT_PATH, "SDKWORK_PACKAGE_ARTIFACT_PATH"),
    "SDKWORK_PACKAGE_ARTIFACT_PATH",
  );
  const artifactPath = resolve(root, artifactRelativePath);
  if (!existsSync(artifactPath)) throw new Error(`artifact does not exist: ${artifactPath}`);
  const evidenceRoot = resolve(root, ".sdkwork", "evidence", packageId);
  return {
    packageId,
    artifactPath,
    artifactRelativePath: portable(artifactRelativePath),
    evidenceRoot,
    signaturePath: resolve(evidenceRoot, "artifact.sig.json"),
    sbomPath: resolve(evidenceRoot, "sbom.spdx.json"),
    provenancePath: resolve(evidenceRoot, "provenance.intoto.jsonl"),
    checksumPath: resolve(evidenceRoot, "checksums.json"),
  };
}

function loadSigningKey(env) {
  const inline = String(env.SDKWORK_RELEASE_SIGNING_PRIVATE_KEY ?? "").trim();
  const file = String(env.SDKWORK_RELEASE_SIGNING_KEY_FILE ?? "").trim();
  if ((!inline && !file) || (inline && file)) {
    throw new Error("configure exactly one real release signing private key source");
  }
  if (file && !existsSync(file)) throw new Error(`signing key file does not exist: ${file}`);
  return createPrivateKey({
    key: inline || readFileSync(file),
    passphrase: String(env.SDKWORK_RELEASE_SIGNING_PRIVATE_KEY_PASSWORD ?? "").trim() || undefined,
  });
}

function verifySignatureEnvelope(signaturePath, digest, artifactRelativePath) {
  const envelope = JSON.parse(readFileSync(signaturePath, "utf8"));
  const expectedMessage = `${signatureDomain}${digest}`;
  if (
    envelope.digest !== `sha256:${digest}`
    || envelope.signedMessage !== expectedMessage
    || envelope.artifact !== artifactRelativePath
  ) {
    throw new Error("detached signature envelope does not match the artifact");
  }
  const algorithm = ["ed25519", "ed448"].includes(envelope.algorithm) ? null : "sha256";
  const verified = verifyBytes(
    algorithm,
    Buffer.from(expectedMessage, "utf8"),
    createPublicKey(envelope.publicKeyPem),
    Buffer.from(envelope.signatureBase64, "base64"),
  );
  if (!verified) throw new Error("detached artifact signature verification failed");
}

function cargoPackages(root) {
  const metadata = JSON.parse(execFileSync(
    "cargo",
    ["metadata", "--format-version=1", "--locked", "--no-deps"],
    { cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
  ));
  return metadata.packages
    .map((pkg) => ({ name: pkg.name, version: pkg.version, license: pkg.license, source: pkg.source }))
    .sort((left, right) => `${left.name}@${left.version}`.localeCompare(`${right.name}@${right.version}`));
}

function writeWorkflowEvidence({ env, paths, root, sourceCommit }) {
  const cli = required(env.SDKWORK_WORKFLOW_CLI, "SDKWORK_WORKFLOW_CLI");
  const outputs = required(env.SDKWORK_ARTIFACT_EVIDENCE_PATHS, "SDKWORK_ARTIFACT_EVIDENCE_PATHS")
    .split(/\r?\n/u)
    .map((value) => value.trim())
    .filter(Boolean);
  for (const output of outputs) {
    const args = [
      cli,
      "evidence:create",
      "--config",
      "sdkwork.workflow.json",
      "--target-id",
      required(env.SDKWORK_PACKAGE_TARGET_ID, "SDKWORK_PACKAGE_TARGET_ID"),
      "--deployment-profile",
      required(env.SDKWORK_DEPLOYMENT_PROFILE, "SDKWORK_DEPLOYMENT_PROFILE"),
      "--version",
      required(env.SDKWORK_PACKAGE_VERSION, "SDKWORK_PACKAGE_VERSION"),
      "--source-commit",
      sourceCommit,
      "--artifact-id",
      paths.packageId,
      "--artifact",
      paths.artifactRelativePath,
      "--artifact-evidence",
      safeRelativePath(output, "SDKWORK_ARTIFACT_EVIDENCE_PATHS"),
      "--sbom",
      portable(relative(root, paths.sbomPath)),
      "--provenance",
      portable(relative(root, paths.provenancePath)),
      "--signature",
      portable(relative(root, paths.signaturePath)),
    ];
    const result = spawnSync(process.execPath, args, { cwd: root, env, stdio: "inherit" });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(`artifact evidence creation failed with exit code ${result.status ?? 1}`);
    }
  }
}

function safeRelativePath(value, label) {
  if (isAbsolute(value) || value.split(/[\\/]/u).includes("..")) {
    throw new Error(`${label} must be a safe repository-relative path`);
  }
  return value;
}

function required(value, label) {
  const text = String(value ?? "").trim();
  if (!text) throw new Error(`${label} is required`);
  return text;
}

function gitHead(root) {
  return execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
}

function portable(value) {
  return value.split(sep).join("/");
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

async function main([command] = process.argv.slice(2)) {
  if (command === "sign") await signReleaseArtifact();
  else if (command === "attest") await createSbomProvenanceAndEvidence();
  else if (command === "verify") await verifyReleaseEvidence();
  else throw new Error("command must be sign, attest, or verify");
}

if (process.argv[1] && resolve(process.argv[1]) === modulePath) {
  main().catch((error) => {
    console.error(`[sdkwork-memory-release-evidence] ${error.message}`);
    process.exitCode = 1;
  });
}
