// Invoked as `node scripts/materialize-runtime-env.mjs` — see `package.json` (`predev`/`prebuild`)
// and `etc/sdkwork.deployment.config.json#materialization.command`.
//
// Deliberately thin. Every rule this is tempted to re-implement — profile identity, the
// standalone same-origin `/` requirement, the cloud unified `api-*` edge set from
// ENVIRONMENT_SPEC §5.1.0.1, the output path, `--check` staleness — already lives once in the
// canonical shared runner. Re-deriving it here is how sibling apps drifted into three different
// mutually inconsistent materializers. This file only maps the app's CLI onto that shared
// implementation, so that `pnpm dev` / `pnpm build` — which bypass the canonical runner through
// their own Vite invocation — cannot disagree with `pnpm build:pc:*`.
//
// No `#!` shebang on purpose: this module is also imported by tests, and a shebang combined with a
// CRLF checkout breaks the esbuild transform (same failure class documented in
// sdkwork-specs/tools/vite-runtime-profile.mjs).

import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

// This file lives one level deeper than the app root, so the sdkwork-specs hop is `../../../../`
// here (app-root scripts such as `build:dev` use `../../../`).
import { materializeBrowserRuntimeEnv } from '../../../../sdkwork-specs/tools/build-browser-client.mjs';

const APP_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPOSITORY_ROOT = path.resolve(APP_ROOT, '..', '..');

function option(argv, name, fallback) {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
}

function main() {
  const argv = process.argv.slice(2);
  const deploymentProfile = option(argv, '--deployment-profile', 'standalone');
  const environment = option(argv, '--environment', 'development');
  const check = argv.includes('--check');

  materializeBrowserRuntimeEnv({
    appRoot: APP_ROOT,
    deploymentProfile,
    environment,
    repositoryRoot: REPOSITORY_ROOT,
    check,
  });

  console.log(
    `[sdkwork-memory-pc] runtime env ${check ? 'verified' : 'materialized'}: ${deploymentProfile}.${environment}`,
  );
}

try {
  main();
} catch (error) {
  console.error(`[sdkwork-memory-pc] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
