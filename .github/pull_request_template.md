## Summary

Describe the bounded change and its authority source.

## Evidence

- [ ] Narrow tests/generator checks are listed with fresh results.
- [ ] `git diff --check` passes.
- [ ] Product source gates were run, or the reason they are out of scope is stated.
- [ ] `pnpm docs:verify` passes when maintained docs, metadata, pins, source locks, or archive
      state changed.

## Documentation and lifecycle

- [ ] Current behavior/modes changed and `docs/current` was updated, or no update is required.
- [ ] An architecture decision changed and an ADR was added/amended, or no ADR is required.
- [ ] An operational procedure changed and its runbook was updated, or no runbook is required.
- [ ] A source-reference identity/license decision changed and the source lock/notices were
      updated, or no reference update is required.
- [ ] A plan/handoff completed and was archived, or no lifecycle update is required.
- [ ] Legacy archive content was preserved with reproducible hashes.

## Boundaries

- [ ] Unrelated user changes were preserved.
- [ ] Source/build, installer, signing, Azure/Foundry, clean-machine, and pilot evidence are
      reported separately.
- [ ] No imported third-party script was executed.
- [ ] External blockers and unresolved owner/storage/review requirements are explicit.

