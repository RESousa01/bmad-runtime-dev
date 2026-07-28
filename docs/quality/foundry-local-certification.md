# Foundry local certification

FND-063. This is the complete local verification matrix for the Foundry
connection. It proves the implementation against deterministic fixtures
before anything is deployed.

## Running it

```powershell
pwsh -File tools/foundry-smoke/local-verify.ps1
```

The report lands at `artifacts/foundry/qualification/local-matrix.json`,
with per-gate stdout and stderr under
`artifacts/foundry/qualification/logs/`.

Exit codes:

| Code | Meaning |
|---|---|
| `0` | Every mandatory gate passed |
| `2` | At least one mandatory gate failed or could not run |

## What it will not do

**It will not launder a missing toolchain into a green matrix.** A gate
that cannot be attempted is recorded as `unavailable` with a machine
-readable reason, and `ready` is false whenever a *mandatory* gate is
unavailable. There is deliberately no outcome that means "skipped, and
that's fine".

**It will not regenerate contracts by default.** Contract regeneration is
global: it rewrites `schema-lock.json` and the entire generated tree from
every schema at once, so whoever regenerates last wins. When more than one
agent or person is working the same checkout, an unrequested regeneration
can silently overwrite someone else's in-flight work. Pass `-Regenerate`
only when this checkout has a single writer; otherwise the read-only
`verify:cross-language` check still proves the committed tree is
self-consistent with its sources.

**It does not certify a deployment.** Every gate here runs against the
deterministic fake Foundry surface and local fixtures. Passing this matrix
says the code is correct and internally consistent. It says nothing about
whether a real project endpoint, managed identity, role assignment, or
network path works. That is the deployed qualification, and it is a
separate gate with separate evidence.

## The matrix

| Layer | Gate | Proves |
|---|---|---|
| contracts | `contracts-cross-language` | Schemas, generated bindings, and fixtures agree across TypeScript, Rust, and .NET |
| support-api | `dotnet-restore-locked` | The reviewed lock files still satisfy the solution |
| support-api | `dotnet-build` | The whole solution compiles warning-free |
| support-api | `dotnet-test` | Discovery, catalog, profiles, routing, Responses, streaming, receipts, security |
| rust | `cargo-test-workspace` | Transport, catalog decoding, selection persistence, receipt verification |
| rust | `cargo-clippy` | Pedantic lint with warnings denied |
| renderer | `ui-typecheck`, `ui-test`, `ui-build` | Catalog client, picker, context, chat |
| hygiene | `check-boundaries` | The IPC command catalog stays byte-identical across its pinned sites |
| hygiene | `check-secrets` | No plaintext credential anywhere in the tree |
| hygiene | `infra-contracts` | Bicep, compose, and workflow contracts still hold |
| foundry | `foundry-fake` | The offline Foundry surface is deterministic and cannot run in production |
| foundry | `prerequisite-checker` | Endpoint, pagination, identity, and role validation fail closed |
| foundry | `network-smoke` | DNS, TLS, token, and egress posture checks fail closed |
| foundry | `profile-validator` | Profiles map to real deployments with real qualification evidence |
| foundry | `profile-publisher` | Publication is atomic, versioned, conditional, and rollbackable |
| infrastructure | `bicep-build` | The template compiles and its asserts are well-formed |

## Reading the report

`ready: true` requires zero failed mandatory gates **and** zero
unavailable mandatory gates. Each gate records its command, working
directory, exit code, duration, tool version, and log path, so a claim in
the report can always be traced back to the bytes that produced it.

`blockedGates` lists mandatory gates that could not run. If that array is
non-empty, the matrix is not a certification, regardless of how many
other gates passed.

## Status

The harness is written and parse-verified. Its first full run is pending
a quiet checkout: running `dotnet build` and `cargo` while another writer
is mid-edit produces failures that reflect the concurrency, not the code.
Run it when this checkout has a single writer, and record the resulting
report as the FND-063 evidence.
