$ErrorActionPreference = "Stop"

function Read-JsonFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (!(Test-Path $Path)) {
        throw "Missing required file: $Path"
    }
    return Get-Content -Raw $Path | ConvertFrom-Json
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Content,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Path
    )
    if (!$Content.Contains($Needle)) {
        throw "$Path must contain: $Needle"
    }
}

$requiredFiles = @(
    "AGENTS.md",
    "CODEX.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".sdkwork/README.md",
    ".sdkwork/skills/README.md",
    ".sdkwork/plugins/README.md",
    "sdkwork.app.config.json",
    "specs/README.md",
    "specs/component.spec.json",
    "docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md",
    "docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md",
    "docs/superpowers/specs/2026-06-10-ai-memory-architecture-design.md",
    "docs/superpowers/specs/2026-06-10-memory-spi-plugin-architecture-design.md",
    "docs/schema-registry/README.md",
    "docs/schema-registry/tables/001-memory-core.yaml",
    "docs/schema-registry/tables/002-memory-learning.yaml",
    "docs/schema-registry/tables/003-memory-retrieval.yaml",
    "docs/schema-registry/tables/004-memory-provider.yaml",
    "docs/schema-registry/tables/005-memory-governance.yaml",
    "sdks/README.md",
    "sdks/sdkwork-memory-sdk/README.md",
    "sdks/sdkwork-memory-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-sdk/specs/README.md",
    "sdks/sdkwork-memory-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json",
    "sdks/sdkwork-memory-app-sdk/README.md",
    "sdks/sdkwork-memory-app-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-app-sdk/specs/README.md",
    "sdks/sdkwork-memory-app-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json",
    "sdks/sdkwork-memory-backend-sdk/README.md",
    "sdks/sdkwork-memory-backend-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-backend-sdk/specs/README.md",
    "sdks/sdkwork-memory-backend-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json",
    "apis/authority-manifest.json",
    "apis/open-api/memory-open-api.openapi.json",
    "apis/app-api/memory-app-api.openapi.json",
    "apis/backend-api/memory-backend-api.openapi.json",
    "sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json",
    "sdks/_route-manifests/app-api/sdkwork-routes-memory-app-api.route-manifest.json",
    "sdks/_route-manifests/backend-api/sdkwork-routes-memory-backend-api.route-manifest.json",
    "package.json",
    "sdkwork.workflow.json",
    ".github/workflows/package.yml"
)

foreach ($file in $requiredFiles) {
    if (!(Test-Path $file)) {
        throw "Missing required phase1 contract artifact: $file"
    }
}

foreach ($forbidden in @("sdks/sdkwork-memory-open-api", "sdks/sdkwork-memory-app-api", "sdks/sdkwork-memory-backend-api", "sdks/memory-open-sdk", "sdks/memory-app-sdk", "sdks/memory-backend-sdk")) {
    if (Test-Path $forbidden) {
        throw "Forbidden SDK/API authority directory exists: $forbidden"
    }
}

$appConfig = Read-JsonFile "sdkwork.app.config.json"
if ($appConfig.schemaVersion -ne 3 -or $appConfig.kind -ne "sdkwork.app") {
    throw "sdkwork.app.config.json must use SDKWork app manifest v3"
}
if ($appConfig.app.key -ne "sdkwork-memory") {
    throw "sdkwork.app.config.json app.key must be sdkwork-memory"
}

$rootSpec = Read-JsonFile "specs/component.spec.json"
if ($rootSpec.component.name -ne "sdkwork-memory") {
    throw "Root component spec must identify sdkwork-memory"
}
if ($rootSpec.component.domain -ne "intelligence" -or $rootSpec.component.capability -ne "memory") {
    throw "Root component spec must use domain=intelligence and capability=memory"
}
if (!$rootSpec.contracts.sdkDependencies -or $rootSpec.contracts.sdkDependencies.Count -eq 0) {
    throw "Root component spec must declare sdkDependencies"
}
if ($null -eq $rootSpec.contracts.dependencyApiExports) {
    throw "Root component spec must explicitly declare dependencyApiExports"
}
if ($null -eq $rootSpec.contracts.dependencyApiSurfaces) {
    throw "Root component spec must explicitly declare dependencyApiSurfaces"
}

