# SDKWork Memory Source Configuration

`sdkwork.deployment.config.json` is the source-controlled profile index for SDKWork Memory. It selects one profile under `topology/`; the topology contract is `../specs/topology.spec.json`.

Supported profiles are `standalone.development`, `standalone.production`, `cloud.development`, and `cloud.production`. Standalone development owns the local unified gateway. Cloud development starts no local service and consumes explicit deployed development URLs.

The cloud gateway TOML files are gateway composition handoff templates. Production secrets, database credentials, signing material, tokens, and local overrides are injected by the deployment platform and are never committed under `etc/`.

Validate with:

```powershell
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root .
pnpm topology:validate
```

<!-- SDKWORK-DEPLOY-LAYOUT: v1 -->
## Installed Runtime Paths

Authority: `APPLICATION_DEPLOY_LAYOUT_SPEC.md` (`../sdkwork-specs/`).

| Item | Value |
| --- | --- |
| `appId` | `sdkwork-memory` |
| `runtimeCode` | `memory` |
| Config root | `/etc/sdkwork/memory/` |
| Runtime TOML | `/etc/sdkwork/memory/config.toml` |
| Secrets | `/etc/sdkwork/memory/secrets/` |
| Override | `SDKWORK_MEMORY_CONFIG_FILE` |

Source profiles live under `etc/` (`sdkwork.deployment.config.json` index). Deploy manifest: `deployments/deploy.yaml`. Web data-plane source: `deployments/webserver/` (`SDKWORK_WEBSERVER_SPEC.md` layout v2).

```bash
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root .
node ../sdkwork-specs/tools/check-application-deploy-layout.mjs --root .
node ../sdkwork-specs/tools/check-webserver-toml-standard.mjs --root deployments/webserver
```
<!-- /SDKWORK-DEPLOY-LAYOUT -->


