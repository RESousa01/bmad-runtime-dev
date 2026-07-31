---
title: "Forge contribution guide"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_commit: "5836aea13453acd4ccadce00ad8d992201bd0e3f"
source_paths:
  - CONTRIBUTING.md
  - AGENTS.md
  - package.json
supersedes: []
---

# Forge contribution guide

## 1. Establish scope and ownership

Before editing, capture:

```powershell
git branch --show-current
git rev-parse HEAD
git status --short
```

Choose one bounded roadmap item or accepted task. Write down:

- source/test authority and related ADRs;
- acceptance checks and evidence output;
- files you own and files already owned by another task;
- external prerequisites and rollback;
- whether the result changes current documentation.

If the worktree is dirty, preserve unrelated changes. Do not clean, regenerate, stage, commit, or
push another owner’s files merely because they are visible.

## 2. Implement against the authority boundary

- Keep filesystem, process, credential, provider, and update effects in the native host.
- Treat workspace content and model output as untrusted data.
- Keep decisions single-use and effects checkpointed, evidenced, and recoverable.
- Use owner generators; avoid broad mechanical rewrites.
- Never execute imported reference-source code.
- Do not convert planned or protocol-known capabilities into available ones without production-path
  proof.

## 3. Verify in increasing scope

1. Reproduce the behavior with the narrowest test or generator `--check`.
2. Run affected package tests, typecheck, lint, or focused Rust suites.
3. Run `pnpm docs:generate` and `pnpm docs:verify` for maintained-document changes.
4. Run `pnpm verify:source` for first-party source changes.
5. Run deferred native, installer, Azure, signing, or pilot gates only when required and authorized.
6. Always run `git diff --check` and inspect final status.

Record exact commands, exit codes, failures, and skipped external gates. A partial green suite must
not be summarized as whole-repository success.

## 4. Update durable guidance

Update `docs/current` when verified behavior, architecture, provider/auth direction, BMAD status,
pins, release gates, or operating procedures change. Update an ADR when the decision changes.
Archive completed plans and handoffs; do not rewrite historical evidence as current truth.

Maintained-document changes require complete frontmatter and regeneration of the knowledge index.
See the [documentation policy](documentation-policy.md).

## 5. Prepare the handoff

Report:

- outcome and files changed;
- verification that passed;
- verification that failed or was not run;
- preserved unrelated worktree state;
- external blockers and follow-up owners;
- commit, branch, push, or PR state when applicable.

Branch, push, release, deployment, and external-message actions still require their normal
authorization. A plan, branch, receipt label, agent report, or commit is not completion evidence by
itself.
