#!/usr/bin/env node

/**
 * Profile-aware local entry point for the Memory `release:sign` lifecycle phase.
 *
 * `PNPM_SCRIPT_SPEC.md` section 7 requires every profile-sensitive release phase to expose both a
 * `:standalone` and a `:cloud` variant. The signing phase is genuinely profile-sensitive because
 * a standalone server tarball and a cloud browser bundle are different immutable artifacts that
 * must each carry their own detached signature.
 *
 * This script keeps that obligation honest instead of cosmetic: it resolves the selected package
 * targets from the canonical `sdkwork.workflow.json` authority through the SDKWork workflow CLI
 * (the same authority the GitHub packaging matrix uses), then drives the shared
 * `signReleaseArtifact` producer for each target. Nothing about package ids, artifact paths, or
 * deployment profiles is re-derived here, so local signing cannot drift from CI packaging.
 */

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { signReleaseArtifact } from "./workflow-supply-chain-evidence.mjs";

const modulePath = fileURLToPath(import.meta.url);
const repositoryRoot = resolve(dirname(modulePath), "..", "..");

export const SUPPORTED_DEPLOYMENT_PROFILES = Object.freeze(["standalone", "cloud"]);
const CONFIG_RELATIVE_PATH = "sdkwork.workflow.json";

function required(value, label) {
  const text = String(value ?? "").trim();
  if (!text) throw new Error(`${label} is required`);
  return text;
}

/**
 * Parses `--flag value` pairs, rejecting anything the signing phase does not understand so a typo
 * cannot silently degrade into "sign the wrong thing".
 *
 * @param {string[]} argv arguments after the interpreter and script path
 * @returns {{ deploymentProfile: string, targetIds: string[] }}
 */