foreach ($family in @(
    @{ Path = "sdks/sdkwork-memory-sdk"; Authority = "sdkwork-memory-open-api"; Prefix = "/mem/v3/api"; Spec = "openapi/memory-open-api.openapi.json"; Client = "SdkworkMemoryOpenClient" },
    @{ Path = "sdks/sdkwork-memory-app-sdk"; Authority = "sdkwork-memory.app"; Prefix = "/app/v3/api"; Spec = "openapi/memory-app-api.openapi.json"; Client = "SdkworkMemoryAppClient" },
    @{ Path = "sdks/sdkwork-memory-backend-sdk"; Authority = "sdkwork-memory.backend"; Prefix = "/backend/v3/api"; Spec = "openapi/memory-backend-api.openapi.json"; Client = "SdkworkMemoryBackendClient" }
)) {
    $manifest = Read-JsonFile (Join-Path $family.Path "sdk-manifest.json")
    $component = Read-JsonFile (Join-Path $family.Path "specs/component.spec.json")

    if ($manifest.sdkOwner -ne "sdkwork-memory") {
        throw "$($family.Path) manifest sdkOwner mismatch"
    }
    if ($manifest.apiAuthority -ne $family.Authority) {
        throw "$($family.Path) apiAuthority mismatch"
    }
    if ($manifest.generationInputSpec -ne $family.Spec) {
        throw "$($family.Path) generationInputSpec mismatch"
    }
    if ($manifest.discoverySurface.apiPrefix -ne $family.Prefix -or $manifest.apiPrefix -ne $family.Prefix) {
        throw "$($family.Path) apiPrefix mismatch"
    }
    if ($null -eq $component.contracts.sdkDependencies) {
        throw "$($family.Path) component spec must declare sdkDependencies"
    }
    if ($null -eq $component.contracts.dependencyApiExports) {
        throw "$($family.Path) component spec must explicitly declare dependencyApiExports"
    }
    if (!$component.contracts.sdkClients.Contains($family.Client)) {
        throw "$($family.Path) component spec must declare client $($family.Client)"
    }
}

