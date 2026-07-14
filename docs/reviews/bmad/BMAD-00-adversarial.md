# BMAD-00 adversarial robustness review

Date: 2026-07-14
Verdict: **PASS**

## Focus

The read-only review covered the packet's required adversarial areas:

- archive/member substitution and automatic lock-update risk;
- license, trademark, and immutable-identity omission;
- absolute, drive, UNC, traversal, alternate-data-stream, and physical
  link/reparse escapes;
- executable/runtime content and authority-field smuggling;
- malformed and duplicate-key JSON ambiguity;
- stale documentation or action aliases overriding live source identity;
- accidental context-library reads, product CI dependencies, or packaging.

## Results

- Relocation succeeds from a copy containing only the foundation package and an
  empty environment.
- Managed-byte drift and archive/source/member substitutions fail without
  automatic lock changes.
- Executable filenames/content and authority-bearing projection fields are
  quarantined.
- Dependency fields and context-library markers select the external-context
  recovery path.
- Manifest/projection traversal and Windows junction material select the
  reference-escape recovery path.
- Missing legal/license decisions and guessed Git identity select their exact
  stable recovery paths.
- Eight malformed ledger-record classes retain stable recovery codes; plain and
  escaped duplicate JSON keys fail closed.
- Exact closure holds for 8 projections, 6 roster entries, 4 prompt bindings,
  and all 76 source members.
- Method remains `sealed_read_only`; Builder projections remain `inactive_data`;
  operational authority is `none`.
- Agent and Workflow action identities are distinct and Convert is absent.
- No normalized descriptor, runtime manifest, Tauri resource inclusion, or
  product lookup of the context library exists in this packet.

The review found one non-blocking alias gap: product guards recognized
`pnpm vault:check` but not a direct Node invocation of the optional verifier.
The guard was extended to cover pnpm/pnpm.cmd and node/node.exe forms with
Windows and POSIX relative paths. A focused follow-up passed and no P0–P3
findings remain.

## Validation

- Foundation: 50/50 tests passed.
- Fixtures: 76/76 tests passed.
- Source hashes: Method 29/29 and Builder 47/47 matched.
- Boundary verification, secret scan, syntax checks, and `git diff --check`
  passed.

## Residual obligations

- `blocked_provenance` remains required until immutable upstream identity or a
  human-owned expiring exception exists.
- Normalized descriptors, runtime manifest, and Tauri resource proofs are
  correctly deferred to BMAD-04.
- Unrelated Desktop Support API/D2 changes, the Obsidian workspace file, and
  `dist-runtime/` were excluded.
