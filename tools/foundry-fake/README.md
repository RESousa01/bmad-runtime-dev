# Deterministic Foundry fake

This tool is an offline contract and failure-injection surface for Forge tests.
It implements only:

- `GET /api/projects/<project>/deployments?api-version=v1`
- `POST /api/projects/<project>/openai/v1/responses`

It is not a production fallback. Startup requires the explicit environment
`Development` or `Test`, and is refused whenever `NODE_ENV`,
`ASPNETCORE_ENVIRONMENT`, `DOTNET_ENVIRONMENT`, or `FORGE_ENVIRONMENT` says
`production`. The .NET in-memory implementation also lives only in the test
assembly and independently rejects every environment except Development/Test.

## Verify

```powershell
node --test tools/foundry-fake/server.test.mjs
dotnet test services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj `
  -c Release -- --filter-class "*FakeFoundry*"
cargo test -p desktop-cloud --test foundry_fixture_contract --locked
```

The implementation plan's legacy .NET `--filter FakeFoundry` spelling is not
accepted by this repository's xUnit v3 Microsoft.Testing.Platform runner. The
`--filter-class` command above is the runner-equivalent verification.

## Run a scripted scenario

```powershell
node tools/foundry-fake/server.mjs `
  --environment Test `
  --scenario-group deployment `
  --scenario multiple_pages `
  --port 0
```

The command prints a small JSON startup record containing the loopback origin
and queued-response count. Requests must send:

```text
Authorization: Bearer forge-foundry-fake-marker-v1
```

This is a fixed synthetic marker, never a production credential. Missing or
different markers receive `401` and do not consume the response queue.

The canonical catalog is
`tools/foundry-fake/fixtures/foundry-fixtures.v1.json`. Its byte-identical Rust
mirror is checked by `foundry_fixture_contract`. The catalog covers every
deployment-list, streaming, refusal, content-filter, timeout, retry, missing
deployment, server failure, malformed stream, truncation, and cancellation
scenario required by FND-003.

Each received request produces only deterministic evidence: sequence, test
clock time, method, normalized route, SHA-256 request-target and body labels, a
truncation flag, and sorted header names. Raw paths, query values, header
values, and request content are never retained. Scripted bodies, request
bodies, and queue depth have hard limits.