function Verify-OpenApi {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Prefix,
        [Parameter(Mandatory = $true)][string]$Authority,
        [Parameter(Mandatory = $true)][string]$SdkFamily,
        [Parameter(Mandatory = $true)][ValidateSet("dual-token", "api-key")][string]$AuthMode,
        [Parameter(Mandatory = $true)][ValidateSet("open-api", "app-api", "backend-api")][string]$ExpectedApiSurface,
        [Parameter(Mandatory = $true)][string[]]$RequiredOperationIds,
        [Parameter(Mandatory = $true)][string[]]$RequiredSchemas
    )

    $spec = Read-JsonFile $Path
    if (!$spec.openapi.StartsWith("3.1.")) {
        throw "$Path must use OpenAPI 3.1.x"
    }
    if ($spec.'x-sdkwork-owner' -ne "sdkwork-memory" -or $spec.'x-sdkwork-api-authority' -ne $Authority -or $spec.'x-sdkwork-sdk-family' -ne $SdkFamily) {
        throw "$Path root SDKWork ownership metadata mismatch"
    }
    if (!$spec.components -or !$spec.components.schemas -or !$spec.components.securitySchemes) {
        throw "$Path must define schemas and securitySchemes"
    }
    $authToken = $spec.components.securitySchemes.AuthToken
    $accessToken = $spec.components.securitySchemes.AccessToken
    $apiKey = $spec.components.securitySchemes.ApiKey
    if ($AuthMode -eq "dual-token") {
        if (!$authToken -or $authToken.type -ne "http" -or $authToken.scheme -ne "bearer") {
            throw "$Path must define AuthToken as http bearer"
        }
        if (!$accessToken -or $accessToken.type -ne "apiKey" -or $accessToken.in -ne "header" -or $accessToken.name -ne "Access-Token") {
            throw "$Path must define AccessToken as Access-Token apiKey header"
        }
        if ($apiKey) {
            throw "$Path dual-token API must not declare ApiKey security scheme"
        }
    }
    if ($AuthMode -eq "api-key") {
        if (!$apiKey -or $apiKey.type -ne "apiKey" -or $apiKey.in -ne "header" -or $apiKey.name -ne "X-API-Key") {
            throw "$Path must define ApiKey as X-API-Key apiKey header"
        }
        if ($authToken -or $accessToken) {
            throw "$Path open API must not declare app/backend token security schemes"
        }
    }

    $operationIds = New-Object System.Collections.Generic.HashSet[string]
    foreach ($pathProperty in $spec.paths.PSObject.Properties) {
        if (!$pathProperty.Name.StartsWith($Prefix)) {
            throw "$Path contains non-canonical path prefix: $($pathProperty.Name)"
        }
        if ($AuthMode -eq "api-key" -and ($pathProperty.Name.StartsWith("/app/v3/api") -or $pathProperty.Name.StartsWith("/backend/v3/api"))) {
            throw "$Path open API must not use app/backend prefix: $($pathProperty.Name)"
        }
        if (($AuthMode -eq "api-key" -or $Prefix -eq "/backend/v3/api") -and $pathProperty.Name -match "/auth|/login|/sessions|/refresh|/logout") {
            throw "$Path backend/open API must not expose auth/session path: $($pathProperty.Name)"
        }

        foreach ($methodProperty in $pathProperty.Value.PSObject.Properties) {
            $methodName = [string]$methodProperty.Name
            if ($methodName -notin @("get", "post", "patch", "delete")) {
                continue
            }
            $operation = $methodProperty.Value
            $operationId = [string]$operation.operationId
            if ([string]::IsNullOrWhiteSpace($operationId)) {
                throw "$Path operation missing operationId at $($pathProperty.Name)"
            }
            if ($operationId.Contains("_") -or $operationId.Contains("__")) {
                throw "$Path operationId must use dotted lowerCamelCase style: $operationId"
            }
            [void]$operationIds.Add($operationId)
            if ($operation.'x-sdkwork-owner' -ne "sdkwork-memory" -or $operation.'x-sdkwork-api-authority' -ne $Authority) {
                throw "$Path operation ownership mismatch: $operationId"
            }
            if (!$operation.'x-sdkwork-permission' -or !$operation.'x-sdkwork-audit-event' -or !$operation.'x-sdkwork-auth-mode') {
                throw "$Path operation missing permission/audit/auth metadata: $operationId"
            }
            if ($operation.'x-sdkwork-auth-mode' -ne $AuthMode) {
                throw "$Path operation auth mode mismatch for $operationId"
            }
            if ($operation.'x-sdkwork-api-surface' -ne $ExpectedApiSurface) {
                throw "$Path operation api surface mismatch for $operationId"
            }
            if ($operation.'x-sdkwork-request-context' -ne "WebRequestContext") {
                throw "$Path operation must declare WebRequestContext for $operationId"
            }
            $security = $operation.security
            if (!$security -or $security.Count -eq 0) {
                throw "$Path operation missing security declaration: $operationId"
            }
            $firstSecurity = $security[0]
            if ($AuthMode -eq "dual-token" -and (!$firstSecurity.PSObject.Properties["AuthToken"] -or !$firstSecurity.PSObject.Properties["AccessToken"])) {
                throw "$Path operation must require both AuthToken and AccessToken: $operationId"
            }
            if ($AuthMode -eq "api-key" -and !$firstSecurity.PSObject.Properties["ApiKey"]) {
                throw "$Path operation must require ApiKey: $operationId"
            }
            if ($AuthMode -eq "api-key" -and ($firstSecurity.PSObject.Properties["AuthToken"] -or $firstSecurity.PSObject.Properties["AccessToken"])) {
                throw "$Path open API operation must not require app/backend tokens: $operationId"
            }
            foreach ($errorStatus in @("400", "404")) {
                $responseProperty = $operation.responses.PSObject.Properties[$errorStatus]
                if ($responseProperty) {
                    $content = $responseProperty.Value.content
                    if (!$content -or !$content.PSObject.Properties["application/problem+json"]) {
                        throw "$Path error response $errorStatus must include application/problem+json: $operationId"
                    }
                }
            }
            if ($pathProperty.Name.Contains("{") -and (!$operation.parameters -or $operation.parameters.Count -eq 0)) {
                throw "$Path operation with path parameter has no parameters: $operationId"
            }
            if (($methodName -eq "post" -or $methodName -eq "patch") -and !$operation.requestBody) {
                throw "$Path mutating operation has no requestBody: $operationId"
            }
            if ($methodName -eq "post" -and $operation.'x-sdkwork-idempotent') {
                $hasIdempotency = $false
                foreach ($parameter in $operation.parameters) {
                    if ($parameter.name -eq "Idempotency-Key" -and $parameter.in -eq "header") {
                        $hasIdempotency = $true
                    }
                }
                if (!$hasIdempotency) {
                    throw "$Path idempotent POST missing Idempotency-Key header: $operationId"
                }
            }
        }
    }

    foreach ($requiredId in $RequiredOperationIds) {
        if (!$operationIds.Contains($requiredId)) {
            throw "$Path missing required operationId: $requiredId"
        }
    }

    foreach ($schemaName in $RequiredSchemas) {
        if (!$spec.components.schemas.PSObject.Properties[$schemaName]) {
            throw "$Path missing required schema: $schemaName"
        }
    }

    Write-Host "Verified $Path with $($operationIds.Count) operations."
}

