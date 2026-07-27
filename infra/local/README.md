# Forge local end-user testing (OpenAI, no Azure deployment)

This is the exact path for running Forge against a real OpenAI key on a clean
Windows machine, with no Azure deployment and no hosted support plane.

## There is no local support plane to stand up

**Forge does not need the Azure support plane, or any local replacement for it,
to run against OpenAI.** This is a property of the code, not a workaround:

- The support-plane transport lives behind the `production-support` Cargo
  feature. That feature is not in `desktop-app`'s `default` set, and no build
  script in `package.json` turns it on.
- Even with the feature on, `production_support_settings()` requires five
  `option_env!` values baked in at compile time (`SAPPHIRUS_SUPPORT_TENANT_ID`
  and friends). Absent, the build falls through to the development modes.
- `ProductionHelpTransport` is never constructed anywhere in the application.
  It is an unused future seam, marked `dead_code` in
  `crates/desktop-app/src/bmad_model/transport.rs`. The coordinator only ever
  builds the offline, deterministic, or direct-provider transports.
- In a default build the only outbound call the host makes is straight to
  `https://api.openai.com/v1/responses` from
  `crates/desktop-cloud/src/direct_provider.rs`. The updater plugin is
  registered but has no configured endpoint, and there is no telemetry client.

So no `docker-compose.yml` was written for this. A container stack in place of
the support plane would be serving a dependency the app does not have.

`infra/local/compose.yaml` still exists and is unchanged. It is a *convenience
and test seam*, not a runtime dependency: it serves the renderer in a container
for browser preview, and it boots the ASP.NET `desktop-support-api` image in its
Development composition so that service's own contract can be exercised. The
desktop app never calls it. You can ignore it entirely for the steps below.

The only thing you actually need is an `OPENAI_API_KEY` and outbound HTTPS to
`api.openai.com`.

## Prerequisites

- Windows 11, with WebView2 (present by default on Windows 11).
- Node 24.18.0 and pnpm 11.15.1 (the versions pinned in `package.json`).
- Rust 1.97.0 (pinned in `rust-toolchain.toml`).
- An OpenAI API key with Responses API access.
- Docker is **not** required.

## Steps from a clean checkout

### 1. Install dependencies

```powershell
pnpm install --frozen-lockfile
```

### 2. Provide the key

The host reads `OPENAI_API_KEY` from its own process environment. Set it in the
shell you are about to launch Forge from:

```powershell
$env:OPENAI_API_KEY = (Read-Host -AsSecureString "OpenAI API key" | ConvertFrom-SecureString -AsPlainText)
```

That keeps the key out of your shell history and off disk. A plain
`$env:OPENAI_API_KEY = "..."` works too, but it lands in history.

Nothing here reads a `.env` file automatically — there is no dotenv dependency
in the Rust host, the Tauri CLI, or Vite. `.env.example` at the repository root
documents the variables. If you prefer to keep a `.env`, copy it and load it
into the session yourself:

```powershell
Copy-Item .env.example .env   # then edit .env and put your real key in it
Get-Content .env | Where-Object { $_ -match '^\s*[^#].*=' } | ForEach-Object {
  $name, $value = $_ -split '=', 2
  Set-Item -Path "Env:$($name.Trim())" -Value $value.Trim()
}
```

`.env` is gitignored (`.gitignore` ignores `.env` and `.env.*` while
re-including `.env.example`). Never commit a filled-in copy.

**Alternative that touches no environment variable at all:** skip this step and
enter the key at runtime in **Settings > Providers** inside the app. The host
moves it straight into zeroizing memory and never writes it to disk or returns
it over IPC. This is the better path for a non-technical tester.

### 3. Run it

For an interactive run against the current source:

```powershell
pnpm desktop:dev
```

This uses `crates/desktop-app/tauri.conf.json` with **default** Cargo features,
which include `direct-model-providers`. It starts Vite on `127.0.0.1:1420` and
opens the native window.

To produce an installer that talks to a real provider:

```powershell
pnpm desktop:build:direct
```

