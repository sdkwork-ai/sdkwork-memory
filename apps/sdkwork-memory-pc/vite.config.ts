import { resolveBrowserDistOutDir } from '../../../sdkwork-specs/tools/browser-dist-layout.mjs';
function resolveViteEnvironment(mode, processEnv = process.env) {
  const profileMatch = /^(standalone|cloud)\.(development|test|staging|production)$/u.exec(mode ?? '');
  return profileMatch?.[2]
    ?? (['development', 'test', 'staging', 'production'].includes(processEnv.SDKWORK_ENVIRONMENT ?? '')
      ? processEnv.SDKWORK_ENVIRONMENT
      : 'production');
}
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), react()],
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
});
