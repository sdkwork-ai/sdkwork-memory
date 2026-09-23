#!/usr/bin/env node
//
// check-crate-inventory-standard.mjs
//
// Validates that every Rust crate directory in this repository is actually
// reachable by the build and by every other gate.
//
// Why this gate exists
// -------------------
// QUALITY_GATE_SPEC.md section 30 (Reachability) forbids a gate that reports
// success while examining nothing. This gate closes the mirror-image hole on the
// *input* side: a crate directory that exists on disk, is committed to git, and
// yet is invisible to `cargo metadata`, `cargo test --workspace`,
// `check-rust-crate-naming-standard.mjs`, and every other checker — because it
// has no `Cargo.toml` or is absent from the root `[workspace] members` list.
//
// That hole was not hypothetical: `crates/sdkwork-memory-mem0/` shipped 39
// committed files / 5740 lines of Rust, including P0 data-safety defects, and no
// gate in the suite could see it. See
// `docs/engineering/reviews/REVIEW-20260923-memory-mem0-implementation-audit.md`
// sections 1.1-1.3.
//
// Rules
// -----
// R1  Every first-level directory under `crates/` and `plugins/` must contain a
//     `Cargo.toml`.
// R2  That manifest's `[package].name` must appear in the root `Cargo.toml`
//     `[workspace] members` list.
// R3  Every path in `[workspace] members` must exist on disk and contain a
//     `Cargo.toml` (no dangling members).
//
// Scanning zero crate directories is itself a failure: an empty scan is
// indistinguishable from a passing one, which is exactly what section 31
// (Fail closed) prohibits.
//
// Usage:
//   node tools/check-crate-inventory-standard.mjs
//   node tools/check-crate-inventory-standard.mjs --root <path>
//   node tools/check-crate-inventory-standard.mjs --json
//
// Exit codes: 0 = clean, 1 = violations found or the scan examined nothing.

import { readdirSync, readFileSync, existsSync, statSync } from 'node:fs';
import { join, resolve, basename, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const getArg = (name, fallback = null) => {
  const index = args.findIndex((a) => a === `--${name}` || a.startsWith(`--${name}=`));
  if (index < 0) return fallback;
  const hit = args[index];
  if (hit.includes('=')) return hit.slice(hit.indexOf('=') + 1);
  const next = args[index + 1];
  return next && !next.startsWith('--') ? next : fallback;
};
const AS_JSON = args.includes('--json');

const repoRoot = getArg('root', resolve(dirname(fileURLToPath(import.meta.url)), '..'));

/** Directories that hold Rust crates but are legitimately absent from members. */
const SKIP_DIR_NAMES = new Set([
  'node_modules',
  'target',
  '.git',
  'external',
  'dist',
  'build',
  'tests',
  'src',
  'benches',
  'examples',
]);

/**
 * Minimal TOML reader: enough to pull `[workspace] members` and `[package] name`
 * out of a Cargo manifest. Deliberately not a general TOML parser — this gate
 * must run with zero dependencies.
 *
 * @param {string} text raw manifest contents
 * @returns {{ sections: Map<string, string[]>, text: string }} parsed view
 */
function parseManifest(text) {
  const sections = new Map();
  let current = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const header = line.match(/^\[{1,2}([^\]]+)\]{1,2}$/);
    if (header) {
      current = header[1];
      if (!sections.has(current)) sections.set(current, []);
      continue;
    }
    if (current) sections.get(current).push(line);
  }
  return { sections, text };
}

/**
 * Reads a `key = "value"` scalar from a named section.
 *
 * @param {ReturnType<typeof parseManifest>} manifest parsed manifest
 * @param {string} sectionName section header, without brackets
 * @param {string} key scalar key
 * @returns {string|null} the value, or null when absent
 */
function scalar(manifest, sectionName, key) {
  const lines = manifest.sections.get(sectionName);
  if (!lines) return null;
  for (const line of lines) {
    const match = line.match(new RegExp(`^${key}\\s*=\\s*(.+)$`));
    if (match) return match[1].trim().replace(/^"|"$/g, '');
  }
  return null;
}

/**
 * Reads a `key = [...]` string array from a named section, tolerating entries
 * split across multiple lines.
 *
 * @param {ReturnType<typeof parseManifest>} manifest parsed manifest
 * @param {string} sectionName section header, without brackets
 * @param {string} key array key
 * @returns {string[]} declared entries in source order
 */
function arrayOfStrings(manifest, sectionName, key) {
  const lines = manifest.sections.get(sectionName);
  if (!lines) return [];
  const joined = lines.join('\n');
  const match = joined.match(new RegExp(`^${key}\\s*=\\s*\\[([\\s\\S]*?)\\]`, 'm'));
  if (!match) return [];
  return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/**
 * Expands a Cargo workspace member entry into concrete repository-relative
 * directories. Bare paths pass through; a single trailing `/*` expands to every
 * matching child directory, which is how Cargo itself treats the pattern.
 *
 * @param {string} entry a `members` entry
 * @returns {string[]} repository-relative member directories
 */
function expandMember(entry) {
  const normalized = entry.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!normalized.includes('*')) return [normalized];
  const parent = normalized.slice(0, normalized.lastIndexOf('*')).replace(/\/+$/, '');
  const absoluteParent = join(repoRoot, parent);
  if (!existsSync(absoluteParent)) return [];
  return readdirSync(absoluteParent, { withFileTypes: true })
    .filter((entryDirent) => entryDirent.isDirectory())
    .map((entryDirent) => `${parent}/${entryDirent.name}`);
}

