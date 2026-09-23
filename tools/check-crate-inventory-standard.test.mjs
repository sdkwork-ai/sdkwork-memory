#!/usr/bin/env node
//
// check-crate-inventory-standard.test.mjs
//
// Self-test for `tools/check-crate-inventory-standard.mjs`.
//
// A gate that always passes is worse than no gate (QUALITY_GATE_SPEC.md section
// 30). This suite therefore builds synthetic repositories in a temp directory and
// asserts that each rule actually turns the gate red, plus that a healthy layout
// turns it green. Nothing here touches the real repository.
//
// Run: node --test tools/check-crate-inventory-standard.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join, dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const GATE = resolve(dirname(fileURLToPath(import.meta.url)), 'check-crate-inventory-standard.mjs');

/**
 * Builds a throwaway repository layout on disk.
 *
 * @param {Record<string, string>} files map of repository-relative path to contents
 * @returns {string} absolute path of the synthetic repository root
 */
function makeRepo(files) {
  const root = mkdtempSync(join(tmpdir(), 'crate-inventory-'));
  for (const [relativePath, contents] of Object.entries(files)) {
    const absolutePath = join(root, relativePath);
    mkdirSync(dirname(absolutePath), { recursive: true });
    writeFileSync(absolutePath, contents, 'utf8');
  }
  return root;
}

/**
 * Runs the gate against a synthetic repository.
 *
 * @param {string} root repository root to scan
 * @returns {{ status: number, stdout: string, stderr: string }} observed result
 */
function runGate(root) {
  const result = spawnSync(process.execPath, [GATE, '--root', root], { encoding: 'utf8' });
  return { status: result.status ?? -1, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

/**
 * Renders a minimal workspace manifest.
 *
 * @param {string[]} members workspace member paths
 * @param {string} [extra] extra top-level TOML appended verbatim
 * @returns {string} manifest text
 */
function rootManifest(members, extra = '') {
  const lines = members.map((member) => `  "${member}",`).join('\n');
  return `[workspace]\nresolver = "2"\nmembers = [\n${lines}\n]\n${extra}`;
}

/**
 * Renders a minimal member manifest.
 *
 * @param {string} name package name
 * @returns {string} manifest text
 */
function memberManifest(name) {
  return `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2021"\n`;
}

test('healthy repository passes and reports the units it examined', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest(['crates/sdkwork-alpha', 'plugins/sdkwork-beta']),
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
    'plugins/sdkwork-beta/Cargo.toml': memberManifest('sdkwork-beta'),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Crate inventory standard passed/);
    assert.match(result.stdout, /2 first-level crate directories scanned/);
    assert.match(result.stdout, /units examined : 4/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('R1: a crate directory with no Cargo.toml turns the gate red', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest(['crates/sdkwork-alpha']),
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
    'crates/sdkwork-residue/src/lib.rs': '// committed, but unreachable\n',
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1, 'a manifest-less crate directory must fail the gate');
    assert.match(result.stderr, /crates\/sdkwork-residue\/ has no Cargo\.toml/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('R2: a crate with a manifest but no members entry turns the gate red', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest(['crates/sdkwork-alpha']),
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
    'crates/sdkwork-orphan/Cargo.toml': memberManifest('sdkwork-orphan'),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1, 'an undeclared member must fail the gate');
    assert.match(result.stderr, /not listed in the root Cargo\.toml \[workspace\] members/);
    assert.match(result.stderr, /sdkwork-orphan/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('R3: a members entry pointing at a missing directory turns the gate red', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest(['crates/sdkwork-alpha', 'crates/sdkwork-ghost']),
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1, 'a dangling member must fail the gate');
    assert.match(result.stderr, /crates\/sdkwork-ghost\/Cargo\.toml does not exist on disk/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('fail closed: scanning zero crate directories is a failure, not a pass', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest([]),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1, 'an empty scan must not report success');
    assert.match(result.stderr, /no crate directories were scanned/);
    assert.doesNotMatch(result.stdout, /passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('members globs expand like cargo, so a globbed crate is not falsely reported', () => {
  const root = makeRepo({
    'Cargo.toml': rootManifest(['crates/*']),
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
    'crates/sdkwork-beta/Cargo.toml': memberManifest('sdkwork-beta'),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /2 resolved on disk/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('missing root manifest fails instead of silently scanning nothing', () => {
  const root = makeRepo({
    'crates/sdkwork-alpha/Cargo.toml': memberManifest('sdkwork-alpha'),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Cargo\.toml must exist/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
