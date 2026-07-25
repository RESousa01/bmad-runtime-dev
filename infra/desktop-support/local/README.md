# Desktop support local runtime/configuration rehearsal

Runs the desktop-support API container locally to rehearse the Azure
Container Apps deployment shape before a real deployment exists. This is a
test harness, not a deployment path: it uses the ASP.NET Development
composition (in-memory device/idempotency stores, development signing and
model brokerage) and placeholder Entra identifiers that satisfy
configuration validation only.

What it does emulate:

- the exact container image the CI image job builds
  (`services/desktop-support-api/Dockerfile`, same build context);
- the Container Apps environment-variable contract from
  `infra/desktop-support/main.bicep` (the `Sapphirus__*` names);
- the liveness/readiness probe paths and the bounded health response shape;
- fail-closed behavior of every authenticated route.

What it does not emulate: Entra token issuance, Azure SQL, Key Vault
signing, App Configuration policy, Azure OpenAI brokerage, VNet/private
endpoints, or managed identity. Token-bearing smoke assertions stay in
`tools/support-smoke/deployed-smoke.ps1` against a real deployment.

An optional `production-configuration-contract` profile provides
**boot/configuration fidelity only**. It starts the same image with
`ASPNETCORE_ENVIRONMENT=Production` and shape-valid, non-secret placeholder
values for every `Sapphirus__*` setting. Its liveness check proves that startup
validation and Production dependency composition succeed. The container uses
`network_mode: none`, so it does not emulate or call Azure services.
`/healthz/ready` is expected to remain unhealthy because SQL, App Configuration,
Key Vault, managed identity, and Azure OpenAI are not present.

Usage:

```powershell
docker compose -f infra/desktop-support/local/docker-compose.yml up --build --detach --wait
./tools/support-smoke/local-smoke.ps1 -SupportOrigin http://127.0.0.1:18080
docker compose -f infra/desktop-support/local/docker-compose.yml `
  --profile production-configuration-contract up --build --detach --wait `
  production-configuration-contract
docker compose -f infra/desktop-support/local/docker-compose.yml `
  exec -T production-configuration-contract `
  wget -q -O - http://127.0.0.1:8080/healthz/live
docker compose -f infra/desktop-support/local/docker-compose.yml `
  --profile production-configuration-contract down
```