/**
 * Lists first-level crate directories under a top-level container directory.
 *
 * @param {string} container repository-relative container, e.g. `crates`
 * @returns {string[]} repository-relative crate directories
 */
function listCrateDirectories(container) {
  const absolute = join(repoRoot, container);
  if (!existsSync(absolute) || !statSync(absolute).isDirectory()) return [];
  return readdirSync(absolute, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .filter((entry) => !entry.name.startsWith('.'))
    .filter((entry) => !SKIP_DIR_NAMES.has(entry.name))
    .map((entry) => `${container}/${entry.name}`)
    .sort();
}

const failures = [];
const warnings = [];

const rootManifestPath = join(repoRoot, 'Cargo.toml');
if (!existsSync(rootManifestPath)) {
  process.stderr.write(`Crate inventory standard failed:\n- ${repoRoot}/Cargo.toml must exist\n`);
  process.exit(1);
}
const rootManifest = parseManifest(readFileSync(rootManifestPath, 'utf8'));

const declaredMembers = arrayOfStrings(rootManifest, 'workspace', 'members');
const memberDirectories = declaredMembers.flatMap(expandMember);
const memberSet = new Set(memberDirectories);

// R1 + R2: every on-disk crate directory must be a declared, manifest-bearing member.
const crateDirectories = [...listCrateDirectories('crates'), ...listCrateDirectories('plugins')];
const manifestsFound = [];

for (const directory of crateDirectories) {
  const manifestPath = join(repoRoot, directory, 'Cargo.toml');
  if (!existsSync(manifestPath)) {
    failures.push(
      `${directory}/ has no Cargo.toml: it is committed but unreachable by cargo, ` +
        `cargo test --workspace, and every crate-scanning gate (QUALITY_GATE_SPEC.md section 30). ` +
        `Give it a workspace-inheriting manifest and add it to the root [workspace] members, ` +
        `or remove the directory.`,
    );
    continue;
  }
  manifestsFound.push(directory);
  const manifest = parseManifest(readFileSync(manifestPath, 'utf8'));
  const packageName = scalar(manifest, 'package', 'name');
  if (!packageName) {
    failures.push(`${directory}/Cargo.toml declares no [package].name`);
    continue;
  }
  if (!memberSet.has(directory)) {
    failures.push(
      `${directory}/ (package "${packageName}") is not listed in the root Cargo.toml ` +
        `[workspace] members: cargo will not build, test, or lint it.`,
    );
  }
  if (basename(directory) !== packageName) {
    warnings.push(
      `${directory}/ directory name differs from package name "${packageName}" ` +
        `(allowed for host-embedded roots; confirm this is intentional)`,
    );
  }
}

// R3: no dangling members.
for (const directory of memberDirectories) {
  const manifestPath = join(repoRoot, directory, 'Cargo.toml');
  if (!existsSync(manifestPath)) {
    failures.push(
      `root Cargo.toml [workspace] members lists "${directory}", ` +
        `but ${directory}/Cargo.toml does not exist on disk`,
    );
  }
}

// Fail closed: an empty scan proves nothing.
if (crateDirectories.length === 0) {
  failures.push(
    'no crate directories were scanned under crates/ or plugins/: an empty scan ' +
      'cannot distinguish "nothing to check" from "checker is broken" ' +
      '(QUALITY_GATE_SPEC.md section 31)',
  );
}
if (manifestsFound.length === 0) {
  failures.push(
    'no Cargo.toml was found under crates/ or plugins/: refusing to report success ' +
      '(QUALITY_GATE_SPEC.md section 31)',
  );
}

const summary = {
  repoRoot,
  crateDirectoriesScanned: crateDirectories.length,
  crateManifestsFound: manifestsFound.length,
  workspaceMembersDeclared: declaredMembers.length,
  workspaceMembersResolved: memberDirectories.length,
  unitsExamined: crateDirectories.length + memberDirectories.length,
  failures,
  warnings,
};

if (AS_JSON) {
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
} else {
  process.stdout.write(`repo root      : ${repoRoot}\n`);
  process.stdout.write(`crates/plugins : ${crateDirectories.length} first-level crate directories scanned\n`);
  process.stdout.write(`manifests      : ${manifestsFound.length} Cargo.toml found\n`);
  process.stdout.write(
    `members        : ${declaredMembers.length} declared -> ${memberDirectories.length} resolved on disk\n`,
  );
  process.stdout.write(`units examined : ${summary.unitsExamined}\n`);
}

if (failures.length > 0) {
  if (!AS_JSON) {
    process.stderr.write(
      `Crate inventory standard failed:\n${failures.map((failure) => `- ${failure}`).join('\n')}\n`,
    );
    if (warnings.length > 0) {
      process.stderr.write(`Warnings:\n${warnings.map((warning) => `- ${warning}`).join('\n')}\n`);
    }
  }
  process.exit(1);
}

if (!AS_JSON) {
  if (warnings.length > 0) {
    process.stdout.write(
      `Crate inventory standard passed with warnings:\n${warnings
        .map((warning) => `- ${warning}`)
        .join('\n')}\n`,
    );
  } else {
    process.stdout.write('Crate inventory standard passed\n');
  }
}