export function parseSignProfileArguments(argv) {
  let deploymentProfile = "";
  const targetIds = [];
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--deployment-profile") {
      deploymentProfile = required(argv[index + 1], "--deployment-profile value");
      index += 1;
      continue;
    }
    if (token === "--target") {
      targetIds.push(required(argv[index + 1], "--target value"));
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${token}`);
  }
  if (!SUPPORTED_DEPLOYMENT_PROFILES.includes(deploymentProfile)) {
    throw new Error(
      `--deployment-profile must be one of ${SUPPORTED_DEPLOYMENT_PROFILES.join(", ")}, found "${deploymentProfile}"`,
    );
  }
  return { deploymentProfile, targetIds };
}

/**
 * Locates the SDKWork workflow CLI. `SDKWORK_WORKFLOW_CLI` is how CI points at the pinned
 * framework checkout; a sibling checkout is the documented local workspace layout.
 *
 * @param {{ env?: NodeJS.ProcessEnv, root?: string }} [options]
 * @returns {string} absolute path to the workflow CLI entry point
 */
export function resolveWorkflowCli({ env = process.env, root = repositoryRoot } = {}) {
  const declared = String(env.SDKWORK_WORKFLOW_CLI ?? "").trim();
  if (declared) {
    if (!existsSync(declared)) {
      throw new Error(`SDKWORK_WORKFLOW_CLI does not exist: ${declared}`);
    }
    return declared;
  }
  const sibling = resolve(root, "..", "sdkwork-github-workflow", "scripts", "sdkwork-workflow.mjs");
  if (existsSync(sibling)) return sibling;
  throw new Error(
    "cannot locate the SDKWork workflow CLI; set SDKWORK_WORKFLOW_CLI or check out sdkwork-github-workflow beside this repository",
  );
}

function defaultWorkflowCliRunner(cli, args, root) {
  return execFileSync(process.execPath, [cli, ...args], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
}

/**
 * Reads the packaged targets for one deployment profile from the canonical workflow authority.
 *
 * @param {{
 *   deploymentProfile: string,
 *   root?: string,
 *   cli?: string,
 *   runner?: (cli: string, args: string[], root: string) => string,
 * }} options
 * @returns {Array<{ packageId: string, artifactPath: string, deploymentProfile: string, runtimeTarget: string, evidencePath: string }>}
 */
export function packageTargetsForProfile({
  deploymentProfile,
  root = repositoryRoot,
  cli = resolveWorkflowCli({ root }),
  runner = defaultWorkflowCliRunner,
}) {
  const raw = runner(
    cli,
    ["matrix", "--config", CONFIG_RELATIVE_PATH, "--deployment-profile", deploymentProfile, "--json"],
    root,
  );
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`workflow CLI did not return JSON for profile ${deploymentProfile}: ${error.message}`);
  }
  if (parsed?.ok !== true) {
    throw new Error(`workflow CLI refused profile ${deploymentProfile}: ${JSON.stringify(parsed?.issues ?? parsed)}`);
  }
  const include = parsed?.matrix?.include;
  if (!Array.isArray(include) || include.length === 0) {
    throw new Error(
      `sdkwork.workflow.json declares no package target for deployment profile "${deploymentProfile}"`,
    );
  }
  return include.map((item) => {
    const resolvedProfile = required(item.deploymentProfile, `target ${item.id}.deploymentProfile`);
    if (resolvedProfile !== deploymentProfile) {
      throw new Error(
        `workflow CLI returned target ${item.id} with profile ${resolvedProfile}, expected ${deploymentProfile}`,
      );
    }
    return {
      packageId: required(item.packageId, `target ${item.id}.packageId`),
      artifactPath: required(item.artifactPath, `target ${item.id}.artifactPath`),
      deploymentProfile: resolvedProfile,
      runtimeTarget: required(item.runtimeTarget, `target ${item.id}.runtimeTarget`),
      evidencePath: required(item.artifactEvidencePathsText, `target ${item.id}.artifactEvidencePaths`),
    };
  });
}

/**
 * Narrows resolved targets to an explicit `--target` selection and fails when a requested target
 * does not belong to the profile, so a mistyped id cannot sign nothing and still report success.
 *
 * @param {{ targets: Array<{ packageId: string }>, targetIds: string[], deploymentProfile: string }} options
 */
export function narrowTargets({ targets, targetIds, deploymentProfile }) {
  if (targetIds.length === 0) return targets;
  const byId = new Map(targets.map((target) => [target.packageId, target]));
  const missing = targetIds.filter((id) => !byId.has(id));
  if (missing.length > 0) {
    throw new Error(
      `deployment profile "${deploymentProfile}" has no package target ${missing.join(", ")}; available: ${targets
        .map((target) => target.packageId)
        .join(", ")}`,
    );
  }
  return targetIds.map((id) => byId.get(id));
}

/**
 * Signs every selected target with the shared detached-signature producer. The signing key
 * material, custody rules, and evidence layout are owned by `SUPPLY_CHAIN_SECURITY_SPEC.md`
 * section 5 and are not re-specified here.
 *
 * @param {{
 *   deploymentProfile: string,
 *   targetIds?: string[],
 *   env?: NodeJS.ProcessEnv,
 *   root?: string,
 *   targets?: Array<object>,
 *   signer?: typeof signReleaseArtifact,
 * }} options
 * @returns {Promise<Array<{ packageId: string, signaturePath: string, digest: string }>>}
 */
export async function signProfileArtifacts({
  deploymentProfile,
  targetIds = [],
  env = process.env,
  root = repositoryRoot,
  targets = packageTargetsForProfile({ deploymentProfile, root }),
  signer = signReleaseArtifact,
}) {
  const selected = narrowTargets({ targets, targetIds, deploymentProfile });
  const signed = [];
  for (const target of selected) {
    const targetEnv = {
      ...env,
      SDKWORK_PACKAGE_ID: target.packageId,
      SDKWORK_PACKAGE_TARGET_ID: target.packageId,
      SDKWORK_PACKAGE_ARTIFACT_PATH: target.artifactPath,
      SDKWORK_DEPLOYMENT_PROFILE: target.deploymentProfile,
      SDKWORK_RUNTIME_TARGET: target.runtimeTarget,
      SDKWORK_ARTIFACT_EVIDENCE_PATHS: target.evidencePath,
    };
    const result = await signer({ env: targetEnv, root });
    signed.push({
      packageId: result.packageId,
      signaturePath: result.signaturePath,
      digest: result.digest,
    });
  }
  return signed;
}

async function main(argv) {
  const { deploymentProfile, targetIds } = parseSignProfileArguments(argv);
  const signed = await signProfileArtifacts({ deploymentProfile, targetIds });
  for (const entry of signed) {
    console.log(
      `[sdkwork-memory-release-sign] ${deploymentProfile} ${entry.packageId} -> ${entry.digest}`,
    );
  }
}

if (process.argv[1] && resolve(process.argv[1]) === modulePath) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(`[sdkwork-memory-release-sign] ${error.message}`);
    process.exitCode = 1;
  });
}
