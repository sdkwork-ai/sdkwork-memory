$ErrorActionPreference = "Stop"

function Assert-PathExists {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$Message = "Missing required path"
    )
    if (!(Test-Path -LiteralPath $Path)) {
        throw "${Message}: ${Path}"
    }
}

function Assert-PathAbsent {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$Message = "Forbidden path exists"
    )
    if (Test-Path -LiteralPath $Path) {
        throw "${Message}: ${Path}"
    }
}

Assert-PathExists "AGENTS.md" "Missing SDKWORK agent entrypoint"
Assert-PathExists "agent.app.config.json" "Missing application manifest"
Assert-PathExists ".sdkwork/README.md" "Missing SDKWORK workspace README"
Assert-PathExists ".sdkwork/skills/README.md" "Missing SDKWORK skills README"
Assert-PathExists ".sdkwork/plugins/README.md" "Missing SDKWORK plugins README"
Assert-PathExists "specs/topology.spec.json" "Missing topology spec"
Assert-PathExists "docs/topology-standard.md" "Missing topology standard doc"
Assert-PathExists "scripts/lib/memory-topology.mjs" "Missing topology adapter"
Assert-PathExists "scripts/memory-dev.mjs" "Missing topology dev orchestrator"
Assert-PathExists "scripts/generate-memory-sdk.mjs" "Missing SDK generation orchestrator"
Assert-PathExists "sdks/standardize-memory-sdk-family.mjs" "Missing SDK family standardizer"

$topologySpec = Get-Content -Raw "specs/topology.spec.json" | ConvertFrom-Json
if ($topologySpec.schemaVersion -ne 5) {
    throw "specs/topology.spec.json schemaVersion must be 5"
}
if ($topologySpec.kind -ne "sdkwork.app.topology") {
    throw "specs/topology.spec.json kind must be sdkwork.app.topology"
}
foreach ($profileId in @($topologySpec.defaults.developmentProfileId, $topologySpec.defaults.productionProfileId)) {
    $profilePath = $topologySpec.profileFiles.$profileId
    if (!$profilePath) {
        throw "specs/topology.spec.json must declare profileFiles.$profileId"
    }
    Assert-PathExists $profilePath "Missing topology profile file for $profileId"
}

$requiredRootDirectories = @(
    "apis", "apps", "crates", "sdks", "tools", "etc", "deployments",
    "scripts", "docs", "tests", "database", "specs", ".sdkwork"
)

foreach ($directory in $requiredRootDirectories) {
    Assert-PathExists $directory "Missing standard root directory"
}

Assert-PathAbsent "services" "Nonstandard top-level services directory must be removed"

$expectedPackages = @(
    "sdkwork-memory-contract",
    "sdkwork-memory-database-host",
    "sdkwork-intelligence-memory-service",
    "sdkwork-intelligence-memory-repository-sqlx",
    "sdkwork-api-memory-standalone-gateway",
    "sdkwork-routes-memory-open-api",
    "sdkwork-routes-memory-app-api",
    "sdkwork-routes-memory-backend-api"
)

$cargoTomls = Get-ChildItem -Path . -Recurse -Filter Cargo.toml -File |
    Where-Object { $_.FullName -notmatch "\\target\\" } |
    Sort-Object FullName

$packageNames = New-Object System.Collections.Generic.List[string]
foreach ($cargoToml in $cargoTomls) {
    $relativePath = $cargoToml.FullName.Substring((Get-Location).Path.Length + 1).Replace("\", "/")
    if ($relativePath -ne "Cargo.toml" -and !$relativePath.StartsWith("crates/") -and !$relativePath.StartsWith("plugins/")) {
        throw "Authored Rust package manifest must live under crates/ or plugins/: $relativePath"
    }

    $match = Select-String -LiteralPath $cargoToml.FullName -Pattern '^name\s*=\s*"([^"]+)"' | Select-Object -First 1
    if ($null -ne $match) {
        [void]$packageNames.Add($match.Matches.Groups[1].Value)
    }
}

foreach ($expectedPackage in $expectedPackages) {
    if (!$packageNames.Contains($expectedPackage)) {
        throw "Expected Cargo package is missing: $expectedPackage"
    }
}

foreach ($routerCrate in @(
    "sdkwork-routes-memory-open-api",
    "sdkwork-routes-memory-app-api",
    "sdkwork-routes-memory-backend-api"
)) {
    Assert-PathExists "crates/$routerCrate/README.md" "Router crate README"
    Assert-PathExists "crates/$routerCrate/specs/component.spec.json" "Router crate component spec"
    Assert-PathExists "crates/$routerCrate/src/web_bootstrap.rs" "Router web bootstrap"
    Assert-PathExists "crates/$routerCrate/src/manifest.rs" "Router manifest"
}

Assert-PathExists "sdks/test/verify-sdk-ownership-boundaries.test.mjs" "SDK ownership boundary test"
Assert-PathExists ".sdkwork/.gitignore" ".sdkwork/.gitignore must exist"
Assert-PathExists "deployments/docker/Dockerfile" "Container Dockerfile"
Assert-PathExists "deployments/kubernetes/deployment.yaml" "Kubernetes deployment manifest"

foreach ($crateComponentSpec in @(
    "crates/sdkwork-memory-contract/specs/component.spec.json",
    "crates/sdkwork-intelligence-memory-service/specs/component.spec.json",
    "crates/sdkwork-intelligence-memory-repository-sqlx/specs/component.spec.json",
    "crates/sdkwork-memory-database-host/specs/component.spec.json",
    "crates/sdkwork-api-memory-standalone-gateway/specs/component.spec.json"
)) {
    Assert-PathExists $crateComponentSpec "Core crate component spec"
}

Write-Host "SDKWork Memory structure verification passed."