$appOpenApiCheck = @{
    Path = "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json"
    Prefix = "/app/v3/api"
    Authority = "sdkwork-memory.app"
    SdkFamily = "sdkwork-memory-app-sdk"
    AuthMode = "dual-token"
    ExpectedApiSurface = "app-api"
    RequiredOperationIds = @(
        "spaces.create", "spaces.list", "spaces.retrieve", "spaces.update",
        "events.create", "events.retrieve",
        "memories.create", "memories.list", "memories.retrieve", "memories.update", "memories.delete", "memories.deleteAll", "memories.sources.list",
        "forgetRequests.create", "forgetRequests.retrieve",
        "extractions.create",
        "candidates.list", "candidates.retrieve", "candidates.approve", "candidates.reject",
        "habits.list", "habits.retrieve", "habits.update", "habits.confirm", "habits.reject",
        "retrievals.create", "retrievals.retrieve",
        "contextPacks.create", "contextPacks.retrieve",
        "feedback.create",
        "exportJobs.create", "exportJobs.retrieve",
        "learningSettings.retrieve", "learningSettings.update"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemorySpace", "MemoryEvent", "MemoryRecord", "MemoryCandidate", "MemoryHabit",
        "MemoryRetrievalRequest", "MemoryRetrievalResult", "MemoryContextPackRequest", "MemoryContextPack",
        "MemoryLearningSettings", "MemoryForgetJob", "MemoryExportJob"
    )
}
Verify-OpenApi @appOpenApiCheck

$openApiCheck = @{
    Path = "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json"
    Prefix = "/mem/v3/api"
    Authority = "sdkwork-memory-open-api"
    SdkFamily = "sdkwork-memory-sdk"
    AuthMode = "api-key"
    ExpectedApiSurface = "open-api"
    RequiredOperationIds = @(
        "capabilities.retrieve",
        "events.create", "events.retrieve",
        "memories.create", "memories.list", "memories.retrieve", "memories.update", "memories.delete", "memories.deleteAll",
        "retrievals.create", "retrievals.retrieve",
        "contextPacks.create", "contextPacks.retrieve",
        "feedback.create",
        "extractions.create",
        "candidates.list", "candidates.retrieve",
        "providerHealth.retrieve"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemoryCapabilities", "MemoryEvent", "MemoryRecord",
        "MemoryRetrievalRequest", "MemoryRetrievalResult", "MemoryContextPackRequest", "MemoryContextPack",
        "MemoryFeedbackRequest", "MemoryFeedback", "MemoryExtractionRequest", "MemoryLearningJob",
        "MemoryCandidate", "MemoryProviderHealth"
    )
}
Verify-OpenApi @openApiCheck

$backendOpenApiCheck = @{
    Path = "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json"
    Prefix = "/backend/v3/api"
    Authority = "sdkwork-memory.backend"
    SdkFamily = "sdkwork-memory-backend-sdk"
    AuthMode = "dual-token"
    ExpectedApiSurface = "backend-api"
    RequiredOperationIds = @(
        "spaces.list", "spaces.retrieve", "spaces.update",
        "memories.list", "memories.retrieve", "memories.update", "memories.supersede",
        "events.list", "events.retrieve",
        "candidates.list", "candidates.approve", "candidates.reject",
        "extractionJobs.create", "extractionJobs.retrieve", "consolidationJobs.create",
        "indexes.create", "indexes.list", "indexes.retrieve", "indexes.update", "indexes.rebuild",
        "retrievalProfiles.create", "retrievalProfiles.list", "retrievalProfiles.retrieve", "retrievalProfiles.update",
        "implementationProfiles.create", "implementationProfiles.list", "implementationProfiles.retrieve", "implementationProfiles.update",
        "providerBindings.create", "providerBindings.list", "providerBindings.update",
        "providerHealth.retrieve",
        "evalRuns.create", "evalRuns.list", "evalRuns.retrieve",
        "retrievalTraces.list", "retrievalTraces.retrieve",
        "auditLogs.list", "retentionJobs.create", "migrationJobs.create", "migrationJobs.retrieve"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemoryIndex", "MemoryRetrievalProfile", "MemoryImplementationProfile",
        "MemoryProviderBinding", "MemoryProviderHealth", "MemoryEvalRun", "MemoryAuditLog",
        "MemoryMigrationJobRequest", "MemoryRetentionJobRequest"
    )
}
Verify-OpenApi @backendOpenApiCheck

