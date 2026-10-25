#!/usr/bin/env node
/**
 * Package SDKWork Memory server release artifacts for workflow targets.
 */
import { execFileSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
if (process.platform !== "linux" || process.arch !== "x64") {
  throw new Error("the linux-x64 standalone archive must be packaged on a linux x64 runner");
}
const version =
  process.env.SDKWORK_PACKAGE_VERSION
  ?? JSON.parse(readFileSync(join(root, "agent.app.config.json"), "utf8")).release
    ?.currentVersion
  ?? "0.1.0";
const artifactRoot = join(root, "deployments", "artifacts", "release");
const stagingDir = join(artifactRoot, `sdkwork-memory-${version}-linux-x64-standalone-server`);
const archivePath = resolve(
  root,
  process.env.SDKWORK_PACKAGE_ARTIFACT_PATH
    ?? "deployments/artifacts/release/sdkwork-memory-linux-x64-standalone-server.tar.gz",
);

rmSync(stagingDir, { recursive: true, force: true });
mkdirSync(stagingDir, { recursive: true });

execFileSync("cargo", [
  "build",
  "--release",
  "--locked",
  "-p",
  "sdkwork-api-memory-standalone-gateway",
  "--features",
  "otel",
], {
  cwd: root,
  stdio: "inherit",
});
const releaseBinary = (() => {
  const candidate = join(root, "target", "release", "sdkwork-api-memory-standalone-gateway");
  if (existsSync(candidate)) return candidate;
  throw new Error("release binary not found after cargo build");
})();
cpSync(releaseBinary, join(stagingDir, "sdkwork-api-memory-standalone-gateway"));
cpSync(join(root, "database"), join(stagingDir, "database"), { recursive: true });

writeFileSync(
  join(stagingDir, "release-manifest.json"),
  `${JSON.stringify(
    {
      appId: "sdkwork-memory",
      version,
      platform: "linux",
      architecture: "x64",
      deploymentProfile: "standalone",
      runtimeTarget: "server",
      binary: "sdkwork-api-memory-standalone-gateway",
      databaseRoot: "database",
      generatedAt: new Date().toISOString(),
    },
    null,
    2,
  )}\n`,
  "utf8",
);

rmSync(archivePath, { force: true });
execFileSync("tar", [
  "-czf",
  archivePath,
  "-C",
  artifactRoot,
  `sdkwork-memory-${version}-linux-x64-standalone-server`,
], {
  cwd: root,
  stdio: "inherit",
});

console.log(`Packaged ${archivePath}`);
