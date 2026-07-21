# Local Docker emulation (testing only)

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

Usage:

```powershell
docker compose -f infra/desktop-support/local/docker-compose.yml up --build --detach
./tools/support-smoke/local-smoke.ps1
docker compose -f infra/desktop-support/local/docker-compose.yml down
```