The output NSIS installer lands in `target/release/bundle/nsis/`.

> Do **not** use `pnpm desktop:build` for this. That script is deliberately
> pinned to `--features deterministic-help`, which compiles in the canned-reply
> fake instead of a real provider. `tools/check-boundaries.mjs` asserts that
> exact string, so it was left alone and `desktop:build:direct` was added
> alongside it.

### 4. Pick the model

The default selection is OpenAI + `gpt-5-mini`. Change it in
**Settings > Providers** if you want a different model. If `OPENAI_API_KEY` was
set before launch, the app starts already connected to that selection.

## Verifying you are really talking to OpenAI

Any one of these is sufficient; the first two are the quickest.

1. **Check the destination label.** Open **Settings > Providers**, or look at
   the model label shown beside the Help/capability actions. A real provider
   connection reads `OpenAI · gpt-5-mini` (the provider display name and your
   selected model). The fake reads
   `Deterministic local model — development only`. These strings come from
   different code paths and cannot be confused —
   `DirectProviderConfig::destination_label()` versus the constant in
   `crates/desktop-app/src/bmad_model/config.rs`.

2. **Check the reported mode.** The `model.auth.status` projection reports
   `direct_provider_development` for a real provider and
   `deterministic_development` for the fake.

3. **Break it on purpose.** Launch with a deliberately wrong key — set
   `OPENAI_API_KEY` to the literal value `replace-me` — and send a request. A
   real build fails with a transport error. A deterministic build answers
   happily regardless of the key, because it never opens a socket. This is the
   most conclusive test: the fake cannot fail this way.

4. **Watch it cost money.** A real call shows up on the OpenAI usage dashboard
   within a few minutes, and answers vary between identical runs. Deterministic
   output is byte-identical every time and never appears in usage.

5. **Check the receipt proof.** Real direct-provider responses carry
   `forge-direct-provider-development-no-trust`; the fake carries
   `deterministic-fake-no-trust`.

## Things worth knowing before you file a bug

- **`OPENAI_BASE_URL` cannot point at a local proxy.** `validate_base_url` in
  `direct_provider.rs` accepts only host `api.openai.com`, scheme `https`, port
  443, and path `/v1`. A LiteLLM/Ollama/self-hosted gateway is rejected at
  startup. There is no supported way to serve the model locally.
- **Both direct-provider modes are explicitly development-only.** They mint a
  local no-trust receipt marker rather than a support-plane signature. That is
  by design and is not a defect.
- **The key never crosses IPC.** If you enter it in the UI, the form clears it
  once the native host accepts it. It is not written to frontend settings or to
  disk.

## The Compose stack (optional, unrelated to the OpenAI path)

Unchanged and not required for the above. It serves the renderer for browser
preview on `http://127.0.0.1:1420` and boots the support API's Development
composition on `http://127.0.0.1:18080` with shape-valid placeholder tenant and
client identifiers. Azure SQL, Key Vault, App Configuration, managed identity,
private networking, and Azure OpenAI remain unset and fail closed. Their
production definitions stay under `infra/desktop-support`; this stack neither
mocks nor replaces them.

The Tauri/Rust desktop host stays native on Windows. It owns WebView2, DPAPI,
workspace access, consent, and typed IPC, so putting it in a Linux container
would no longer test the desktop product.

```powershell
pnpm local:up
docker compose -f infra/local/compose.yaml ps
docker compose -f infra/local/compose.yaml logs --follow
pnpm local:down
```

Set `FORGE_SUPPORT_PORT` before starting Compose if port `18080` is already in
use. The container continues to listen on its production-compatible port 8080.

`pnpm desktop:local` starts that stack and then the native host together. It is
the browser-preview workflow; for plain OpenAI testing `pnpm desktop:dev` is
simpler and needs no Docker.

Model credentials are never passed to Compose, and the launcher never reads
them. For a Production-mode startup/configuration contract (not Azure service
emulation), see the optional profile in
`infra/desktop-support/local/README.md`.
