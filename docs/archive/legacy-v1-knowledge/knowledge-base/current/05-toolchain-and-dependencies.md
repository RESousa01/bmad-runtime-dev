---
title: "Toolchain and Dependencies"
authority: current
repository_commit: f18ef124a6e61754391b793a1b37a8a0f67491ab
research_cutoff: 2026-07-16
claim_ids: [KB-TOOL-001, KB-TOOL-002, KB-TOOL-003, KB-TOOL-004, KB-TOOL-005]
---

# Toolchain and Dependencies

The project pins Node 24.18.0, pnpm 11.12.0, TypeScript 7.0.2, Rust 1.97.0,
Tauri 2.11.5, React 19.2.7, Vite 8.1.4, and .NET SDK 10.0.302
[KB-TOOL-001]. Pin parity is validated mechanically; a pin is not automatically
described as the newest release.

On the research cutoff, Node published 24.18.0 as LTS [KB-TOOL-002],
TypeScript 7.0 lacked a public programmatic API [KB-TOOL-003], React listed
19.2.7 in the 19.2 patch line [KB-TOOL-004], and Vite supported the 8.1 line
[KB-TOOL-005]. Revalidate these external facts after the cutoff.