foreach ($routeManifestPath in @(
    "sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json",
    "sdks/_route-manifests/app-api/sdkwork-routes-memory-app-api.route-manifest.json",
    "sdks/_route-manifests/backend-api/sdkwork-routes-memory-backend-api.route-manifest.json"
)) {
    $routeManifest = Read-JsonFile $routeManifestPath
    if ($routeManifest.kind -ne "sdkwork.route.manifest") {
        throw "$routeManifestPath must use sdkwork.route.manifest kind"
    }
    foreach ($route in $routeManifest.routes) {
        if ($route.requestContext -ne "WebRequestContext") {
            throw "$routeManifestPath route $($route.method) $($route.path) must declare WebRequestContext"
        }
        if ($route.apiSurface -notin @("open-api", "app-api", "backend-api")) {
            throw "$routeManifestPath route $($route.method) $($route.path) must declare canonical apiSurface"
        }
    }
    Write-Host "Verified $routeManifestPath with $($routeManifest.routes.Count) routes."
}

foreach ($schemaPath in Get-ChildItem -Path "docs/schema-registry/tables" -Filter "*.yaml") {
    $content = Get-Content -Raw $schemaPath.FullName
    Assert-Contains -Content $content -Needle "module: memory" -Path $schemaPath.FullName
    Assert-Contains -Content $content -Needle "owner: sdkwork-memory" -Path $schemaPath.FullName
    Assert-Contains -Content $content -Needle "table: ai_" -Path $schemaPath.FullName
}

$allSchemaText = (Get-ChildItem -Path "docs/schema-registry/tables" -Filter "*.yaml" | ForEach-Object { Get-Content -Raw $_.FullName }) -join [Environment]::NewLine
foreach ($requiredTable in @(
    "ai_space", "ai_event", "ai_record", "ai_record_source", "ai_entity", "ai_edge",
    "ai_candidate", "ai_habit", "ai_habit_signal", "ai_learning_job",
    "ai_index", "ai_index_entry", "ai_retrieval_profile", "ai_retrieval_trace", "ai_retrieval_hit", "ai_context_pack",
    "ai_implementation_profile", "ai_provider_binding", "ai_policy",
    "ai_audit_log", "ai_eval_run", "ai_outbox_event"
)) {
    if (!$allSchemaText.Contains("table: $requiredTable")) {
        throw "Schema registry missing required table: $requiredTable"
    }
}

$design = Get-Content -Raw "docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md"
foreach ($snippet in @(
    "Embedding Optional",
    "Multi-Implementation Abstraction",
    "Open API Contract Draft",
    "App API Contract Draft",
    "Backend API Contract Draft",
    "Database And Storage Design",
    "ai_"
)) {
    Assert-Contains -Content $design -Needle $snippet -Path "docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md"
}

$spiDesign = Get-Content -Raw "docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md"
foreach ($snippet in @(
    "MemoryPluginManifest",
    "MemoryRuntimePlugin",
    "MemoryCoreRuntime",
    "Stable Core And Plugin Boundaries",
    "SPI Port Families",
    "Runtime Plugin Manifest",
    "Built-In Plugin Families",
    "Conformance suite",
    "0.1.0 Implementation Decisions",
    "Static Rust registration",
    "JSON manifest plus Rust constant",
    "Runtime plugins are not Codex agent plugins",
    'Do not place runtime Memory plugins under `.sdkwork/plugins/`',
    "Industry References"
)) {
    Assert-Contains -Content $spiDesign -Needle $snippet -Path "docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md"
}

foreach ($legacyStub in @("docs/superpowers/specs/2026-06-10-ai-memory-architecture-design.md", "docs/superpowers/specs/2026-06-10-memory-spi-plugin-architecture-design.md")) {
    $stubContent = Get-Content -Raw $legacyStub
    Assert-Contains -Content $stubContent -Needle "Migrated" -Path $legacyStub
    Assert-Contains -Content $stubContent -Needle "docs/architecture/tech/" -Path $legacyStub
}

if ($spiDesign.Contains("## 17. Open Decisions")) {
    throw "SPI design must resolve first-landing open decisions before runtime implementation starts."
}

Write-Host "SDKWork Memory phase1 contract verification passed."
