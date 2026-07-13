# Reference vault provenance and authority

Status: accepted for implementation reference; excluded from every product package and executable
surface.

The repaired vault manifest was regenerated with the reviewed repository script and validated on
2026-07-13. The recorded result is [`vault-validation.json`](./vault-validation.json). The vault has
105 root Markdown files, a verified manifest, and no validator errors or warnings.

## Authority precedence

For the Windows desktop delivery model, notes 93 through 100 override conflicting older text. Note
100 is the semantic authority for the reviewed BMAD Method and Builder snapshots. Note 90 supplies
work-packet and review discipline only; its cloud-first phase sequence is not desktop authority.

| Reference | SHA-256 |
|---|---|
| `90 - LLM-Tailored Development Plan and Agent Workflow.md` | `989913abccec860700fab1ad0c869c00e8d5be43b20f7fae3890f3bc239df1bb` |
| `93 - Split Web and Windows Desktop Architecture Plans.md` | `d024d20e13168b0b8a688fcfc557a9add34d4373a82dfa43f77e12148b7b4c66` |
| `94 - Windows Desktop Native Host and IPC.md` | `e70690defd25d2d37807cbbdf8bf1f90d4332e487f0736b2ada8b1157c33de58` |
| `95 - Windows Local Workspace and Execution.md` | `661beca08ebaaf20c74ead5e8f922827082cee23998a9e1917efca9a972e1d99` |
| `96 - Windows Local State, Evidence, Checkpoint, and Rollback.md` | `dba41687e1eb2fdcd93efe1d0c3f67dd7c18b98bf6a16b4291e3eb3df8dd956e` |
| `97 - Windows Desktop Security and Trust Model.md` | `9573a03e625c32c0bf7d382f1493f7aeaf034870aac148b0cf60bfca34d1e7ce` |
| `98 - Azure Support Plane for Windows Desktop.md` | `6b4f8e131e5510ef7a4f688ebd80cd2736a2fef96cf7c7ff6928da25741bb9f4` |
| `99 - Dual-Delivery Contract and Conformance Specification.md` | `3e94e63f63e5a86a3244494a7943e02c182fa1419035d6b4c4b38cb40940d5ea` |
| `100 - BMAD Method and Builder Deep Comprehension Audit.md` | `172b7b78f560f06b3f9ba29658e11e4c797f2b76eddcf0f17a20a4f3976dbacc` |

## Source-intake boundary

The vault's source ledger records the following archive identities. These are archive hashes, not
proof of an upstream Git identity:

| Source | Declared version | Archive SHA-256 | Release decision |
|---|---:|---|---|
| BMAD Method | 6.10.0 | `A7C049038099B99081FBD03D22C6A5180EDD88DEE656BB37C4276B1CC31B4A32` | Semantic reference and sealed fixture input only; release promotion remains blocked on immutable upstream identity, trademark, and redistribution review. |
| BMAD Builder | package 2.1.0 / module 1.0.0 | `D3C70744A9875623B01856CC907CF558324BACC920F0D860C36AD2788A4D2852` | Inactive draft semantics only; no upstream scripts execute and release promotion remains blocked on the same intake gate. |
| OpenClaw | 2026.6.11 | `6D1F477A4C69204FB22C9480081281EB547FF2BC353592077559F02D01B4ED8E` | Research patterns only; never packaged. |
| Hermes | 0.18.2 / 2026.7.7.2 | `E5E0941C515867EC024B343E775D07F34B323B363CB0570863CF6690B9291095` | Research patterns only; never packaged. Restrictive PowerPoint assets are explicitly excluded. |
| Odysseus | conflicting 1.0.1 / 1.0.0 metadata | unavailable | AGPL clean-room UX requirements only. No code, prompt, style sheet, or asset is copied or linked. |

Canonical upstream URLs, immutable commit/tag identities, acquisition evidence, and release signatures
are absent from the supplied archives. This gap is recorded rather than guessed. No reference
snapshot is a build input, runtime dependency, executable fixture, or packaged resource. Imported
third-party scripts are never invoked from this monorepo.
