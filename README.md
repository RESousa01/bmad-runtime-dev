# Forge Desktop

Forge is an internal Windows desktop workspace for governed AI-assisted development. The native
Rust host owns workspace access, credentials, approvals, durable state, file effects, checkpoints,
rollback, and evidence. The React/WebView2 renderer is a typed client; it never receives generic
filesystem, process, credential, database, or updater authority.

Start with the [team documentation hub](docs/README.md). For the shortest accurate overview, read
[product state](docs/current/product-state.md),
[architecture and authority](docs/current/architecture-and-authority.md), and
[release readiness](docs/current/release-readiness.md).

## Status at a glance

| Area | Current evidence | Boundary |
|---|---|---|
| Local workspace | Folder grants, bounded tree/read/search, sensitive-path handling | Native host authority |
| Governed edits | Exact review, single-use approval, checkpointed apply, verification, rollback, recovery | Effects remain fail-closed |
| BMAD | Pinned Method/Builder catalog and deterministic local Help | Only independently qualified capabilities may execute |
| Direct providers | Development-only OpenAI and Azure OpenAI Responses transport | In-memory API key; exact-context review still required |
| Local Docker | Frontend and support-API configuration/liveness rehearsal | Not Azure, installer, signing, or pilot proof |
| Distribution | Earlier offline NSIS lifecycle evidence exists | Exact-current signing and clean-machine release evidence remain outstanding |

Forge is useful as a local development prototype, but it is not release- or pilot-ready. Source
checks, a local build, an installer, a signed artifact, deployed Azure evidence, and a pilot are
separate acceptance gates.

## Architecture invariants

- `desktop-app` is the sole native composition root.
- Workspace content and model output are data, never authority.
- Every model request requires reviewed context and a fresh, single-use decision.
- Every proposed change re-enters the host-owned review, apply, evidence, and rollback boundary.
- Direct API-key transport is prototype evidence, not production authentication.
- Current development and intended deployment must work without Entra; the production credential
  and attribution design is incomplete.
- Imported reference sources are never executed by the product toolchain.

See the [accepted ADR index](docs/architecture/adr-index.md) for the decisions behind these rules.

## Quick start

Use the pinned toolchain: Node.js `24.18.0`, pnpm `11.15.1`, Rust `1.97.0`, and .NET SDK
`10.0.302`.

```powershell
pnpm toolchain:check
pnpm install --frozen-lockfile
pnpm docs:verify
```

Run deterministic local mode:

```powershell
pnpm desktop:dev:deterministic
```

Run the development-only direct-provider mode:

```powershell
pnpm desktop:dev
```

Run the local support seam alongside the native host:

```powershell
pnpm desktop:local
pnpm local:down
```

Provider credentials are entered in **Forge Settings > Providers**. The frontend submits the key
once over typed IPC and clears the field; the Rust host keeps it only in zeroizing process memory
until disconnect or process exit. Connecting does not itself send workspace content.

For prerequisites and PATH troubleshooting, use the [local setup guide](docs/development/local-setup.md).

## Verification

| Command | What it proves |
|---|---|
| `pnpm docs:verify` | Maintained docs, metadata, generated indexes, claims, references, and archive hashes |
| `pnpm verify:source` | Broad first-party source, contract, BMAD, frontend, and build checks |
| `pnpm verify:deferred-full` | Cross-language and broader native Rust verification |
| `pnpm desktop:build` | A local deterministic native build |
| `pnpm verify:bmad:100` | Strict BMAD campaign completion against canonical receipts |

Run the narrowest relevant check first, then the required broader gate. A green documentation gate
does not prove product source, and a green source gate does not prove installer, Azure, signing,
clean-machine, or pilot readiness.

## Repository map

- `apps/desktop-ui` — Forge workbench renderer and UI tests.
- `crates/desktop-app` — native composition root and Tauri application.
- `crates/desktop-runtime` — governed runtime and BMAD capability adapters.
- `crates/desktop-*` — native authority services and platform adapters.
- `packages/bmad-foundation` — sealed BMAD source, ledgers, catalog, and runtime projections.
- `packages/contracts` — schema source, fixtures, generators, and cross-language boundaries.
- `services/desktop-support-api` — support-plane scaffold and local contract seam.
- `infra` — local rehearsal and deployment infrastructure.
- `docs` — current guidance, ADRs, runbooks, evidence, references, and archives.

`bmad-runtime-lib` is a compatibility pointer and retained reference-source boundary. It is not
current guidance, a workspace package, a build input, a runtime dependency, or a distribution
artifact.

## Documentation and contribution

- [Documentation hub](docs/README.md)
- [Onboarding](docs/development/onboarding.md)
- [Contribution guide](docs/development/contribution-guide.md)
- [Testing and verification](docs/development/testing-and-verification.md)
- [Roadmap and known gaps](docs/current/roadmap-and-known-gaps.md)

Source, tests, and named gates outrank documentation. Accepted ADRs outrank current summaries;
plans, reviews, source analyses, and archives cannot prove implementation. Existing
`sapphirus.*`, `@sapphirus/*`, and `SAPPHIRUS_*` names are legacy compatibility identifiers, not
the human-facing product name.
