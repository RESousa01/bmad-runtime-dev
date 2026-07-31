# Forge local development paths

Forge’s native host runs directly on Windows. Docker is optional and does not replace the Tauri
host, WebView2, DPAPI, workspace grants, consent, or typed IPC.

## Choose the path you need

| Goal | Command | Docker | Model credential |
|---|---|---:|---|
| Deterministic local development | `pnpm desktop:dev:deterministic` | No | None |
| Real OpenAI/Azure OpenAI prototype request | `pnpm desktop:dev` | No | Enter in Forge Settings |
| Browser/support-API configuration rehearsal | `pnpm local:up` | Yes | Never passed to Compose |
| Native host plus local support seam | `pnpm desktop:local` | Yes | Enter in Forge Settings |

For ordinary direct-provider testing, use `pnpm desktop:dev`; the support API and Compose stack are
not runtime dependencies of that path.

## Prerequisites

- Windows 11 with WebView2.
- Node.js `24.18.0` and pnpm `11.15.1`.
- Rust `1.97.0` with the MSVC toolchain.
- An OpenAI or Azure OpenAI API key only when testing direct-provider mode.

```powershell
pnpm toolchain:check
pnpm install --frozen-lockfile
```

## Direct-provider development

```powershell
pnpm desktop:dev
```

Open **Forge Settings > Providers**, choose OpenAI or Azure OpenAI, select the model, enter the key,
and connect. This is the preferred path because the frontend submits the credential once over typed
IPC, clears the field, and the Rust host retains it only in zeroizing memory until disconnect or
process exit.

Do not put credentials in committed files, `.env`, Compose configuration, frontend settings,
fixtures, logs, or documentation. An inherited process environment may be supported for controlled
developer automation, but it is not the normal tester workflow.

Connecting does not send workspace data. Each request still requires exact-context review and a
fresh single-use decision. Direct-provider receipts are development-only and do not establish
production trust.

Build the direct-provider development installer with:

```powershell
pnpm desktop:build:direct
```

`pnpm desktop:build` intentionally builds deterministic Help instead.

## Optional Compose rehearsal

```powershell
pnpm local:up
docker compose -f infra/local/compose.yaml ps
docker compose -f infra/local/compose.yaml logs --follow
pnpm local:down
```

The stack serves the renderer for browser preview and boots the support API in Development mode.
It does not emulate Azure SQL, Key Vault, App Configuration, managed identity, private networking,
Container Apps behavior, signing, or Azure OpenAI. The native desktop app does not gain authority
from the containers, and model credentials are never passed to them.

Set `FORGE_SUPPORT_PORT` before startup if host port `18080` is unavailable. The container continues
to listen on port `8080`.

For the Production configuration/liveness profile—which is still not Azure emulation—see
[`infra/desktop-support/local/README.md`](../desktop-support/local/README.md). For the product
evidence boundary, see [authentication and model access](../../docs/current/authentication-and-model-access.md)
and [release readiness](../../docs/current/release-readiness.md).
