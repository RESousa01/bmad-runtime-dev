# Forge local test stack

This Compose project runs the container-shaped parts of Forge together without
changing the production Azure design.

## Services

- `frontend` builds the active Forge V2 renderer and serves the production
  bundle on `http://127.0.0.1:1420`.
- `desktop-support-api` builds the existing D2 ASP.NET image used by the Azure
  lane and exposes its local health surface on `http://127.0.0.1:18080`.
- The Tauri/Rust desktop host remains native on Windows. It owns WebView2,
  DPAPI, workspace access, consent, and typed IPC, so putting it in a Linux
  container would no longer test the desktop product.

The support API runs its existing `Development` composition with shape-valid
placeholder tenant and client identifiers. Azure SQL, Key Vault, App
Configuration, managed identity, private networking, and Azure OpenAI remain
unset and fail closed. Their production definitions remain under
`infra/desktop-support`; this stack neither mocks nor replaces them.

For a Production-mode startup/configuration contract (not Azure service
emulation), use the optional profile documented in
`infra/desktop-support/local/README.md`.

Model credentials are never passed to Compose. Choose the provider and
model/deployment, enter the API key, and connect in **Forge Settings >
Providers**. The form clears the key after the native host accepts it; the
credential is not written to frontend settings or disk.

## Commands

```powershell
pnpm local:up
docker compose -f infra/local/compose.yaml ps
docker compose -f infra/local/compose.yaml logs --follow
pnpm local:down
```

Set `FORGE_SUPPORT_PORT` before starting Compose if port `18080` is already in
use. The container continues to listen on its production-compatible port 8080.

To start the containers and the native desktop host together for an OpenAI
prototype run:

```powershell
pnpm desktop:local
```
