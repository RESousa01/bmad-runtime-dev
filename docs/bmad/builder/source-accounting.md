# BMad Builder 2.1.0 source accounting

This report describes the immutable source graph retained from the pinned BMad
Builder 2.1.0 archive. The machine-readable authority is
`packages/bmad-foundation/builder-source-ledger.json`; the strict verifier
regenerates the ledger from the pinned source manifest and rejects any drift.

The archive identity is:

- Version: `2.1.0`
- SHA-256: `d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852`
- Archive members: `299/299`
- Sapphirus governance sidecars: `2/2`
- Total accounted read-only members: `301/301`
- Directed resource edges: `319`

Source inclusion grants no authority. Builder prose, scripts, templates,
samples, tests, and generated content remain untrusted data. Runtime use must
still pass through typed plans, Airlock policy, exact approval where required,
deterministic execution, post-validation, and durable evidence.

## Production skill closure

| Skill | Product role | Members | Closed | Closure |
| --- | --- | ---: | ---: | ---: |
| `bmad-agent-builder` | Agent Builder | 50 | 50 | 100% |
| `bmad-bmb-setup` | Setup | 6 | 6 | 100% |
| `bmad-eval-runner` | Internal Eval Runner | 13 | 13 | 100% |
| `bmad-module-builder` | Module Builder | 20 | 20 | 100% |
| `bmad-workflow-builder` | Workflow Builder | 34 | 34 | 100% |

Each production skill root owns its complete recursive resource tree. Parsed
Markdown links, skill-relative resource paths, directory references, and static
JavaScript imports add separate directed edges. References inside authoring
templates and examples that address a future generated package are recorded as
`generated_output_reference` edges rather than being misreported as missing
pinned source.

## Classification

| Classification | Members | Product treatment |
| --- | ---: | --- |
| Production source | 111 | Runtime and validator input; catalogued roots also feed the UI catalog |
| Nested setup template | 6 | Generated-package, installer, and conformance input |
| Sample fixture | 96 | Sandbox and conformance fixtures only |
| Upstream test | 8 | Conformance evidence only |
| Web/distribution asset | 28 | Distribution only |
| Documentation | 31 | Distribution only |
| Package metadata and governance sidecars | 21 | Distribution-only metadata, plus validator and conformance use for the two Sapphirus sidecars |

The seven sample roots are fixtures, not production capabilities:

- `samples/bmad-agent-code-coach`
- `samples/bmad-agent-creative-muse`
- `samples/bmad-agent-diagram-reviewer`
- `samples/bmad-agent-dream-weaver`
- `samples/bmad-agent-sentinel`
- `samples/bmad-excalidraw`
- `samples/sample-module-setup`

Website content, repository automation, documentation tooling, and package
distribution metadata likewise contribute zero runtime capabilities. The
production capability count remains exactly five, with Eval Runner explicitly
internal.

## Strict checks

The verifier fails closed on:

- changed source bytes or source-manifest identity;
- a Builder version or archive hash other than the pin;
- missing, extra, duplicated, or unclassified ledger members;
- duplicate case-folded canonical identities;
- unresolved concrete local references;
- a production skill whose recursive member closure is below 100%; or
- any ledger content that differs from deterministic regeneration.

Run:

```text
node packages/bmad-foundation/scripts/builder-resource-closure.mjs --builder 2.1.0 --strict
```
