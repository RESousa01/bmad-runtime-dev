# Forge agent working agreement

## Before meaningful work

- Inspect branch, worktree, HEAD, and dirty state.
- Read `README.md`, `docs/README.md`, `package.json`, relevant accepted ADRs, and the active plan.
- Treat source, tests, and named gates as stronger authority than documentation.
- Preserve unrelated user changes and active task leases.
- Use the pinned Node, pnpm, Rust, and .NET toolchains.

## Authority and scope

Authority order:

```text
source/tests/gates > accepted ADR > docs/current > runbook > active plan
> dated review > source analysis > archive
```

Forge is the product name. Existing legacy technical identifiers remain compatibility contracts
until a dedicated migration proves otherwise. Current development and intended deployment must
work without Entra; prototype API-key transport is not production trust evidence.

`desktop-app` remains the local Rust authority. The renderer is a typed presentation client.
`bmad-runtime-lib` is archive/reference material, not current authority or a runtime dependency.
Never execute imported third-party scripts.

## Editing discipline

- Do not run concurrent agents against shared authority, metadata, generated index, or integration
  files.
- Do not perform broad generated-content rewrites.
- Keep integration-owned files single-owner.
- Run narrow tests first, then named broader gates in proportion to risk.
- Report exact failures, environment limitations, and evidence boundaries.
- Never treat a plan, branch, receipt label, or agent report as completion by itself.

## Durable context

Update repository current docs when verified behavior or decisions change. Archive completed plans
and handoffs. If a connected durable knowledge base is available, update it only after evidence
exists; the repository must remain understandable without personal paths or external notes.

