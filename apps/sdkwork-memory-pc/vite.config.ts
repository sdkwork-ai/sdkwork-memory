import { resolveViteEnvironment } from '../../../sdkwork-specs/tools/vite-runtime-profile.mjs';
import { resolveBrowserDistOutDir } from '../../../sdkwork-specs/tools/browser-dist-layout.mjs';
import { buildBrowserDevRuntimeEnvDocument } from '../../../sdkwork-specs/tools/browser-runtime-env.mjs';
import { createBrowserRuntimeEnvVitePlugin } from '../../../sdkwork-specs/tools/browser-runtime-env-vite.mjs';

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const RUNTIME_ENV_DOCUMENT_PATH = "/runtime-env.json";

/**
 * Serve-only dev runtime document (APP_RUNTIME_ENV_SPEC.md §2/§6,
 * BROWSER_RUNTIME_ENV_SPEC.md §2; the shared Vite integration factory owns the middleware wiring).
 *
 * `public/runtime-env.json` is a per-build deploy artifact — `prebuild` writes it — so without this
 * middleware the dev server would fall through to whatever the last build left there and bootstrap
 * against stale deploy-time values (and it would 404 on a fresh checkout, where the file is absent
 * because it is git-ignored). Both deployment profiles serve the SAME same-origin relative document
 * in dev; the profile only changes the server-side fan-out target.
 *
 * The BUILD document stays owned by the canonical browser build runner, which materializes it from
 * `etc/sdkwork.deployment.config.json` with deploy-time authority. This plugin deliberately does not
 * emit a build asset.
 */
function memoryRuntimeEnvDocumentPlugin(mode: string) {
  return createBrowserRuntimeEnvVitePlugin({
    name: "memory-runtime-env-document",
    path: RUNTIME_ENV_DOCUMENT_PATH,
    resolveServeDocument: () =>
      JSON.stringify(buildBrowserDevRuntimeEnvDocument({ profileId: mode })),
  });
}

export default defineConfig(({ mode }: { mode: string }) => ({
  plugins: [memoryRuntimeEnvDocumentPlugin(mode), tailwindcss(), react()],
  resolve: {
    dedupe: ["react", "react-dom", "react-router", "react-router-dom"],
  },
  server: {
    host: "127.0.0.1",
    port: 3910,
  },
  preview: {
    host: "127.0.0.1",
    port: 4910,
  },
  build: {
    outDir: resolveBrowserDistOutDir(resolveViteEnvironment(mode, process.env)),
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            { name: "react-runtime", test: /node_modules[\\/].*(?:react|react-dom|react-router)/ },
            { name: "memory-app-sdk", test: /sdkwork-memory-app-sdk/ },
            { name: "sdkwork-pc-runtime", test: /sdkwork-(?:appbase|auth|core-pc|iam|ui-pc)/ },
            { name: "vendor", test: /node_modules/ },
          ],
        },
      },
    },
    sourcemap: false,
    target: "es2022",
  },
}));
