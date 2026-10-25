#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { deflateRawSync } from "node:zlib";

const appRoot = resolve(import.meta.dirname, "..");
const workspaceRoot = resolve(appRoot, "..", "..");
const manifestPath = join(appRoot, "agent.app.config.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const version = manifest.release.currentVersion;
const packageId = process.env.SDKWORK_PACKAGE_ID ?? manifest.publish.defaultPackageId;
const distRoot = join(appRoot, "dist");
const outputRoot = join(workspaceRoot, "deployments", "artifacts", "pc");
const archiveName = "sdkwork-memory-pc-web-universal-cloud-browser.zip";
const archivePath = join(outputRoot, archiveName);
const releaseTimestamp = resolveReleaseTimestamp(manifest);

if (!statSync(distRoot, { throwIfNoEntry: false })?.isDirectory()) {
  throw new Error("Memory PC dist directory is missing; run `pnpm build` before packaging");
}

mkdirSync(outputRoot, { recursive: true });

const distFiles = listFiles(distRoot).map((path) => ({
  archivePath: `app/${toPosix(relative(distRoot, path))}`,
  bytes: readFileSync(path),
}));
const fileEvidence = distFiles.map((file) => ({
  path: file.archivePath,
  sha256: sha256(file.bytes),
  sizeBytes: file.bytes.length,
}));
const sourceCommit = readGitValue(["rev-parse", "HEAD"], "unknown");
const sourceTreeState = readGitValue(["status", "--porcelain"], "").trim() ? "dirty" : "clean";
const sbom = createSpdxSbom(version, releaseTimestamp);
const provenance = {
  schemaVersion: 1,
  kind: "sdkwork.browser-artifact.provenance",
  application: manifest.app.key,
  version,
  packageId,
  deploymentProfile: "cloud",
  runtimeTarget: "browser",
  sourceCommit,
  sourceTreeState,
  buildCommand: "pnpm --dir apps/sdkwork-memory-pc build",
  packageCommand: "pnpm --dir apps/sdkwork-memory-pc build:browser:cloud",
  generatedAt: releaseTimestamp,
  files: fileEvidence,
};
const releaseManifest = {
  schemaVersion: 1,
  application: manifest.app.key,
  version,
  packageId,
  platform: "WEB",
  targetPlatform: "web",
  architecture: "universal",
  clientArchitecture: "pc-web",
  deploymentProfile: "cloud",
  runtimeTarget: "browser",
  entrypoint: "app/index.html",
  sbom: "evidence/sbom.spdx.json",
  provenance: "evidence/provenance.json",
  generatedAt: releaseTimestamp,
};

const evidenceFiles = [
  jsonEntry("evidence/sbom.spdx.json", sbom),
  jsonEntry("evidence/provenance.json", provenance),
  jsonEntry("release-manifest.json", releaseManifest),
];
const archiveBytes = createDeterministicZip([...distFiles, ...evidenceFiles]);
writeFileSync(archivePath, archiveBytes);

const digest = sha256(archiveBytes);
const checksums = {
  schemaVersion: 1,
  generatedAt: releaseTimestamp,
  artifacts: [{
    packageId,
    path: toPosix(relative(workspaceRoot, archivePath)),
    algorithm: "SHA-256",
    digest,
    sizeBytes: archiveBytes.length,
  }],
};
writeFileSync(join(outputRoot, `${archiveName}.checksums.json`), `${JSON.stringify(checksums, null, 2)}\n`, "utf8");
writeFileSync(join(outputRoot, `${archiveName}.sbom.spdx.json`), `${JSON.stringify(sbom, null, 2)}\n`, "utf8");
writeFileSync(join(outputRoot, `${archiveName}.provenance.json`), `${JSON.stringify(provenance, null, 2)}\n`, "utf8");

console.log(JSON.stringify({ archive: archivePath, packageId, sha256: digest, sizeBytes: archiveBytes.length }, null, 2));

function createSpdxSbom(releaseVersion, created) {
  const dependencyTree = JSON.parse(execPnpm(["--dir", appRoot, "list", "--prod", "--json", "--depth", "Infinity"]));
  const packages = new Map();
  for (const root of dependencyTree) collectDependencies(root.dependencies ?? {}, packages);
  const sorted = [...packages.values()].sort((left, right) => `${left.name}@${left.version}`.localeCompare(`${right.name}@${right.version}`));
  return {
    spdxVersion: "SPDX-2.3",
    dataLicense: "CC0-1.0",
    SPDXID: "SPDXRef-DOCUMENT",
    name: `sdkwork-memory-pc-${releaseVersion}`,
    documentNamespace: `https://sdkwork.com/apps/sdkwork-memory-pc/sbom/${releaseVersion}`,
    creationInfo: { created, creators: ["Tool: apps/sdkwork-memory-pc/scripts/package-web-release.mjs"] },
    packages: [
      { name: "@sdkwork/memory-pc", versionInfo: releaseVersion, SPDXID: "SPDXRef-Root", downloadLocation: "NOASSERTION", filesAnalyzed: false },
      ...sorted.map((dependency, index) => ({
        name: dependency.name,
        versionInfo: dependency.version,
        SPDXID: `SPDXRef-Package-${index + 1}`,
        downloadLocation: dependency.resolved ?? "NOASSERTION",
        filesAnalyzed: false,
      })),
    ],
  };
}

function collectDependencies(dependencies, packages) {
  for (const [name, value] of Object.entries(dependencies)) {
    const version = resolveDependencyVersion(value);
    const key = `${name}@${version}`;
    if (!packages.has(key)) packages.set(key, { name, version, resolved: value.resolved });
    collectDependencies(value.dependencies ?? {}, packages);
  }
}

function resolveDependencyVersion(value) {
  if (typeof value.version === "string" && !value.version.startsWith("link:")) return value.version;
  if (typeof value.path === "string") {
    try {
      return JSON.parse(readFileSync(join(value.path, "package.json"), "utf8")).version ?? "0.0.0-workspace";
    } catch {
      return "0.0.0-workspace";
    }
  }
  return "0.0.0-unknown";
}

function createDeterministicZip(files) {
  const normalized = files
    .map((file) => ({ archivePath: toPosix(file.archivePath), bytes: Buffer.from(file.bytes) }))
    .sort((left, right) => left.archivePath.localeCompare(right.archivePath));
  const localParts = [];
  const centralParts = [];
  let offset = 0;
  for (const file of normalized) {
    const name = Buffer.from(file.archivePath, "utf8");
    const compressed = deflateRawSync(file.bytes, { level: 9 });
    const crc = crc32(file.bytes);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6);
    local.writeUInt16LE(8, 8);
    local.writeUInt16LE(0, 10);
    local.writeUInt16LE(33, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(compressed.length, 18);
    local.writeUInt32LE(file.bytes.length, 22);
    local.writeUInt16LE(name.length, 26);
    local.writeUInt16LE(0, 28);
    localParts.push(local, name, compressed);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(0x0314, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt16LE(0, 12);
    central.writeUInt16LE(33, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(compressed.length, 20);
    central.writeUInt32LE(file.bytes.length, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE((0o100644 << 16) >>> 0, 38);
    central.writeUInt32LE(offset, 42);
    centralParts.push(central, name);
    offset += local.length + name.length + compressed.length;
  }
  const centralSize = centralParts.reduce((size, part) => size + part.length, 0);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(normalized.length, 8);
  end.writeUInt16LE(normalized.length, 10);
  end.writeUInt32LE(centralSize, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...localParts, ...centralParts, end]);
}

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function listFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .flatMap((entry) => {
      const path = join(root, entry.name);
      return entry.isDirectory() ? listFiles(path) : entry.isFile() ? [path] : [];
    });
}

function jsonEntry(archivePath, value) {
  return { archivePath, bytes: Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8") };
}

function resolveReleaseTimestamp(appManifest) {
  const sourceDateEpoch = Number.parseInt(process.env.SOURCE_DATE_EPOCH ?? "", 10);
  if (Number.isSafeInteger(sourceDateEpoch) && sourceDateEpoch >= 315532800) {
    return new Date(sourceDateEpoch * 1000).toISOString();
  }
  return "1980-01-01T00:00:00.000Z";
}

function execPnpm(args) {
  return execFileSync("pnpm", args, {
    cwd: workspaceRoot,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    shell: process.platform === "win32",
  });
}

function readGitValue(args, fallback) {
  try {
    return execFileSync("git", args, { cwd: workspaceRoot, encoding: "utf8" }).trim();
  } catch {
    return fallback;
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function toPosix(path) {
  return path.split(sep).join("/");
}
