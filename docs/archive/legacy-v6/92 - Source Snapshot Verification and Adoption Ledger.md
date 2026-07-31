---
title: "Source Snapshot Verification and Adoption Ledger"
aliases:
  - "Source Verification Ledger"
  - "OpenClaw Hermes BMAD Odysseus Intake"
tags:
  - bmad-runtime
  - vault/source-review
  - provenance
  - licensing
section: "Review and Decisions"
order: 92
status: v6.16-reviewed
reviewed_on: 2026-07-09
project: Sapphirus BMAD Runtime
---

# Source Snapshot Verification and Adoption Ledger

> Historical provenance/adoption evidence. Preserve hashes, licenses, extraction findings, and earlier decisions exactly. Current BMAD semantic review is [[100 - BMAD Method and Builder Deep Comprehension Audit]]. New delivery-model adoption and implementation authority are recorded in [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

## Verdict

The supplied Hermes and OpenClaw ZIPs have now been extracted into complete review trees. The earlier partial trees remain historical evidence but are no longer authoritative for source conclusions.

Archive completeness is not release provenance. Neither ZIP includes enough Git identity to prove an upstream commit/tag, so both remain research snapshots until origin/ref/signature evidence is added. Root licenses also do not authorize every bundled component.

### V6.18 BMAD semantic audit location

The 2026-07-10 deep audit inspected all 47 Method source skill entrypoints, 37 Method customization files, the Method installer/config/help/validation paths, all five live Builder skill roots, the embedded setup-skill template, 48 Builder reference files, module/setup/eval scripts, manifests, changelog, and representative tests. Findings and the corrected adoption plan are in [[100 - BMAD Method and Builder Deep Comprehension Audit]]. These counts describe the extracted review surface; they do not close the immutable upstream ref/signature gap recorded below.

## Authoritative review locations

| Source | Authoritative review tree | Role |
|---|---|---|
| BMAD Method | `_source_review/BMAD-METHOD-main/BMAD-METHOD-main` | Product/method foundation |
| BMAD Builder | `_source_review/bmad-builder-main/bmad-builder-main` | Authoring/evaluation foundation |
| OpenClaw | `_full/o/openclaw-main` | Complete comparable-runtime research snapshot |
| Hermes | `_full/h/hermes-agent-main` | Complete comparable-runtime research snapshot |
| Odysseus | Existing reviewed source tree referenced by [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]] | Pattern/requirements input with incomplete provenance |

The former `_source_review/openclaw-main/openclaw-main` and `_source_review/hermes-agent-main/hermes-agent-main` trees are partial/historical and must not be used for completeness claims.

## Archive identity and verification

| Source | Declared version | Archive SHA-256 | Archive bytes | ZIP entries | Extraction/content verification | Remaining identity gap |
|---|---:|---|---:|---:|---|---|
| BMAD Method | 6.10.0 | `A7C049038099B99081FBD03D22C6A5180EDD88DEE656BB37C4276B1CC31B4A32` | Not re-counted in this pass | Not re-counted in this pass | Reviewed extracted tree and archive hash | Record canonical upstream URL, commit/tag, acquisition time, and signature/release evidence |
| BMAD Builder | package 2.1.0; module 1.0.0 | `D3C70744A9875623B01856CC907CF558324BACC920F0D860C36AD2788A4D2852` | Not re-counted in this pass | Not re-counted in this pass | Reviewed extracted tree and archive hash | Record canonical upstream URL, commit/tag, acquisition time, and signature/release evidence |
| OpenClaw | 2026.6.11 | `6D1F477A4C69204FB22C9480081281EB547FF2BC353592077559F02D01B4ED8E` | 73,046,632 | 23,305 = 21,980 regular files + 1,293 directories + 32 symlinks | Every regular archive path is present and uncompressed size matches. Archive hash is known. A complete per-file content-hash pass was not completed and must not be claimed. | No `.git`, commit, tag, signed release, or independently verified origin in the ZIP |
| Hermes | 0.18.2 / release 2026.7.7.2 | `E5E0941C515867EC024B343E775D07F34B323B363CB0570863CF6690B9291095` | 68,120,646 | 7,075 = 6,205 regular files + 870 directories | Every regular file was compared to its ZIP member by SHA-256; no missing, size/hash mismatch, unsafe path, or case-collision result was found. | No `.git`, commit, tag, signed release, or independently verified origin in the ZIP |
| Odysseus | Source metadata disagrees: 1.0.1 vs 1.0.0 | Not available | Not available | Not available | Broad source review only; no archive/Git completeness ledger | Obtain immutable source archive/commit, full tree, license/notice inventory, and version reconciliation |

Original user-supplied archives:

- `C:\Users\rodrigocsousa\Downloads\openclaw-main.zip`
- `C:\Users\rodrigocsousa\Downloads\hermes-agent-main.zip`

The reusable read-only verifier is `_source_review/verify_zip_snapshot.py`. Its successful Hermes result may be cited. Its interrupted OpenClaw serial hash run may not be represented as a successful per-file hash verification.

## Recovered source coverage

### OpenClaw

The complete extraction exposed 9,016 archive paths that were absent from the earlier partial review tree. Important newly reviewable owners include:

| Surface | Files now present in complete snapshot |
|---|---:|
| `src/gateway` | 797 |
| plugins/extensions | 646 |
| configuration | 413 |
| context engine | 13 |
| tasks | 63 |
| logging/audit | 77 |
| infrastructure | 832 |
| UI | 706 |
| top-level tests | 677 |

The snapshot also contains 21 core package directories, 140 plugin manifests, and 202 QA YAML files. These counts describe review surface, not test success or maturity.

The 32 archive symlinks comprise 23 `CLAUDE.md -> AGENTS.md` aliases and 9 workspace/`node_modules` links. Their targets exist in the archive layout. The implementation source is review-complete by entry/size, but a direct installation from the Windows extraction must restore or generate the workspace links in hosted CI/remote Linux build rather than assuming local symlink behavior.

### Hermes

The complete extraction recovered 296 regular files absent from the prior tree. They are under `website/`, primarily Docusaurus/static/Chinese documentation. No runtime, workflow, provider, gateway, test, or operational conclusion changed solely because of those recovered files.

## Component-level license ledger

| Source/component | Observed terms | Current decision |
|---|---|---|
| BMAD Method root | MIT plus BMAD trademark notice | Foundation semantics/fixtures permitted only after final provenance, attribution, trademark, and redistribution review |
| BMAD Builder root/package | MIT/trademark context; package metadata is private | Foundation semantics permitted; distribution/product naming and bundled-file inventory require explicit review |
| OpenClaw root | MIT plus third-party notices | Pattern adoption allowed; copied code/assets require path-level license and provenance decision |
| OpenClaw `skills/skill-creator/license.txt` | Apache-2.0 | Separate attribution/notice decision; root MIT is not blanket authorization |
| Hermes root | MIT | Pattern adoption allowed; copied code/assets require path-level decision |
| Hermes `plugins/security-guidance` | Apache-2.0 with NOTICE provenance | Separate license/notice decision required |
| Hermes `skills/productivity/powerpoint/LICENSE.txt` | Restrictive Anthropic terms | `EXCLUDE`: do not copy, modify, redistribute, package, or use as a Builder fixture without documented entitlement and legal approval |
| Odysseus | AGPL-3.0-or-later | Clean-room requirements/patterns only by default; no copying/linking into a differently licensed product without explicit legal approval |

Before any source-derived code, asset, fixture, prompt, skill, or test enters a release, Source Intake must hash and classify every license/notice path and create a `ComponentLicenseDecision` of `include`, `exclude`, `clean_room_pattern_only`, or `legal_review_required`.

## Adoption boundaries from the full review

| Source | Safe adoption target | Boundary/correction |
|---|---|---|
| BMAD Method | Canonical method package/config/workflow/skill/artifact/help semantics and lineage | Runtime API still owns durable state; Airlock owns governed effects; a digest-pinned importer normalizes upstream data outside the .NET process |
| BMAD Builder | Authoring workflow, scan/evaluation concepts, baseline/variant/trigger quality fixtures | Generated output is a proposal; clean directory is not containment; scripts have per-runtime requirements; activation requires Azure-isolated exact-digest rehearsal and evidence |
| OpenClaw | Exact `system.run` approval-binding fields, plugin contract hygiene, context lifecycle concepts, task/audit/release test cases | Generic plugin approval is consent metadata, not enforcement; default sandbox/exec settings are unsafe for hosting; plugins run in-process; task/audit/replay stores are not distributed durable authority; context fallback can hide degradation |
| Hermes | Provider/cache contracts, narrow extension footprint, automation/turn/memory/budget failure fixtures | Require atomic `TurnCommit`, parsed exact Azure credential binding, governed finalized-memory promotion, fail-closed secret scope, immutable plugins, durable claims/outbox, atomic compression, artifact-bound release evidence, and aggregate budgets |
| Odysseus | Product shell, setup/admin, plan/artifact, workflow and connector requirements | Clean-room only; reject unsandboxed shell, mutable artifact versions, non-durable replay, CI `continue-on-error`, and disabled planning discipline |

## Source Intake release gate

A snapshot is promotable only when all fields below are present and verified:

- canonical upstream URL and owner;
- immutable commit/tag plus archive/release signature when provided;
- acquisition timestamp and tool/method;
- archive/tree hash and safe extraction report;
- regular-file, directory, symlink, case-collision, and unsafe-path accounting;
- declared versions reconciled across manifests;
- component-level SPDX/license/notice inventory and decision;
- copied/derived file map with attribution and clean-room record where applicable;
- dependency lock, registry/source, signer/provenance, vulnerability and SBOM evidence;
- fixture/generated-output hashes;
- review status, named human owner, expiry/revalidation trigger, and release decision.

Until that gate passes, OpenClaw, Hermes, and Odysseus remain research inputs. BMAD Method and Builder remain the selected product foundation, but release artifacts must still pin their exact accepted source identity.

## Related reviews

- [[83 - BMAD Source Code Review - Method and Builder]]
- [[84 - OpenClaw Source Review - Comparable Runtime Patterns]]
- [[85 - OpenClaw Structured Code Review]]
- [[86 - Hermes Source Code Review - Agent Runtime and Learning Loop]]
- [[87 - Hermes Deep Review - Extension Runtime and Operational Contracts]]
- [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]]
- [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]]
- [[91 - Technology, Language, Method, and LLM Implementation Review]]
