# Builder 2.1.0 source compatibility notes

> **Implementation authority:** The upstream files under
> `packages/bmad-foundation/source/builder/2.1.0/` are pinned source evidence
> and untrusted runtime input. They are not the Forge/Sapphirus runtime
> architecture, installation policy, or authority model.

These notes apply only to BMad Builder `2.1.0` from archive SHA-256
`d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852`.
The machine-readable records beside the retained source bind every correction
below to that exact identity:

- `source-notes.json` classifies upstream documentation drift.
- `distribution-decision.json` records the Forge distribution, promotion, and
  visibility decisions.

Neither record changes or substitutes for any archive member.

## Stale contributor guidance

The pinned upstream `AGENTS.md` describes a historical `src/` tree with YAML
agents and workflows. The pinned archive has no root `src/` directory; its
production Builder surface is the five skill directories under `skills/`.
`AGENTS.md` also names `npm test`, `test:schemas`, `test:refs`,
`validate:schemas`, and `validate:refs`, but none of those scripts exists in the
pinned `package.json`.

The same file says that Builder modules are published as npm packages and that
`src/` is installed through `npx bmad-method install`. That contributor note is
not an installation instruction for Forge. The pinned package is marked
`private: true`, and its declared distribution files are `skills/`,
`.claude-plugin/`, and `CHANGELOG.md`.

Implementation agents must use the canonical Sapphirus contracts, capability
catalog, runtime ports, and Builder promotion lifecycle. They must not
reconstruct the historical `src/` architecture or run missing upstream scripts.

## Distribution decision

Builder 2.1.0 is a hash-pinned, bundled source input. Marketplace metadata such
as `.claude-plugin/marketplace.json` is retained for compatibility and
provenance, but is parsed as untrusted metadata. It never grants installation,
filesystem, process, network, model, secret, publishing, or activation
authority.

Custom Builder sources may enter Forge only through the governed import and
promotion path:

```text
source acquisition and identity verification
→ validation
→ analysis
→ evaluation
→ review
→ approval
→ staging
→ installation
→ atomic catalog activation
→ live conformance
→ receipt
```

Direct npm installation, direct marketplace activation, and activation merely
because files exist are prohibited.

## Eval Runner visibility

The pinned source contains `skills/bmad-eval-runner/SKILL.md`, but
`skills/module-help.csv` exposes exactly nine public actions and no Eval Runner
row. Forge therefore records 9 public actions plus 1 internal Eval Runner action,
for 10 total Builder actions.

Internal means reachable only through typed Builder evaluation orchestration.
It is not advertised as a public help action and receives no ambient execution,
model, filesystem, network, or credential authority from its skill text.
