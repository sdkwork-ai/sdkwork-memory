# SDKWork Memory SDK Workspace

This directory owns SDKWork Memory SDK families and authority OpenAPI documents.

SDK families:

- `sdkwork-memory-sdk` for `sdkwork-memory-open-api` and `/mem/v3/api`
- `sdkwork-memory-app-sdk` for `sdkwork-memory-app-api` and `/app/v3/api`
- `sdkwork-memory-backend-sdk` for `sdkwork-memory-backend-api` and `/backend/v3/api`

Protected Open API clients use `X-API-Key` through generated SDK credential providers. They must not join app/backend token-manager client lists.

RPC SDK families are deferred until high-throughput backend/native RPC integration is needed.
